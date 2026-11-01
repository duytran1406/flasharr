use async_trait::async_trait;
pub mod restart;
pub mod selection;
#[cfg(not(test))]
pub mod torbox;
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Availability {
    Cached,
    PartiallyCached,
    NotCached,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AvailabilityResult {
    pub info_hash: String,
    pub availability: Availability,
    pub file_ids: Vec<u64>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderAvailability {
    pub availability: Availability,
    pub file_ids: Vec<u64>,
}

impl ProviderAvailability {
    pub fn cached() -> Self {
        Self {
            availability: Availability::Cached,
            file_ids: Vec::new(),
        }
    }
    pub fn partially_cached() -> Self {
        Self {
            availability: Availability::PartiallyCached,
            file_ids: Vec::new(),
        }
    }
    pub fn not_cached() -> Self {
        Self {
            availability: Availability::NotCached,
            file_ids: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AvailabilityError {
    retryable: bool,
    _private_detail: String,
}

impl AvailabilityError {
    pub fn transient(detail: impl Into<String>) -> Self {
        Self {
            retryable: true,
            _private_detail: detail.into(),
        }
    }
    pub fn permanent(detail: impl Into<String>) -> Self {
        Self {
            retryable: false,
            _private_detail: detail.into(),
        }
    }
}

#[async_trait]
pub trait AvailabilityProvider: Send + Sync {
    async fn lookup(
        &self,
        hashes: &[String],
    ) -> Result<HashMap<String, ProviderAvailability>, AvailabilityError>;
}

#[derive(Debug, Clone)]
pub struct CachePolicy {
    pub cached_ttl: Duration,
    pub not_cached_ttl: Duration,
    pub unknown_ttl: Duration,
    pub timeout: Duration,
    pub retries: usize,
    pub batch_size: usize,
    pub breaker_failures: usize,
    pub breaker_cooldown: Duration,
}

struct CacheEntry {
    value: AvailabilityResult,
    expires: Instant,
}
struct State {
    cache: HashMap<String, CacheEntry>,
    failures: usize,
    open_until: Option<Instant>,
}

pub struct AvailabilityService<P> {
    provider: P,
    policy: CachePolicy,
    state: Arc<Mutex<State>>,
}

impl<P: AvailabilityProvider> AvailabilityService<P> {
    pub fn new(provider: P, policy: CachePolicy) -> Self {
        Self {
            provider,
            policy,
            state: Arc::new(Mutex::new(State {
                cache: HashMap::new(),
                failures: 0,
                open_until: None,
            })),
        }
    }

    pub async fn lookup(&self, hashes: &[String]) -> Vec<AvailabilityResult> {
        let hashes = normalize_hashes(hashes);
        let now = Instant::now();
        let mut values = HashMap::new();
        let mut missing = Vec::new();
        {
            let mut state = self.state.lock().unwrap();
            state.cache.retain(|_, entry| entry.expires > now);
            for hash in &hashes {
                match state.cache.get(hash) {
                    Some(entry) => {
                        values.insert(hash.clone(), entry.value.clone());
                    }
                    None => missing.push(hash.clone()),
                }
            }
        }
        for batch in missing.chunks(self.policy.batch_size.max(1)) {
            let fetched = self.fetch_batch(batch).await;
            for value in fetched {
                values.insert(value.info_hash.clone(), value);
            }
        }
        hashes
            .into_iter()
            .filter_map(|hash| values.remove(&hash))
            .collect()
    }

    async fn fetch_batch(&self, hashes: &[String]) -> Vec<AvailabilityResult> {
        if self.breaker_open() {
            return self.unknown(hashes);
        }
        let mut response = None;
        for attempt in 0..=self.policy.retries {
            match tokio::time::timeout(self.policy.timeout, self.provider.lookup(hashes)).await {
                Ok(Ok(found)) => {
                    response = Some(found);
                    self.record_success();
                    break;
                }
                Ok(Err(error)) if error.retryable && attempt < self.policy.retries => continue,
                _ => {
                    self.record_failure();
                    break;
                }
            }
        }
        let results = match response {
            Some(mut found) => hashes
                .iter()
                .map(|hash| {
                    let availability = found.remove(hash).unwrap_or(ProviderAvailability {
                        availability: Availability::Unknown,
                        file_ids: Vec::new(),
                    });
                    AvailabilityResult {
                        info_hash: hash.clone(),
                        availability: availability.availability,
                        file_ids: availability.file_ids,
                        error: None,
                    }
                })
                .collect(),
            None => self.unknown(hashes),
        };
        self.cache(results)
    }

    fn unknown(&self, hashes: &[String]) -> Vec<AvailabilityResult> {
        hashes
            .iter()
            .map(|hash| AvailabilityResult {
                info_hash: hash.clone(),
                availability: Availability::Unknown,
                file_ids: Vec::new(),
                error: Some("debrid provider unavailable".into()),
            })
            .collect()
    }

    fn cache(&self, results: Vec<AvailabilityResult>) -> Vec<AvailabilityResult> {
        let now = Instant::now();
        let mut state = self.state.lock().unwrap();
        for value in &results {
            let ttl = match value.availability {
                Availability::Cached | Availability::PartiallyCached => self.policy.cached_ttl,
                Availability::NotCached => self.policy.not_cached_ttl,
                Availability::Unknown => self.policy.unknown_ttl,
            };
            state.cache.insert(
                value.info_hash.clone(),
                CacheEntry {
                    value: value.clone(),
                    expires: now + ttl,
                },
            );
        }
        results
    }

    fn breaker_open(&self) -> bool {
        self.state
            .lock()
            .unwrap()
            .open_until
            .is_some_and(|until| until > Instant::now())
    }
    fn record_success(&self) {
        let mut state = self.state.lock().unwrap();
        state.failures = 0;
        state.open_until = None;
    }
    fn record_failure(&self) {
        let mut state = self.state.lock().unwrap();
        state.failures += 1;
        if state.failures >= self.policy.breaker_failures.max(1) {
            state.open_until = Some(Instant::now() + self.policy.breaker_cooldown);
        }
    }
}

fn normalize_hashes(hashes: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    hashes
        .iter()
        .map(|hash| hash.trim().to_ascii_lowercase())
        .filter(|hash| {
            hash.len() == 40
                && hash.bytes().all(|b| b.is_ascii_hexdigit())
                && seen.insert(hash.clone())
        })
        .collect()
}
