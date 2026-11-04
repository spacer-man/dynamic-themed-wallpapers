//! Rendered wallpapers on disk, indexed in SQLite, with a size budget and LRU eviction.
//!
//! The daily change is fully offline: prefetch (Wi-Fi + charging) renders several wallpapers
//! into this cache as `ready`; the daily job just takes the best `ready` one.

use std::path::{Path, PathBuf};

use rusqlite::{params, OptionalExtension};

use super::Store;
use crate::analysis::Swatch;
use crate::error::{Error, Result};
use crate::pipeline::Evaluated;
use crate::score::Features;
use crate::source::Candidate;

pub const DEFAULT_CACHE_BUDGET_BYTES: u64 = 150 * 1024 * 1024;

/// A prefetched wallpaper, ready to be installed.
#[derive(Clone, Debug)]
pub struct Ready {
    pub candidate: Candidate,
    pub features: Features,
    pub palette: Vec<Swatch>,
    pub dhash: u64,
    pub score: f32,
    pub path: PathBuf,
}

pub struct ImageCache<'a> {
    store: &'a Store,
    dir: PathBuf,
    budget_bytes: u64,
}

/// File names never contain anything from the (untrusted) candidate id: a hash only.
fn file_name(candidate_id: &str) -> String {
    // FNV-1a 64, twice with different seeds → 128 bits, plenty against accidental collisions.
    fn fnv(seed: u64, s: &str) -> u64 {
        s.bytes().fold(seed, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3)
        })
    }
    format!(
        "{:016x}{:016x}.jpg",
        fnv(0xcbf2_9ce4_8422_2325, candidate_id),
        fnv(0x8422_2325_cbf2_9ce4, candidate_id)
    )
}

