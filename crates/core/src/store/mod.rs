//! On-device storage: history, feedback, the taste model, settings and the wallpaper cache
//! index. Everything stays on the device (see docs/privacy.md).

use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::analysis::Swatch;
use crate::error::{Error, Result};
use crate::score::{Features, Model};
use crate::source::Candidate;

pub mod cache;
mod db;

pub use cache::{ImageCache, Ready, DEFAULT_CACHE_BUDGET_BYTES};
pub use db::SCHEMA_VERSION;

/// Seconds since the Unix epoch.
pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// A row of the history list.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: i64,
    pub shown_at: i64,
    pub candidate: Candidate,
    pub palette: Vec<Swatch>,
    /// `Some(true)` liked, `Some(false)` skipped, `None` no feedback yet.
    pub liked: Option<bool>,
}

pub struct Store {
    pub(crate) conn: Connection,
}

impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        Ok(Self {
            conn: db::open(path)?,
        })
    }

    pub fn open_in_memory() -> Result<Self> {
        Ok(Self {
            conn: db::open_in_memory()?,
        })
    }

    // --- settings -------------------------------------------------------------------------

    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| {
                r.get(0)
            })
            .optional()?)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO settings(key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [key, value],
        )?;
        Ok(())
    }

    pub fn delete_setting(&self, key: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM settings WHERE key = ?1", [key])?;
        Ok(())
    }

    /// Typed settings stored as JSON.
    pub fn get_json<T: for<'de> Deserialize<'de>>(&self, key: &str) -> Result<Option<T>> {
        self.get_setting(key)?
            .map(|s| serde_json::from_str(&s).map_err(Error::from))
            .transpose()
    }

    pub fn set_json<T: Serialize>(&self, key: &str, value: &T) -> Result<()> {
        self.set_setting(key, &serde_json::to_string(value)?)
    }

    // --- model ----------------------------------------------------------------------------

    pub fn load_model(&self) -> Result<Model> {
        let json: Option<String> = self
            .conn
            .query_row("SELECT json FROM model WHERE id = 1", [], |r| r.get(0))
            .optional()?;
        match json {
            // A model we can't read (older feature set…) is replaced by the prior, not an error:
            // the user loses a little learning, not the app.
            Some(j) => Ok(serde_json::from_str(&j).unwrap_or_default()),
            None => Ok(Model::default()),
        }
    }

    pub fn save_model(&self, model: &Model) -> Result<()> {
        self.conn.execute(
            "INSERT INTO model(id, json) VALUES (1, ?1)
             ON CONFLICT(id) DO UPDATE SET json = excluded.json",
            [serde_json::to_string(model)?],
        )?;
        Ok(())
    }

    pub fn reset_model(&self) -> Result<()> {
        self.conn.execute("DELETE FROM model", [])?;
        Ok(())
    }

    // --- history --------------------------------------------------------------------------

    /// Record that `candidate` became the wallpaper.
    pub fn record_shown(
        &self,
        candidate: &Candidate,
        features: &Features,
        palette: &[Swatch],
        dhash: u64,
        at: i64,
    ) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO history(candidate_id, shown_at, dhash, place_label, candidate, features, palette)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                candidate.id,
                at,
                dhash as i64,
                candidate.place_label,
                serde_json::to_string(candidate)?,
                serde_json::to_string(features)?,
                serde_json::to_string(palette)?,
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// dHashes of the last `n` shown photos, newest first.
    pub fn recent_hashes(&self, n: u32) -> Result<Vec<u64>> {
        let mut stmt = self
            .conn
            .prepare("SELECT dhash FROM history ORDER BY shown_at DESC, id DESC LIMIT ?1")?;
        let rows = stmt.query_map([n], |r| r.get::<_, i64>(0))?;
        rows.map(|r| r.map(|h| h as u64).map_err(Error::from))
            .collect()
    }

    pub fn was_shown(&self, candidate_id: &str) -> Result<bool> {
        Ok(self
            .conn
            .query_row(
                "SELECT 1 FROM history WHERE candidate_id = ?1 LIMIT 1",
                [candidate_id],
                |_| Ok(()),
            )
            .optional()?
            .is_some())
    }

    /// Newest first.
    pub fn history(&self, limit: u32, offset: u32) -> Result<Vec<HistoryEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT h.id, h.shown_at, h.candidate, h.palette, f.liked
             FROM history h LEFT JOIN feedback f ON f.candidate_id = h.candidate_id
             ORDER BY h.shown_at DESC, h.id DESC LIMIT ?1 OFFSET ?2",
        )?;
        let rows = stmt.query_map(params![limit, offset], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, Option<bool>>(4)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (id, shown_at, candidate, palette, liked) = row?;
            out.push(HistoryEntry {
                id,
                shown_at,
                candidate: serde_json::from_str(&candidate)?,
                palette: serde_json::from_str(&palette)?,
                liked,
            });
        }
        Ok(out)
    }

    /// The photo that is the wallpaper right now.
    pub fn current(&self) -> Result<Option<HistoryEntry>> {
        Ok(self.history(1, 0)?.into_iter().next())
    }

    pub fn history_len(&self) -> Result<u64> {
        Ok(self
            .conn
            .query_row("SELECT COUNT(*) FROM history", [], |r| r.get::<_, i64>(0))?
            as u64)
    }

    /// Delete history older than `keep_days`, but never the most recent `keep_min` entries.
    pub fn prune_history(&self, keep_days: u32, keep_min: u32, now: i64) -> Result<u64> {
        let cutoff = now - i64::from(keep_days) * 86_400;
        let n = self.conn.execute(
            "DELETE FROM history WHERE shown_at < ?1 AND id NOT IN
               (SELECT id FROM history ORDER BY shown_at DESC, id DESC LIMIT ?2)",
            params![cutoff, keep_min],
        )?;
        Ok(n as u64)
    }

    // --- feedback -------------------------------------------------------------------------

    /// Store a like/skip for a photo that was shown and teach the model from it.
    /// Repeating the same verdict is a no-op; changing it (like → skip) is a new lesson. SGD
    /// can't be undone, so the old lesson stays, softened by the L2 pull towards the prior.
    pub fn record_feedback(&mut self, candidate_id: &str, liked: bool, at: i64) -> Result<Model> {
        let tx = self.conn.transaction()?;
        let features_json: String = tx
            .query_row(
                "SELECT features FROM history WHERE candidate_id = ?1 ORDER BY id DESC LIMIT 1",
                [candidate_id],
                |r| r.get(0),
            )
            .optional()?
            .ok_or_else(|| Error::NotFound(format!("photo {candidate_id} was never shown")))?;
        let features: Features = serde_json::from_str(&features_json)?;

        let previous: Option<bool> = tx
            .query_row(
                "SELECT liked FROM feedback WHERE candidate_id = ?1",
                [candidate_id],
                |r| r.get(0),
            )
            .optional()?;
        tx.execute(
            "INSERT INTO feedback(candidate_id, liked, at) VALUES (?1, ?2, ?3)
             ON CONFLICT(candidate_id) DO UPDATE SET liked = excluded.liked, at = excluded.at",
            params![candidate_id, liked, at],
        )?;

        let model_json: Option<String> = tx
            .query_row("SELECT json FROM model WHERE id = 1", [], |r| r.get(0))
            .optional()?;
        let mut model: Model = model_json
            .and_then(|j| serde_json::from_str(&j).ok())
            .unwrap_or_default();
        // The same verdict twice (double tap) must not count twice.
        if previous != Some(liked) {
            model.learn(&features, liked);
        }
        tx.execute(
            "INSERT INTO model(id, json) VALUES (1, ?1)
             ON CONFLICT(id) DO UPDATE SET json = excluded.json",
            [serde_json::to_string(&model)?],
        )?;
        tx.commit()?;
        Ok(model)
    }

    pub fn feedback_counts(&self) -> Result<(u64, u64)> {
        let (likes, skips): (i64, i64) = self.conn.query_row(
            "SELECT COALESCE(SUM(liked), 0), COALESCE(SUM(1 - liked), 0) FROM feedback",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        Ok((likes as u64, skips as u64))
    }

    // --- wipe -----------------------------------------------------------------------------

    /// "Forget everything": history, feedback, model, cache index. Settings are kept.
    pub fn forget_all(&self) -> Result<()> {
        self.conn.execute_batch(
            "DELETE FROM history; DELETE FROM feedback; DELETE FROM model; DELETE FROM cache;",
        )?;
        Ok(())
    }
}

/// Helpers for tests that need to damage the database on purpose.
#[doc(hidden)]
pub mod testing {
    use super::*;

    /// Store a model blob that no longer deserialises (as after a feature-set change).
    pub fn corrupt_model(store: &Store) {
        store
            .conn
            .execute(
                "INSERT INTO model(id, json) VALUES (1, '{\"weights\": [1, 2]}')
                 ON CONFLICT(id) DO UPDATE SET json = excluded.json",
                [],
            )
            .unwrap();
    }

    /// Pretend the file was written by a newer app version.
    pub fn set_user_version(path: &Path, version: u32) {
        let conn = Connection::open(path).unwrap();
        conn.pragma_update(None, "user_version", version).unwrap();
    }
}
