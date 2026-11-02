use super::sqlite::Db;
use rusqlite::{params, OptionalExtension, Result, TransactionBehavior};
use serde::Serialize;
use std::collections::BTreeMap;
use uuid::Uuid;

pub const MIGRATION_VERSION: i64 = 2026072001;
const MIGRATION_SQL: &str = r#"
CREATE TABLE acquisition_plan(id TEXT PRIMARY KEY,idempotency_key TEXT NOT NULL UNIQUE,media_kind TEXT NOT NULL CHECK(media_kind IN('movie','tv_season')),show_tvdb_id INTEGER,movie_tmdb_id INTEGER,season_number INTEGER,mode TEXT NOT NULL CHECK(mode IN('CONSISTENT','FAST','MANUAL')),status TEXT NOT NULL,quality_policy TEXT NOT NULL,language_policy TEXT NOT NULL,cache_policy TEXT NOT NULL,created_at TEXT NOT NULL DEFAULT(datetime('now')),CHECK((media_kind='movie' AND movie_tmdb_id IS NOT NULL AND show_tvdb_id IS NULL AND season_number IS NULL) OR (media_kind='tv_season' AND show_tvdb_id IS NOT NULL AND season_number>=0 AND movie_tmdb_id IS NULL)));
CREATE TABLE source_candidate(id TEXT PRIMARY KEY,plan_id TEXT NOT NULL REFERENCES acquisition_plan(id) ON DELETE CASCADE,provider TEXT NOT NULL,provider_item_id TEXT NOT NULL,manifest_hash TEXT NOT NULL,manifest_revision TEXT NOT NULL,instance_id TEXT,cache_status TEXT NOT NULL,cache_checked_at TEXT,created_at TEXT NOT NULL DEFAULT(datetime('now')),UNIQUE(plan_id,provider,provider_item_id,manifest_hash,manifest_revision));
CREATE TABLE plan_selection(id TEXT PRIMARY KEY,plan_id TEXT NOT NULL REFERENCES acquisition_plan(id) ON DELETE CASCADE,candidate_id TEXT NOT NULL REFERENCES source_candidate(id) ON DELETE RESTRICT,media_leaf_key TEXT NOT NULL,selected INTEGER NOT NULL CHECK(selected IN(0,1)),stremio_file_idx INTEGER CHECK(stremio_file_idx IS NULL OR stremio_file_idx>=0),provider_file_id TEXT,mapping_confidence REAL NOT NULL CHECK(mapping_confidence>=0 AND mapping_confidence<=1),quality TEXT NOT NULL,language TEXT NOT NULL,status TEXT NOT NULL DEFAULT 'SELECTED',failure_message TEXT,created_at TEXT NOT NULL DEFAULT(datetime('now')),UNIQUE(plan_id,media_leaf_key,candidate_id,stremio_file_idx));
CREATE UNIQUE INDEX ux_plan_selected_leaf ON plan_selection(plan_id,media_leaf_key) WHERE selected=1 AND status='SELECTED';
CREATE TABLE provider_transfer(id TEXT PRIMARY KEY,provider_kind TEXT NOT NULL,provider_id TEXT NOT NULL,external_job_id TEXT,instance_id TEXT NOT NULL DEFAULT '',normalized_info_hash TEXT,state TEXT NOT NULL,progress REAL NOT NULL DEFAULT 0,size_bytes INTEGER,create_claimed_at TEXT,adopted_at TEXT,created_at TEXT NOT NULL DEFAULT(datetime('now')),updated_at TEXT NOT NULL DEFAULT(datetime('now')),CHECK(normalized_info_hash IS NULL OR normalized_info_hash=lower(normalized_info_hash)));
CREATE UNIQUE INDEX ux_transfer_hash ON provider_transfer(instance_id,normalized_info_hash) WHERE normalized_info_hash IS NOT NULL;
CREATE UNIQUE INDEX ux_transfer_external_job ON provider_transfer(provider_kind,provider_id,external_job_id) WHERE external_job_id IS NOT NULL;
CREATE TABLE media_artifact(id TEXT PRIMARY KEY,transfer_id TEXT NOT NULL REFERENCES provider_transfer(id) ON DELETE RESTRICT,file_idx INTEGER,provider_file_id TEXT,source_filename TEXT NOT NULL,source_path TEXT NOT NULL,library_symlink_path TEXT,media_leaf_key TEXT,quality_rank INTEGER NOT NULL DEFAULT 0,state TEXT NOT NULL DEFAULT 'READY',arr_series_id INTEGER,arr_episode_id INTEGER,arr_download_id TEXT,arr_confirmed_at TEXT,created_at TEXT NOT NULL DEFAULT(datetime('now')),updated_at TEXT NOT NULL DEFAULT(datetime('now')),UNIQUE(transfer_id,file_idx),UNIQUE(transfer_id,source_path));
CREATE TABLE transfer_media_ownership(id TEXT PRIMARY KEY,plan_selection_id TEXT NOT NULL REFERENCES plan_selection(id) ON DELETE RESTRICT,transfer_id TEXT NOT NULL REFERENCES provider_transfer(id) ON DELETE RESTRICT,artifact_id TEXT NOT NULL REFERENCES media_artifact(id) ON DELETE RESTRICT,media_leaf_key TEXT NOT NULL,state TEXT NOT NULL CHECK(state IN('PENDING','CURRENT','HISTORICAL','FAILED','RELEASED')),valid_from TEXT NOT NULL DEFAULT(datetime('now')),valid_to TEXT,superseded_by TEXT REFERENCES transfer_media_ownership(id) ON DELETE RESTRICT);
CREATE UNIQUE INDEX ux_ownership_current_leaf ON transfer_media_ownership(media_leaf_key) WHERE state='CURRENT';
CREATE INDEX ix_ownership_transfer ON transfer_media_ownership(transfer_id,state);
CREATE TABLE artifact_lease(id TEXT PRIMARY KEY,transfer_id TEXT NOT NULL REFERENCES provider_transfer(id) ON DELETE RESTRICT,artifact_id TEXT REFERENCES media_artifact(id) ON DELETE RESTRICT,purpose TEXT NOT NULL,holder TEXT NOT NULL,seeding_payload TEXT NOT NULL,acquired_at TEXT NOT NULL DEFAULT(datetime('now')),expires_at TEXT NOT NULL,released_at TEXT,release_reason TEXT);
CREATE UNIQUE INDEX ux_live_artifact_lease ON artifact_lease(transfer_id,purpose) WHERE released_at IS NULL;
CREATE INDEX ix_selection_leaf ON plan_selection(media_leaf_key,status);
CREATE INDEX ix_artifact_leaf ON media_artifact(media_leaf_key,state);
"#;

