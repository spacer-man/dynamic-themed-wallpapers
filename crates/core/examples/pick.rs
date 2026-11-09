//! CLI demo of the whole selection pipeline, no Android needed:
//!
//! ```sh
//! cargo run -p wallpaper-core --example pick -- --place "Kyoto" --tone dark --screen 1080x2400
//! ```
//!
//! Prints every candidate with its verdict and feature scores, then writes the chosen photo
//! (already cropped to the screen) to `out/`.

use std::process::ExitCode;

use wallpaper_core::http::UreqClient;
use wallpaper_core::pipeline::{self, Screen};
use wallpaper_core::score::{Model, ToneFilter, EPSILON, FEATURE_NAMES};
use wallpaper_core::source::{CandidateRequest, CommonsSource, Search, Source, UnsplashSource};
use wallpaper_core::{Scope, Wikidata};

struct Args {
    place: String,
    tone: ToneFilter,
    screen: Screen,
    lang: String,
    limit: u32,
    seed: u64,
    out_dir: String,
    unsplash_key: Option<String>,
    explain: bool,
}

const USAGE: &str =
    "usage: pick --place <text|Qid> [--tone any|dark|light] [--screen WxH] [--lang ru|en] \
[--limit N] [--seed N] [--out DIR] [--explain]\n\
Unsplash is used only when the UNSPLASH_ACCESS_KEY environment variable is set.";

fn parse_args() -> Result<Args, String> {
    let mut a = Args {
        place: String::new(),
        tone: ToneFilter::Any,
        screen: Screen {
            width: 1080,
            height: 2400,
        },
        lang: "en".into(),
        limit: 24,
        seed: 0,
        out_dir: "out".into(),
        unsplash_key: std::env::var("UNSPLASH_ACCESS_KEY")
            .ok()
            .filter(|k| !k.is_empty()),
        explain: false,
    };
    let mut it = std::env::args().skip(1);
    while let Some(flag) = it.next() {
        let mut value = |name: &str| it.next().ok_or_else(|| format!("{name} needs a value"));
        match flag.as_str() {
            "--place" => a.place = value("--place")?,
            "--tone" => {
                a.tone = match value("--tone")?.as_str() {
                    "any" => ToneFilter::Any,
                    "dark" => ToneFilter::Dark,
                    "light" => ToneFilter::Light,
                    other => return Err(format!("unknown tone {other:?}")),
                }
            }
            "--screen" => {
                let v = value("--screen")?;
                a.screen = Screen::parse(&v).ok_or_else(|| format!("bad screen size {v:?}"))?;
            }
            "--lang" => a.lang = value("--lang")?,
            "--limit" => a.limit = value("--limit")?.parse().map_err(|_| "bad --limit")?,
            "--seed" => a.seed = value("--seed")?.parse().map_err(|_| "bad --seed")?,
            "--out" => a.out_dir = value("--out")?,
            "--explain" => a.explain = true,
            "-h" | "--help" => return Err(USAGE.into()),
            other => return Err(format!("unknown argument {other:?}\n{USAGE}")),
        }
    }
    if a.place.trim().is_empty() {
        return Err(USAGE.into());
    }
    Ok(a)
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(a) => a,
        Err(msg) => {
            eprintln!("{msg}");
            return ExitCode::from(2);
        }
    };
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: &Args) -> Result<(), Box<dyn std::error::Error>> {
    let http = UreqClient::new();
    let wd = Wikidata::new(&http, &args.lang);

    let qid = if wallpaper_core::place::is_qid(&args.place) {
        args.place.clone()
    } else {
        let hits = wd.search(&args.place, 5)?;
        let first = hits.first().ok_or("no such place on Wikidata")?;
        println!(
            "place: {} ({}) — {}",
            first.label,
            first.qid,
            first.description.as_deref().unwrap_or("-")
        );
        first.qid.clone()
    };
    let place = wd.resolve(&qid)?;
    let scope = place.default_scope();
    println!("kind: {:?}, scope: {scope:?}", place.kind);

    let mut req = CandidateRequest::new(args.screen.width, args.screen.height);
    req.limit = args.limit;
    req.seed = if args.seed == 0 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs()
    } else {
        args.seed
    };
    if scope == Scope::Hierarchy {
        req.sub_places = wd.sub_places(&place, 40)?;
        println!("sub-places: {}", req.sub_places.len());
    }

    let commons = CommonsSource::new(&http);
    let unsplash = match &args.unsplash_key {
        Some(k) => Some(UnsplashSource::new(&http, k)?),
        None => None,
    };
    let search = Search::Place {
        place: &place,
        scope: &scope,
    };
    let mut candidates = commons.candidates(&search, &req)?;
    if let Some(u) = &unsplash {
        candidates.extend(u.candidates(&search, &req)?);
    }
    println!("candidates: {}", candidates.len());

    let (analysed, failed) = pipeline::analyse_all(&http, candidates, args.screen, 4);
    for (id, e) in &failed {
        eprintln!("skipped {id}: {e}");
    }
    let evaluated = pipeline::evaluate(
        analysed,
        &Model::default(),
        args.screen,
        args.tone,
        &[],
        false,
    );

    for e in &evaluated {
        let a = &e.analysis;
        let verdict = match e.verdict {
            Ok(s) => format!("score {s:.3}"),
            Err(r) => format!("REJECT {r:?}"),
        };
        println!(
            "- {:<60.60} {:?} luma {:.2} sharp {:>6.0} top {:.3} | {verdict}",
            e.candidate.title, a.tone, a.luma_median, a.sharpness, a.top_band_busyness
        );
        if args.explain {
            let feats: Vec<String> = FEATURE_NAMES
                .iter()
                .zip(e.features.0)
                .map(|(n, v)| format!("{n}={v:.2}"))
                .collect();
            println!("    {}", feats.join(" "));
        }
    }

    let mut rng = fastrand::Rng::with_seed(req.seed);
    let Some(i) = pipeline::choose(&evaluated, EPSILON, &mut rng) else {
        return Err("no candidate passed the filters (try --tone any or a bigger place)".into());
    };
    let chosen = &evaluated[i];
    let source: &dyn Source = match chosen.candidate.source {
        wallpaper_core::source::SourceKind::Commons => &commons,
        wallpaper_core::source::SourceKind::Unsplash => {
            unsplash.as_ref().ok_or("no unsplash source")?
        }
        other => return Err(format!("the demo does not use {}", other.name()).into()),
    };
    let jpeg = pipeline::render(&http, source, chosen, args.screen)?;
    std::fs::create_dir_all(&args.out_dir)?;
    let path = std::path::Path::new(&args.out_dir).join(format!(
        "{}.jpg",
        chosen.candidate.id.replace([':', '/'], "_")
    ));
    std::fs::write(&path, &jpeg)?;

    println!("\nchosen: {}", chosen.candidate.title);
    println!("where:  {}", chosen.candidate.place_label);
    println!("credit: {}", chosen.candidate.credit_line());
    println!("page:   {}", chosen.candidate.page_url);
    println!(
        "palette: {}",
        chosen
            .analysis
            .palette
            .iter()
            .map(|s| s.hex())
            .collect::<Vec<_>>()
            .join(" ")
    );
    println!("saved:  {} ({} KiB)", path.display(), jpeg.len() / 1024);
    Ok(())
}
