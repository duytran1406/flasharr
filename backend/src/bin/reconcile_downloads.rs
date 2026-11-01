#[path = "../utils/parser.rs"]
pub mod parser;
#[path = "../utils/smart_tokenizer.rs"]
mod smart_tokenizer;
mod utils {
    pub use crate::parser;
}

use anyhow::{Context, Result};
use parser::FilenameParser;
use rusqlite::Connection;
use serde::Serialize;
use smart_tokenizer::smart_parse;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    env, fs,
};

#[derive(Clone)]
struct Media {
    key: String,
    tmdb: Option<i64>,
    tvdb: Option<i64>,
    kind: String,
    title: String,
    original: Option<String>,
    year: Option<i64>,
}
#[derive(Serialize)]
struct Item {
    id: String,
    filename: String,
    action: String,
    reason: String,
    parsed_title: String,
    season: Option<u32>,
    episode: Option<u32>,
    current_media_key: Option<String>,
    target_media_key: Option<String>,
    target_batch_id: Option<String>,
}
#[derive(Serialize, Default)]
struct Summary {
    total: usize,
    already_canonical: usize,
    repair: usize,
    ambiguous: usize,
    unmatched: usize,
    delete_candidates: usize,
    split_show_groups: usize,
    duplicate_episode_groups: usize,
}
#[derive(Serialize)]
struct Report {
    summary: Summary,
    split_shows: BTreeMap<String, Vec<String>>,
    duplicate_episodes: BTreeMap<String, Vec<String>>,
    items: Vec<Item>,
}

