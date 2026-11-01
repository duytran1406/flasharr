use super::{Availability, AvailabilityError, AvailabilityProvider, ProviderAvailability};
use crate::torbox::client::TorBoxClient;
use async_trait::async_trait;
use std::collections::HashMap;

pub struct TorBoxAvailabilityProvider {
    client: TorBoxClient,
}

impl TorBoxAvailabilityProvider {
    pub fn new(client: TorBoxClient) -> Self {
        Self { client }
    }
}

#[async_trait]
impl AvailabilityProvider for TorBoxAvailabilityProvider {
    async fn lookup(
        &self,
        hashes: &[String],
    ) -> Result<HashMap<String, ProviderAvailability>, AvailabilityError> {
        let hits = self
            .client
            .check_cache(hashes)
            .await
            .map_err(|error| AvailabilityError::transient(error.to_string()))?;
        Ok(hashes
            .iter()
            .map(|hash| {
                let value = match hits.get(hash) {
                    Some(hit) if hit.files.is_empty() => ProviderAvailability {
                        availability: Availability::PartiallyCached,
                        file_ids: Vec::new(),
                    },
                    Some(hit) => ProviderAvailability {
                        availability: Availability::Cached,
                        file_ids: hit.files.iter().map(|file| file.id).collect(),
                    },
                    None => ProviderAvailability::not_cached(),
                };
                (hash.clone(), value)
            })
            .collect())
    }
}