impl<'a> ImageCache<'a> {
    pub fn new(store: &'a Store, dir: impl Into<PathBuf>, budget_bytes: u64) -> Result<Self> {
        let dir = dir.into();
        std::fs::create_dir_all(&dir)?;
        Ok(Self {
            store,
            dir,
            budget_bytes,
        })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Render result goes to disk and into the `ready` queue. Evicts if over budget.
    pub fn put_ready(&self, e: &Evaluated, jpeg: &[u8], now: i64) -> Result<PathBuf> {
        let score = e
            .verdict
            .map_err(|r| Error::Store(format!("refusing to cache a rejected photo ({r:?})")))?;
        let name = file_name(&e.candidate.id);
        let path = self.dir.join(&name);
        let tmp = self.dir.join(format!("{name}.tmp"));
        std::fs::write(&tmp, jpeg)?;
        std::fs::rename(&tmp, &path)?;
        self.store.conn.execute(
            "INSERT INTO cache(candidate_id, file, bytes, created_at, last_used_at, status, score,
                               dhash, candidate, features, palette)
             VALUES (?1, ?2, ?3, ?4, ?4, 'ready', ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(candidate_id) DO UPDATE SET
                bytes = excluded.bytes, created_at = excluded.created_at,
                last_used_at = excluded.last_used_at, status = 'ready', score = excluded.score",
            params![
                e.candidate.id,
                name,
                jpeg.len() as i64,
                now,
                f64::from(score),
                e.analysis.dhash as i64,
                serde_json::to_string(&e.candidate)?,
                serde_json::to_string(&e.features)?,
                serde_json::to_string(&e.analysis.palette)?,
            ],
        )?;
        self.evict()?;
        Ok(path)
    }

    pub fn ready_count(&self) -> Result<u32> {
        Ok(self.store.conn.query_row(
            "SELECT COUNT(*) FROM cache WHERE status = 'ready'",
            [],
            |r| r.get(0),
        )?)
    }

    pub fn total_bytes(&self) -> Result<u64> {
        let n: i64 =
            self.store
                .conn
                .query_row("SELECT COALESCE(SUM(bytes), 0) FROM cache", [], |r| {
                    r.get(0)
                })?;
        Ok(n as u64)
    }

    /// Take the best prefetched wallpaper (highest score) and mark it used.
    pub fn take_next_ready(&self, now: i64) -> Result<Option<Ready>> {
        let tx_conn = &self.store.conn;
        let row = tx_conn
            .query_row(
                "SELECT candidate_id, file, score, dhash, candidate, features, palette
                 FROM cache WHERE status = 'ready' ORDER BY score DESC, created_at ASC LIMIT 1",
                [],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, f64>(2)?,
                        r.get::<_, i64>(3)?,
                        r.get::<_, String>(4)?,
                        r.get::<_, String>(5)?,
                        r.get::<_, String>(6)?,
                    ))
                },
            )
            .optional()?;
        let Some((id, file, score, dhash, candidate, features, palette)) = row else {
            return Ok(None);
        };
        let path = self.dir.join(&file);
        if !path.exists() {
            // Index says ready but the file is gone (cleared by the OS, manual deletion).
            tx_conn.execute("DELETE FROM cache WHERE candidate_id = ?1", [&id])?;
            return self.take_next_ready(now);
        }
        tx_conn.execute(
            "UPDATE cache SET status = 'used', last_used_at = ?2 WHERE candidate_id = ?1",
            params![id, now],
        )?;
        Ok(Some(Ready {
            candidate: serde_json::from_str(&candidate)?,
            features: serde_json::from_str(&features)?,
            palette: serde_json::from_str(&palette)?,
            dhash: dhash as u64,
            score: score as f32,
            path,
        }))
    }

    /// Delete files until the cache fits the budget. Used wallpapers go first (oldest first),
    /// then the lowest-scored prefetched ones. The most recently used file is never evicted:
    /// it is the current wallpaper.
    pub fn evict(&self) -> Result<u32> {
        let mut evicted = 0;
        while self.total_bytes()? > self.budget_bytes {
            let victim: Option<(String, String)> = self
                .store
                .conn
                .query_row(
                    "SELECT candidate_id, file FROM cache
                     WHERE candidate_id != COALESCE(
                         (SELECT candidate_id FROM cache WHERE status = 'used'
                          ORDER BY last_used_at DESC LIMIT 1), '')
                     ORDER BY CASE status WHEN 'used' THEN 0 ELSE 1 END, last_used_at ASC, score ASC
                     LIMIT 1",
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?;
            let Some((id, file)) = victim else { break };
            let _ = std::fs::remove_file(self.dir.join(file));
            self.store
                .conn
                .execute("DELETE FROM cache WHERE candidate_id = ?1", [id])?;
            evicted += 1;
        }
        Ok(evicted)
    }

    /// Make files and index agree: drop rows without a file and files without a row.
    /// Call on startup — Android may clear cache directories under storage pressure.
    pub fn reconcile(&self) -> Result<u32> {
        let mut removed = 0;
        let rows: Vec<(String, String)> = {
            let mut stmt = self
                .store
                .conn
                .prepare("SELECT candidate_id, file FROM cache")?;
            let r = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
            r.collect::<std::result::Result<_, _>>()?
        };
        let mut known = std::collections::HashSet::new();
        for (id, file) in rows {
            if self.dir.join(&file).exists() {
                known.insert(file);
            } else {
                self.store
                    .conn
                    .execute("DELETE FROM cache WHERE candidate_id = ?1", [id])?;
                removed += 1;
            }
        }
        for entry in std::fs::read_dir(&self.dir)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if !known.contains(&name) && entry.file_type()?.is_file() {
                let _ = std::fs::remove_file(entry.path());
                removed += 1;
            }
        }
        Ok(removed)
    }

    /// Delete every cached file ("clear cache" button). History stays.
    pub fn clear(&self) -> Result<()> {
        self.store.conn.execute("DELETE FROM cache", [])?;
        for entry in std::fs::read_dir(&self.dir)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                let _ = std::fs::remove_file(entry.path());
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_names_are_safe_and_stable() {
        let evil = "commons:../../etc/passwd\0";
        let n = file_name(evil);
        assert_eq!(n.len(), 36);
        assert!(n.bytes().all(|b| b.is_ascii_hexdigit()
            || b == b'.'
            || b == b'j'
            || b == b'p'
            || b == b'g'));
        assert_eq!(n, file_name(evil));
        assert_ne!(file_name("commons:1"), file_name("commons:2"));
    }
}
