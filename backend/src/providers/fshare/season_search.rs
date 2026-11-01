use async_trait::async_trait;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::time::Instant;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Item {
    pub code: String,
    pub name: String,
    pub parent: Option<String>,
    pub directory: bool,
    pub season: Option<u32>,
    pub episode: Option<u32>,
    pub ambiguous: bool,
}

#[async_trait]
pub trait SeasonSearchRepository: Sync {
    async fn mapped_folders(&self, tmdb_id: i64) -> Result<Vec<Item>, String>;
    async fn search_folders(&self, query: &str, tmdb_id: Option<i64>) -> Result<Vec<Item>, String>;
    async fn descendants_batch(&self, folder_codes: &[String]) -> Result<Vec<Item>, String>;
    async fn search_files(&self, query: &str) -> Result<Vec<Item>, String>;
    async fn siblings(&self, file_code: &str) -> Result<Vec<Item>, String>;
}

#[derive(Debug, Clone)]
pub struct Request {
    pub title: String,
    pub aliases: Vec<String>,
    pub year: u32,
    pub season: u32,
    pub targets: BTreeSet<u32>,
    pub tmdb_id: Option<i64>,
}

impl Request {
    pub fn new(
        title: impl Into<String>,
        year: u32,
        season: u32,
        targets: std::ops::RangeInclusive<u32>,
    ) -> Self {
        Self {
            title: title.into(),
            aliases: Vec::new(),
            year,
            season,
            targets: targets.collect(),
            tmdb_id: None,
        }
    }

    pub fn with_aliases(mut self, aliases: Vec<String>) -> Self {
        self.aliases = aliases;
        self
    }

    pub fn with_targets(mut self, targets: BTreeSet<u32>) -> Self {
        self.targets = targets;
        self
    }

    pub fn with_tmdb_id(mut self, tmdb_id: Option<i64>) -> Self {
        self.tmdb_id = tmdb_id;
        self
    }

    pub fn folder_queries(&self) -> Vec<String> {
        let mut out = Vec::new();
        let mut seen = HashSet::new();
        for title in std::iter::once(&self.title).chain(self.aliases.iter()) {
            for q in [
                format!("{title} Season {}", self.season),
                format!("{title} S{:02}", self.season),
                format!("{title} {} Season {}", self.year, self.season),
                title.clone(),
            ] {
                if seen.insert(q.clone()) {
                    out.push(q);
                }
            }
        }
        out
    }