fn norm(s: &str) -> String {
    s.chars()
        .flat_map(char::to_lowercase)
        .filter(|c| c.is_alphanumeric())
        .collect()
}
fn main() -> Result<()> {
    let db = env::args()
        .nth(1)
        .context("usage: reconcile_downloads <snapshot.db> <report.json>")?;
    let out = env::args().nth(2).context("missing report path")?;
    let c = Connection::open_with_flags(db, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let media: Vec<Media> = {
        let mut q=c.prepare("select media_key,tmdb_id,tvdb_id,media_type,title,original_title,year from media_items")?;
        let collected = q
            .query_map([], |r| {
                Ok(Media {
                    key: r.get(0)?,
                    tmdb: r.get(1)?,
                    tvdb: r.get(2)?,
                    kind: r.get(3)?,
                    title: r.get(4)?,
                    original: r.get(5)?,
                    year: r.get(6)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        collected
    };
    let by_key: HashMap<_, _> = media.iter().map(|m| (m.key.clone(), m)).collect();
    let valid_episodes: HashSet<(String, u32, u32)> = {
        let mut q =
            c.prepare("select media_key,season_number,episode_number from media_episodes")?;
        let collected = q
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<rusqlite::Result<_>>()?;
        collected
    };
    let mut title_index: HashMap<String, Vec<&Media>> = HashMap::new();
    for m in &media {
        for title in [&m.title, m.original.as_ref().unwrap_or(&m.title)] {
            title_index.entry(norm(title)).or_default().push(m);
        }
    }
    let mut rows=c.prepare("select id,filename,source_filename,batch_id,batch_name,tmdb_id,tvdb_id,media_type,media_key,tmdb_season,tmdb_episode from downloads where is_current=1 order by created_at,id")?;
    let rows = rows
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, Option<i64>>(5)?,
                r.get::<_, Option<i64>>(6)?,
                r.get::<_, String>(7)?,
                r.get::<_, Option<String>>(8)?,
                r.get::<_, Option<u32>>(9)?,
                r.get::<_, Option<u32>>(10)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut items = Vec::new();
    let mut show_batches: BTreeMap<String, HashSet<String>> = BTreeMap::new();
    let mut episodes: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut summary = Summary {
        total: rows.len(),
        ..Default::default()
    };
    for (id, filename, source, batch, _batch_name, tmdb, tvdb, kind, key, season, episode) in rows {
        let input = source.as_deref().unwrap_or(&filename);
        let smart = smart_parse(input);
        let legacy = FilenameParser::parse(input);
        let ps = season.or(smart.season).or(legacy.season);
        let pe = episode.or(smart.episode).or(legacy.episode);
        let pt = if smart.title.trim().is_empty() {
            legacy.title.clone()
        } else {
            smart.title.clone()
        };
        let current = key
            .as_ref()
            .and_then(|k| by_key.get(k).copied())
            .or_else(|| {
                media
                    .iter()
                    .find(|m| tmdb.is_some() && m.tmdb == tmdb && m.kind == kind)
            })
            .or_else(|| {
                media
                    .iter()
                    .find(|m| tvdb.is_some() && m.tvdb == tvdb && m.kind == kind)
            });
        let candidates = title_index
            .get(&norm(&pt))
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter(|m| kind.is_empty() || m.kind == kind)
            .filter(|m| smart.year.map(|y| m.year == Some(y as i64)).unwrap_or(true))
            .collect::<Vec<_>>();
        let target = current.or_else(|| {
            if candidates.len() == 1 {
                Some(candidates[0])
            } else {
                None
            }
        });
        let episode_valid=target.map(|m|m.kind!="tv"||matches!((ps,pe),(Some(s),Some(e)) if valid_episodes.contains(&(m.key.clone(),s,e)))).unwrap_or(false);
        let (action, reason) = if let Some(m) = target {
            if !episode_valid {
                (
                    "ambiguous",
                    "TV episode coordinates are absent from canonical episode catalog",
                )
            } else {
                let desired = Some(m.key.clone());
                let wrong = key != desired
                    || tmdb != m.tmdb
                    || tvdb != m.tvdb
                    || season != ps
                    || episode != pe;
                if wrong {
                    (
                        "repair",
                        "identity/episode metadata differs from canonical catalog",
                    )
                } else {
                    (
                        "already-canonical",
                        "identity and episode metadata already canonical",
                    )
                }
            }
        } else if candidates.len() > 1 {
            ("ambiguous", "multiple exact catalog title matches")
        } else {
            ("unmatched", "no exact canonical catalog match")
        };
        match action {
            "repair" => summary.repair += 1,
            "already-canonical" => summary.already_canonical += 1,
            "ambiguous" => summary.ambiguous += 1,
            _ => {
                summary.unmatched += 1;
                summary.delete_candidates += 1
            }
        }
        if let Some(m) = target {
            if m.kind == "tv" {
                if let Some(b) = batch.clone() {
                    show_batches.entry(m.key.clone()).or_default().insert(b);
                }
                if let (Some(s), Some(e)) = (ps, pe) {
                    episodes
                        .entry(format!("{}:{s}:{e}", m.key))
                        .or_default()
                        .push(id.clone());
                }
            }
        }
        let target_batch = target
            .filter(|m| m.kind == "tv")
            .map(|m| format!("media:{}", m.key));
        items.push(Item {
            id,
            filename,
            action: action.into(),
            reason: reason.into(),
            parsed_title: pt,
            season: ps,
            episode: pe,
            current_media_key: key,
            target_media_key: target.map(|m| m.key.clone()),
            target_batch_id: target_batch,
        });
    }
    let split_shows = show_batches
        .into_iter()
        .filter(|(_, v)| v.len() > 1)
        .map(|(k, v)| (k, v.into_iter().collect()))
        .collect::<BTreeMap<_, _>>();
    let duplicate_episodes = episodes
        .into_iter()
        .filter(|(_, v)| v.len() > 1)
        .collect::<BTreeMap<_, _>>();
    summary.split_show_groups = split_shows.len();
    summary.duplicate_episode_groups = duplicate_episodes.len();
    let report = Report {
        summary,
        split_shows,
        duplicate_episodes,
        items,
    };
    fs::create_dir_all(std::path::Path::new(&out).parent().unwrap())?;
    fs::write(out, serde_json::to_string_pretty(&report)?)?;
    println!("{}", serde_json::to_string(&report.summary)?);
    Ok(())
}