#[derive(Clone)]
pub struct PlanInput {
    pub id: String,
    pub idempotency_key: String,
    pub media_kind: String,
    pub show_tvdb_id: Option<i64>,
    pub movie_tmdb_id: Option<i64>,
    pub season_number: Option<i64>,
    pub mode: String,
    pub status: String,
    pub quality_policy: String,
    pub language_policy: String,
    pub cache_policy: String,
}
#[derive(Clone)]
pub struct CandidateInput {
    pub id: String,
    pub provider: String,
    pub provider_item_id: String,
    pub manifest_hash: String,
    pub manifest_revision: String,
    pub instance_id: Option<String>,
    pub cache_status: String,
    pub cache_checked_at: Option<String>,
}
#[derive(Clone)]
pub struct SelectionInput {
    pub id: String,
    pub candidate_id: String,
    pub media_leaf_key: String,
    pub selected: bool,
    pub stremio_file_idx: Option<i64>,
    pub provider_file_id: Option<String>,
    pub mapping_confidence: f64,
    pub quality: String,
    pub language: String,
}
#[derive(Clone)]
pub struct TransferInput {
    pub id: String,
    pub provider_kind: String,
    pub provider_id: String,
    pub external_job_id: Option<String>,
    pub instance_id: String,
    pub normalized_info_hash: Option<String>,
    pub state: String,
    pub size_bytes: Option<i64>,
}
#[derive(Debug)]
pub struct TransferClaim {
    pub transfer_id: String,
    pub creator: bool,
}
#[derive(Debug, Serialize)]
pub struct DownloadShow {
    pub tvdb_id: i64,
    pub seasons: Vec<DownloadSeason>,
}
#[derive(Debug, Serialize)]
pub struct DownloadSeason {
    pub season_number: i64,
    pub episodes: Vec<DownloadEpisode>,
}
#[derive(Debug, Serialize)]
pub struct DownloadEpisode {
    pub episode_number: i64,
    pub media_leaf_key: String,
    pub artifact_id: String,
}
#[derive(Debug, Serialize)]
pub struct TransferRoot {
    pub transfer_id: String,
    pub instance_id: String,
    pub normalized_info_hash: Option<String>,
    pub size_bytes: Option<i64>,
    pub owned_media: Vec<String>,
}