    fn file_query(&self, episode: u32) -> String {
        format!("{} S{:02}E{:02}", self.title, self.season, episode)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Terminal {
    Complete,
    Degraded,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
pub struct WorkLedger {
    pub metadata_folder_hits: usize,
    pub folder_queries: usize,
    pub folder_hits: usize,
    pub folder_expansions: usize,
    pub file_queries: usize,
    pub parent_expansions: usize,
    pub errors: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Plan {
    pub alternatives: BTreeMap<u32, Vec<Item>>,
    pub covered: BTreeSet<u32>,
    pub missing: BTreeSet<u32>,
    pub ambiguous: BTreeSet<u32>,
    pub terminal: Terminal,
    pub ledger: WorkLedger,
    pub phase_durations_ms: BTreeMap<String, u64>,
    pub total_duration_ms: u64,
}

pub async fn plan_full_season<R: SeasonSearchRepository>(repo: &R, request: Request) -> Plan {
    let total_start = Instant::now();
    let folder_start = Instant::now();
    let mut ledger = WorkLedger::default();
    let mut seen_folders = HashSet::new();

    // Metadata is a fast path only; exhaustive FTS discovery still runs below.
    if let Some(tmdb_id) = request.tmdb_id {
        match repo.mapped_folders(tmdb_id).await {
            Ok(folders) => {
                for folder in folders.into_iter().filter(|item| item.directory) {
                    ledger.metadata_folder_hits += 1;
                    if seen_folders.insert(folder.code.clone()) {
                        ledger.folder_expansions += 1;
                    }
                }
            }
            Err(_) => ledger.errors += 1,
        }
    }

    // Exhaust every variant and every folder hit. Coverage never terminates this phase.
    for query in request.folder_queries() {
        ledger.folder_queries += 1;
        match repo.search_folders(&query, request.tmdb_id).await {
            Ok(hits) => {
                for folder in hits.into_iter().filter(|x| x.directory) {
                    ledger.folder_hits += 1;
                    if seen_folders.insert(folder.code.clone()) {
                        ledger.folder_expansions += 1;
                    }
                }
            }
            Err(_) => ledger.errors += 1,
        }
    }

    let roots = seen_folders.into_iter().collect::<Vec<_>>();
    let candidates = match repo.descendants_batch(&roots).await {
        Ok(items) => items,
        Err(_) => {
            ledger.errors += 1;
            Vec::new()
        }
    };
    let mut mapped = map_candidates(&request, candidates);
    let folder_ms = folder_start.elapsed().as_millis() as u64;
    let fallback_start = Instant::now();
    let mut pending: BTreeSet<u32> = request
        .targets
        .difference(&mapped.covered)
        .filter(|episode| !mapped.ambiguous.contains(episode))
        .copied()
        .collect();
    while let Some(episode) = pending.iter().next().copied() {
        pending.remove(&episode);
        ledger.file_queries += 1;
        match repo.search_files(&request.file_query(episode)).await {
            Ok(hits) => {
                let mut expanded = hits.clone();
                for hit in hits {
                    ledger.parent_expansions += 1;
                    match repo.siblings(&hit.code).await {
                        Ok(items) => expanded.extend(items),
                        Err(_) => ledger.errors += 1,
                    }
                }
                mapped.merge(map_candidates(&request, expanded));
                pending.retain(|e| !mapped.covered.contains(e));
            }
            Err(_) => ledger.errors += 1,
        }
    }

    let missing = request
        .targets
        .difference(&mapped.covered)
        .copied()
        .collect::<BTreeSet<_>>();
    let terminal = if ledger.errors == 0 && missing.is_empty() && mapped.ambiguous.is_empty() {
        Terminal::Complete
    } else {
        Terminal::Degraded
    };
    let mut phase_durations_ms = BTreeMap::new();
    phase_durations_ms.insert("folder_graph".to_string(), folder_ms);
    phase_durations_ms.insert(
        "file_fallback".to_string(),
        fallback_start.elapsed().as_millis() as u64,
    );
    Plan {
        alternatives: mapped.alternatives,
        covered: mapped.covered,
        missing,
        ambiguous: mapped.ambiguous,
        terminal,
        ledger,
        phase_durations_ms,
        total_duration_ms: total_start.elapsed().as_millis() as u64,
    }
}

fn db_item(x: crate::db::sqlite::CachedFolderItem) -> Item {
    let ambiguous = episode_markers(&x.name).len() > 1;
    Item {
        code: x.linkcode,
        name: x.name,
        parent: (!x.parent_linkcode.is_empty()).then_some(x.parent_linkcode),
        directory: x.is_directory,
        season: x.season,
        episode: x.episode,
        ambiguous,
    }
}

/// Adapter: the production SQLite folder cache graph is the season-search repository.
#[async_trait]
impl SeasonSearchRepository for crate::db::Db {
    async fn mapped_folders(&self, tmdb_id: i64) -> Result<Vec<Item>, String> {
        self.folder_cache_folders_by_tmdb_id(tmdb_id, "tv")
            .map(|items| items.into_iter().map(db_item).collect())
            .map_err(|error| error.to_string())
    }
    async fn search_folders(&self, query: &str, tmdb_id: Option<i64>) -> Result<Vec<Item>, String> {
        match tmdb_id {
            Some(tmdb_id) => self.search_folder_cache_folders_for_tmdb(query, tmdb_id),
            None => self.search_folder_cache_exhaustive(query, true),
        }
        .map(|v| v.into_iter().map(db_item).collect())
        .map_err(|e| e.to_string())
    }
    async fn descendants_batch(&self, codes: &[String]) -> Result<Vec<Item>, String> {
        self.folder_cache_descendants_batch(codes)
            .map(|v| v.into_iter().map(db_item).collect())
            .map_err(|e| e.to_string())
    }
    async fn search_files(&self, query: &str) -> Result<Vec<Item>, String> {
        self.search_folder_cache_exhaustive(query, false)
            .map(|v| v.into_iter().map(db_item).collect())
            .map_err(|e| e.to_string())
    }
    async fn siblings(&self, code: &str) -> Result<Vec<Item>, String> {
        self.folder_cache_siblings(code)
            .map(|v| v.into_iter().map(db_item).collect())
            .map_err(|e| e.to_string())
    }
}

#[derive(Default)]
struct Mapping {
    alternatives: BTreeMap<u32, Vec<Item>>,
    covered: BTreeSet<u32>,
    ambiguous: BTreeSet<u32>,
}
impl Mapping {
    fn merge(&mut self, other: Self) {
        self.ambiguous.extend(other.ambiguous);
        for (episode, items) in other.alternatives {
            let alternatives = self.alternatives.entry(episode).or_default();
            for item in items {
                if !alternatives.iter().any(|x| x.code == item.code) {
                    alternatives.push(item);
                }
            }
        }
        self.covered = self
            .alternatives
            .keys()
            .copied()
            .filter(|e| !self.ambiguous.contains(e))
            .collect();
    }
}

fn map_candidates(request: &Request, items: Vec<Item>) -> Mapping {
    let mut out = Mapping::default();
    for item in items.into_iter().filter(|x| !x.directory) {
        if item.ambiguous {
            for e in episode_markers(&item.name).intersection(&request.targets) {
                out.ambiguous.insert(*e);
            }
            continue;
        }
        if item.season != Some(request.season) || item.episode.is_none() {
            continue;
        }
        let episode = item.episode.unwrap();
        if !request.targets.contains(&episode) {
            continue;
        } // rejects E09/E10 for official E01-E08
        out.alternatives.entry(episode).or_default().push(item);
    }
    for items in out.alternatives.values_mut() {
        items.sort_by(|a, b| a.code.cmp(&b.code));
        items.dedup_by(|a, b| a.code == b.code);
    }
    out.covered = out
        .alternatives
        .keys()
        .copied()
        .filter(|e| !out.ambiguous.contains(e))
        .collect();
    out
}

fn episode_markers(name: &str) -> BTreeSet<u32> {
    let upper = name.to_ascii_uppercase();
    let bytes = upper.as_bytes();
    let mut found = BTreeSet::new();
    for i in 0..bytes.len().saturating_sub(1) {
        if bytes[i] == b'E' && bytes[i + 1].is_ascii_digit() {
            let digits: String = bytes[i + 1..]
                .iter()
                .take_while(|b| b.is_ascii_digit())
                .map(|b| *b as char)
                .collect();
            if let Ok(n) = digits.parse() {
                found.insert(n);
            }
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::RefCell, collections::HashMap};

    struct FixtureRepo {
        folders: HashMap<String, Result<Vec<Item>, String>>,
        descendants: HashMap<String, Vec<Item>>,
        files: HashMap<String, Vec<Item>>,
        sibling_map: HashMap<String, Vec<Item>>,
        folder_queries: RefCell<Vec<String>>,
        file_queries: RefCell<Vec<u32>>,
    }
    unsafe impl Sync for FixtureRepo {}
    fn file(code: &str, episode: &str, parent: &str) -> Item {
        Item {
            code: code.into(),
            name: format!("Show.S01{episode}.mkv"),
            parent: Some(parent.into()),
            directory: false,
            season: Some(1),
            episode: episode_markers(&format!("Show.S01{episode}.mkv"))
                .into_iter()
                .next(),
            ambiguous: episode_markers(&format!("Show.S01{episode}.mkv")).len() > 1,
        }
    }
    fn dir(code: &str) -> Item {
        Item {
            code: code.into(),
            name: code.into(),
            parent: None,
            directory: true,
            season: None,
            episode: None,
            ambiguous: false,
        }
    }
    impl FixtureRepo {
        fn approved() -> Self {
            let request = Request::new("Stranger Things", 2016, 1, 1..=8);
            let mut folders = HashMap::new();
            for q in request.folder_queries() {
                folders.insert(q, Ok(vec![]));
            }
            folders.insert(
                "Stranger Things Season 1".into(),
                Ok(vec![dir("a"), dir("b")]),
            );
            folders.insert("Stranger Things".into(), Ok(vec![dir("c")]));
            let mut descendants = HashMap::new();
            descendants.insert(
                "a".into(),
                (1..=4)
                    .map(|e| Item {
                        code: format!("a{e}"),
                        name: format!("Stranger.Things.S01E{e:02}.mkv"),
                        parent: Some("a".into()),
                        directory: false,
                        season: Some(1),
                        episode: Some(e),
                        ambiguous: false,
                    })
                    .collect(),
            );
            descendants.insert(
                "b".into(),
                (3..=8)
                    .map(|e| Item {
                        code: format!("b{e}"),
                        name: format!("Stranger.Things.S01E{e:02}.mkv"),
                        parent: Some("b".into()),
                        directory: false,
                        season: Some(1),
                        episode: Some(e),
                        ambiguous: false,
                    })
                    .collect(),
            );
            descendants.insert(
                "c".into(),
                (9..=10)
                    .map(|e| Item {
                        code: format!("c{e}"),
                        name: format!("Stranger.Things.S01E{e:02}.mkv"),
                        parent: Some("c".into()),
                        directory: false,
                        season: Some(1),
                        episode: Some(e),
                        ambiguous: false,
                    })
                    .collect(),
            );
            Self {
                folders,
                descendants,
                files: HashMap::new(),
                sibling_map: HashMap::new(),
                folder_queries: RefCell::new(vec![]),
                file_queries: RefCell::new(vec![]),
            }
        }
        fn fallback() -> Self {
            let request = Request::new("Show", 2020, 1, 1..=4);
            let mut folders = HashMap::new();
            for q in request.folder_queries() {
                folders.insert(q, Ok(vec![]));
            }
            folders.insert("Show S01".into(), Ok(vec![dir("base")]));
            Self {
                folders,
                descendants: [(
                    "base".into(),
                    vec![file("1", "E01", "base"), file("2", "E02", "base")],
                )]
                .into(),
                files: [("Show S01E03".into(), vec![file("3", "E03", "parent")])].into(),
                sibling_map: [(
                    "3".into(),
                    vec![file("3", "E03", "parent"), file("4", "E04", "parent")],
                )]
                .into(),
                folder_queries: RefCell::new(vec![]),
                file_queries: RefCell::new(vec![]),
            }
        }
        fn ambiguous() -> Self {
            let mut x = Self::fallback();
            x.descendants.insert(
                "base".into(),
                vec![
                    file("1", "E01", "base"),
                    Item {
                        code: "m".into(),
                        name: "Show.S01E02-E03.mkv".into(),
                        parent: Some("base".into()),
                        directory: false,
                        season: Some(1),
                        episode: Some(2),
                        ambiguous: true,
                    },
                ],
            );
            x.files.clear();
            x
        }
        fn failing() -> Self {
            let mut x = Self::fallback();
            for value in x.folders.values_mut() {
                *value = Err("offline".into())
            }
            x.files.clear();
            x
        }
    }
    #[async_trait]
    impl SeasonSearchRepository for FixtureRepo {
        async fn mapped_folders(&self, _tmdb_id: i64) -> Result<Vec<Item>, String> {
            Ok(Vec::new())
        }
        async fn search_folders(
            &self,
            q: &str,
            _tmdb_id: Option<i64>,
        ) -> Result<Vec<Item>, String> {
            self.folder_queries.borrow_mut().push(q.into());
            self.folders.get(q).cloned().unwrap_or(Ok(vec![]))
        }
        async fn descendants_batch(&self, codes: &[String]) -> Result<Vec<Item>, String> {
            Ok(codes
                .iter()
                .flat_map(|c| self.descendants.get(c).cloned().unwrap_or_default())
                .collect())
        }
        async fn search_files(&self, q: &str) -> Result<Vec<Item>, String> {
            self.file_queries
                .borrow_mut()
                .push(q.rsplit('E').next().unwrap().parse().unwrap());
            Ok(self.files.get(q).cloned().unwrap_or_default())
        }
        async fn siblings(&self, c: &str) -> Result<Vec<Item>, String> {
            Ok(self.sibling_map.get(c).cloned().unwrap_or_default())
        }
    }

    #[tokio::test]
    async fn exhaustive_folder_phase_rejects_out_of_range_and_keeps_alternatives() {
        let repo = FixtureRepo::approved();
        let p = plan_full_season(&repo, Request::new("Stranger Things", 2016, 1, 1..=8)).await;
        assert_eq!(p.covered, (1..=8).collect());
        assert!(!p.alternatives.contains_key(&9));
        assert_eq!(p.alternatives[&3].len(), 2);
        assert_eq!(
            p.ledger,
            WorkLedger {
                folder_queries: 4,
                folder_hits: 3,
                folder_expansions: 3,
                ..Default::default()
            }
        );
        assert_eq!(p.terminal, Terminal::Complete);
    }
    #[tokio::test]
    async fn missing_only_fallback_expands_parent_and_removes_pending_work() {
        let r = FixtureRepo::fallback();
        let p = plan_full_season(&r, Request::new("Show", 2020, 1, 1..=4)).await;
        assert_eq!(p.covered, (1..=4).collect());
        assert_eq!(&*r.file_queries.borrow(), &[3]);
        assert_eq!(p.ledger.file_queries, 1);
        assert_eq!(p.ledger.parent_expansions, 1);
    }
    #[tokio::test]
    async fn ambiguous_multi_episode_fails_closed() {
        let r = FixtureRepo::ambiguous();
        let p = plan_full_season(&r, Request::new("Show", 2020, 1, 1..=3)).await;
        assert_eq!(p.covered, [1].into_iter().collect());
        assert_eq!(p.ambiguous, [2, 3].into_iter().collect());
        assert_eq!(p.terminal, Terminal::Degraded);
    }
    #[tokio::test]
    async fn failures_terminal_degraded_after_all_folder_queries() {
        let r = FixtureRepo::failing();
        let p = plan_full_season(&r, Request::new("Show", 2020, 1, 1..=2)).await;
        assert_eq!(p.terminal, Terminal::Degraded);
        assert_eq!(p.ledger.folder_queries, 4);
        assert!(p.ledger.errors >= 4);
    }
}