impl Db {
    pub(super) fn migrate_acquisition(&self, conn: &mut rusqlite::Connection) -> Result<()> {
        conn.execute_batch("CREATE TABLE IF NOT EXISTS schema_migrations(version INTEGER PRIMARY KEY,applied_at TEXT NOT NULL);")?;
        if conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version=?1)",
            [MIGRATION_VERSION],
            |r| r.get::<_, i64>(0),
        )? != 0
        {
            return Ok(());
        }
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute_batch(MIGRATION_SQL)?;
        tx.execute(
            "INSERT INTO schema_migrations VALUES(?1,datetime('now'))",
            [MIGRATION_VERSION],
        )?;
        tx.commit()
    }
    pub fn save_plan_bundle(
        &self,
        p: &PlanInput,
        cs: &[CandidateInput],
        ss: &[SelectionInput],
    ) -> Result<()> {
        let mut c = self.connection()?;
        let tx = c.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute("INSERT INTO acquisition_plan(id,idempotency_key,media_kind,show_tvdb_id,movie_tmdb_id,season_number,mode,status,quality_policy,language_policy,cache_policy) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11) ON CONFLICT(idempotency_key) DO NOTHING",params![p.id,p.idempotency_key,p.media_kind,p.show_tvdb_id,p.movie_tmdb_id,p.season_number,p.mode,p.status,p.quality_policy,p.language_policy,p.cache_policy])?;
        let plan_id: String = tx.query_row(
            "SELECT id FROM acquisition_plan WHERE idempotency_key=?1",
            [&p.idempotency_key],
            |r| r.get(0),
        )?;
        for x in cs {
            tx.execute("INSERT OR IGNORE INTO source_candidate(id,plan_id,provider,provider_item_id,manifest_hash,manifest_revision,instance_id,cache_status,cache_checked_at) VALUES(?1,?2,?3,?4,lower(?5),?6,?7,?8,?9)",params![x.id,plan_id,x.provider,x.provider_item_id,x.manifest_hash,x.manifest_revision,x.instance_id,x.cache_status,x.cache_checked_at])?;
        }
        for x in ss {
            tx.execute("INSERT OR IGNORE INTO plan_selection(id,plan_id,candidate_id,media_leaf_key,selected,stremio_file_idx,provider_file_id,mapping_confidence,quality,language) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",params![x.id,plan_id,x.candidate_id,x.media_leaf_key,x.selected as i64,x.stremio_file_idx,x.provider_file_id,x.mapping_confidence,x.quality,x.language])?;
        }
        tx.commit()
    }
    pub fn upsert_transfer(&self, x: &TransferInput) -> Result<String> {
        Ok(self.claim_transfer(x)?.transfer_id)
    }
    pub fn claim_transfer(&self, x: &TransferInput) -> Result<TransferClaim> {
        let mut c = self.connection()?;
        let tx = c.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let creator = tx.execute("INSERT INTO provider_transfer(id,provider_kind,provider_id,external_job_id,instance_id,normalized_info_hash,state,size_bytes,create_claimed_at) VALUES(?1,?2,?3,?4,?5,lower(?6),?7,?8,datetime('now')) ON CONFLICT DO NOTHING",params![x.id,x.provider_kind,x.provider_id,x.external_job_id,x.instance_id,x.normalized_info_hash,x.state,x.size_bytes])? == 1;
        let transfer_id = tx.query_row("SELECT id FROM provider_transfer WHERE id=?1 OR (normalized_info_hash IS NOT NULL AND instance_id=?2 AND normalized_info_hash=lower(?3)) OR (external_job_id IS NOT NULL AND provider_kind=?4 AND provider_id=?5 AND external_job_id=?6) LIMIT 1",params![x.id,x.instance_id,x.normalized_info_hash,x.provider_kind,x.provider_id,x.external_job_id],|r|r.get(0))?;
        if !creator {
            tx.execute("UPDATE provider_transfer SET adopted_at=COALESCE(adopted_at,datetime('now')) WHERE id=?1", [&transfer_id])?;
        }
        tx.commit()?;
        Ok(TransferClaim {
            transfer_id,
            creator,
        })
    }
    pub fn record_leaf_failure(&self, plan: &str, selection: &str, message: &str) -> Result<()> {
        let mut c = self.connection()?;
        let tx = c.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "UPDATE plan_selection SET status='FAILED',failure_message=?2 WHERE id=?1",
            params![selection, message],
        )?;
        tx.execute(
            "UPDATE acquisition_plan SET status='PARTIAL' WHERE id=?1",
            [plan],
        )?;
        tx.commit()
    }
    pub fn retry_failed_leaf(&self, selection: &str) -> Result<bool> {
        Ok(self.connection()?.execute("UPDATE plan_selection SET status='SELECTED',failure_message=NULL WHERE id=?1 AND status='FAILED'", [selection])? == 1)
    }
    pub fn plan_status(&self, id: &str) -> Result<Option<String>> {
        self.connection()?
            .query_row(
                "SELECT status FROM acquisition_plan WHERE id=?1",
                [id],
                |r| r.get(0),
            )
            .optional()
    }
    pub fn selection_status(&self, id: &str) -> Result<Option<String>> {
        self.connection()?
            .query_row("SELECT status FROM plan_selection WHERE id=?1", [id], |r| {
                r.get(0)
            })
            .optional()
    }
    fn insert_artifact(
        tx: &rusqlite::Transaction,
        id: &str,
        t: &str,
        idx: Option<i64>,
        name: &str,
        src: &str,
        lib: Option<&str>,
        leaf: Option<&str>,
        rank: i64,
    ) -> Result<()> {
        tx.execute("INSERT INTO media_artifact(id,transfer_id,file_idx,source_filename,source_path,library_symlink_path,media_leaf_key,quality_rank) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",params![id,t,idx,name,src,lib,leaf,rank])?;
        Ok(())
    }
    pub fn add_artifact_and_claim(
        &self,
        id: &str,
        t: &str,
        idx: Option<i64>,
        name: &str,
        src: &str,
        lib: Option<&str>,
        leaf: &str,
        sel: &str,
        rank: i64,
    ) -> Result<()> {
        let mut c = self.connection()?;
        let tx = c.transaction_with_behavior(TransactionBehavior::Immediate)?;
        Self::insert_artifact(&tx, id, t, idx, name, src, lib, Some(leaf), rank)?;
        tx.execute("INSERT INTO transfer_media_ownership(id,plan_selection_id,transfer_id,artifact_id,media_leaf_key,state) VALUES(?1,?2,?3,?4,?5,'CURRENT')",params![Uuid::new_v4().to_string(),sel,t,id,leaf])?;
        tx.commit()
    }
    pub fn add_unresolved_artifact(&self, id: &str, t: &str, name: &str, src: &str) -> Result<()> {
        let mut c = self.connection()?;
        let tx = c.transaction()?;
        Self::insert_artifact(&tx, id, t, None, name, src, None, None, 0)?;
        tx.commit()
    }
    pub fn current_owner_count(&self, leaf: &str) -> Result<i64> {
        self.connection()?.query_row("SELECT count(*) FROM transfer_media_ownership WHERE media_leaf_key=?1 AND state='CURRENT'",[leaf],|r|r.get(0))
    }
    pub fn unresolved_artifact_count(&self) -> Result<i64> {
        self.connection()?.query_row(
            "SELECT count(*) FROM media_artifact WHERE media_leaf_key IS NULL",
            [],
            |r| r.get(0),
        )
    }
    pub fn confirm_artifact_arr(
        &self,
        id: &str,
        series: i64,
        episode: i64,
        download: &str,
    ) -> Result<bool> {
        Ok(self.connection()?.execute("UPDATE media_artifact SET state='CONFIRMED',arr_series_id=?2,arr_episode_id=?3,arr_download_id=?4,arr_confirmed_at=datetime('now'),updated_at=datetime('now') WHERE id=?1",params![id,series,episode,download])?==1)
    }
    pub fn current_artifact(&self, leaf: &str) -> Result<Option<String>> {
        self.connection()?.query_row("SELECT artifact_id FROM transfer_media_ownership WHERE media_leaf_key=?1 AND state='CURRENT'",[leaf],|r|r.get(0)).optional()
    }
    pub fn artifact_state(&self, id: &str) -> Result<Option<String>> {
        self.connection()?
            .query_row("SELECT state FROM media_artifact WHERE id=?1", [id], |r| {
                r.get(0)
            })
            .optional()
    }
    pub fn promote_replacement(
        &self,
        id: &str,
        t: &str,
        idx: Option<i64>,
        name: &str,
        src: &str,
        lib: Option<&str>,
        leaf: &str,
        sel: &str,
        rank: i64,
    ) -> Result<bool> {
        let mut c = self.connection()?;
        let tx = c.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let old:Option<(String,i64)>=tx.query_row("SELECT o.id,a.quality_rank FROM transfer_media_ownership o JOIN media_artifact a ON a.id=o.artifact_id WHERE o.media_leaf_key=?1 AND o.state='CURRENT'",[leaf],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
        if old.as_ref().is_some_and(|x| rank <= x.1) {
            return Ok(false);
        }
        Self::insert_artifact(&tx, id, t, idx, name, src, lib, Some(leaf), rank)?;
        let oid = Uuid::new_v4().to_string();
        tx.execute("INSERT INTO transfer_media_ownership(id,plan_selection_id,transfer_id,artifact_id,media_leaf_key,state) VALUES(?1,?2,?3,?4,?5,'PENDING')",params![oid,sel,t,id,leaf])?;
        if let Some((old, _)) = old {
            tx.execute("UPDATE transfer_media_ownership SET state='HISTORICAL',valid_to=datetime('now'),superseded_by=?1 WHERE id=?2",params![oid,old])?;
        }
        tx.execute(
            "UPDATE transfer_media_ownership SET state='CURRENT' WHERE id=?1",
            [oid],
        )?;
        tx.commit()?;
        Ok(true)
    }
    pub fn acquire_lease(
        &self,
        t: &str,
        purpose: &str,
        holder: &str,
        payload: &str,
        expires: &str,
    ) -> Result<Option<String>> {
        let c = self.connection()?;
        let id = Uuid::new_v4().to_string();
        let n=c.execute("INSERT OR IGNORE INTO artifact_lease(id,transfer_id,purpose,holder,seeding_payload,expires_at) VALUES(?1,?2,?3,?4,?5,?6)",params![id,t,purpose,holder,payload,expires])?;
        Ok((n == 1).then_some(id))
    }
    pub fn release_lease(&self, id: &str, reason: &str) -> Result<bool> {
        Ok(self.connection()?.execute("UPDATE artifact_lease SET released_at=datetime('now'),release_reason=?2 WHERE id=?1 AND released_at IS NULL",params![id,reason])?==1)
    }
    pub fn detach_artifact(&self, id: &str) -> Result<()> {
        let mut c = self.connection()?;
        let tx = c.transaction()?;
        tx.execute("UPDATE transfer_media_ownership SET state='RELEASED',valid_to=datetime('now') WHERE artifact_id=?1 AND state IN('CURRENT','PENDING')",[id])?;
        tx.execute(
            "UPDATE media_artifact SET state='DETACHED',updated_at=datetime('now') WHERE id=?1",
            [id],
        )?;
        tx.commit()
    }
    pub fn transfer_cleanup_allowed(&self, t: &str) -> Result<bool> {
        Ok(self.connection()?.query_row("SELECT NOT EXISTS(SELECT 1 FROM transfer_media_ownership WHERE transfer_id=?1 AND state IN('CURRENT','PENDING')) AND NOT EXISTS(SELECT 1 FROM artifact_lease WHERE transfer_id=?1 AND released_at IS NULL)",[t],|r|r.get::<_,i64>(0))?!=0)
    }
    pub fn project_downloads(&self) -> Result<Vec<DownloadShow>> {
        let c = self.connection()?;
        let mut q=c.prepare("SELECT o.media_leaf_key,o.artifact_id FROM transfer_media_ownership o WHERE o.state='CURRENT' AND o.media_leaf_key LIKE 'episode:%' ORDER BY o.media_leaf_key")?;
        let rows = q.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        let mut m: BTreeMap<(i64, i64), Vec<DownloadEpisode>> = BTreeMap::new();
        for row in rows {
            let (k, a) = row?;
            let p: Vec<_> = k.split(':').collect();
            if p.len() != 4 {
                continue;
            }
            let tv = p[1].parse().unwrap_or(-1);
            let s = p[2].parse().unwrap_or(-1);
            let e = p[3].parse().unwrap_or(-1);
            m.entry((tv, s)).or_default().push(DownloadEpisode {
                episode_number: e,
                media_leaf_key: k,
                artifact_id: a,
            });
        }
        let mut shows: BTreeMap<i64, Vec<DownloadSeason>> = BTreeMap::new();
        for ((tv, s), episodes) in m {
            shows.entry(tv).or_default().push(DownloadSeason {
                season_number: s,
                episodes,
            });
        }
        Ok(shows
            .into_iter()
            .map(|(tvdb_id, seasons)| DownloadShow { tvdb_id, seasons })
            .collect())
    }
    pub fn project_transfers(&self) -> Result<Vec<TransferRoot>> {
        let c = self.connection()?;
        let mut q=c.prepare("SELECT t.id,t.instance_id,t.normalized_info_hash,t.size_bytes,o.media_leaf_key FROM provider_transfer t LEFT JOIN transfer_media_ownership o ON o.transfer_id=t.id AND o.state IN('CURRENT','HISTORICAL') ORDER BY t.id,o.media_leaf_key")?;
        let rows = q.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, Option<i64>>(3)?,
                r.get::<_, Option<String>>(4)?,
            ))
        })?;
        let mut m: BTreeMap<String, TransferRoot> = BTreeMap::new();
        for row in rows {
            let (id, instance_id, hash, size, leaf) = row?;
            let x = m.entry(id.clone()).or_insert(TransferRoot {
                transfer_id: id,
                instance_id,
                normalized_info_hash: hash,
                size_bytes: size,
                owned_media: vec![],
            });
            if let Some(l) = leaf {
                x.owned_media.push(l)
            }
        }
        Ok(m.into_values().collect())
    }
}
