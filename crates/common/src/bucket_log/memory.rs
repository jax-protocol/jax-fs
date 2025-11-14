use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use uuid::Uuid;

use super::provider::{BucketLogError, BucketLogProvider};
use crate::linked_data::Link;

/// In-memory bucket log provider using HashMaps
#[derive(Debug, Clone)]
pub struct MemoryBucketLogProvider {
    inner: Arc<RwLock<MemoryBucketLogProviderInner>>,
}

#[derive(Debug, Default)]
struct MemoryBucketLogProviderInner {
    /// Store log entries: bucket_id -> height -> Vec<Link>
    /// Multiple links at same height represent forks
    entries: HashMap<Uuid, HashMap<u64, Vec<Link>>>,
    /// Track the maximum height for each bucket
    max_heights: HashMap<Uuid, u64>,
    /// Index for quick lookup: bucket_id -> link -> Vec<heights>
    link_index: HashMap<Uuid, HashMap<Link, Vec<u64>>>,
    /// Store bucket names (optional, for caching)
    names: HashMap<Uuid, String>,
}

#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
pub enum MemoryBucketLogProviderError {
    #[error("memory provider error: {0}")]
    Internal(String),
}

impl MemoryBucketLogProvider {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(MemoryBucketLogProviderInner::default())),
        }
    }
}

impl Default for MemoryBucketLogProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl BucketLogProvider for MemoryBucketLogProvider {
    type Error = MemoryBucketLogProviderError;

    async fn exists(&self, id: Uuid) -> Result<bool, BucketLogError<Self::Error>> {
        let inner = self.inner.read().map_err(|e| {
            BucketLogError::Provider(MemoryBucketLogProviderError::Internal(format!(
                "failed to acquire read lock: {}",
                e
            )))
        })?;

        Ok(inner.entries.contains_key(&id))
    }

    async fn heads(&self, id: Uuid, height: u64) -> Result<Vec<Link>, BucketLogError<Self::Error>> {
        let inner = self.inner.read().map_err(|e| {
            BucketLogError::Provider(MemoryBucketLogProviderError::Internal(format!(
                "failed to acquire read lock: {}",
                e
            )))
        })?;

        Ok(inner
            .entries
            .get(&id)
            .and_then(|heights| heights.get(&height))
            .cloned()
            .unwrap_or_default())
    }

    async fn append(
        &self,
        id: Uuid,
        name: String,
        current: Link,
        previous: Option<Link>,
        height: u64,
    ) -> Result<(), BucketLogError<Self::Error>> {
        let mut inner = self.inner.write().map_err(|e| {
            BucketLogError::Provider(MemoryBucketLogProviderError::Internal(format!(
                "failed to acquire write lock: {}",
                e
            )))
        })?;

        // Update bucket name
        inner.names.insert(id, name);

        // Get or create bucket entries
        let bucket_entries = inner.entries.entry(id).or_insert_with(HashMap::new);

        // Check for conflict: same link at same height already exists
        if let Some(existing_links) = bucket_entries.get(&height) {
            if existing_links.contains(&current) {
                return Err(BucketLogError::Conflict);
            }
        }

        // Validate the append based on previous link
        if let Some(prev_link) = &previous {
            // If there's a previous link, it should exist at height - 1
            if height == 0 {
                return Err(BucketLogError::InvalidAppend(
                    current,
                    prev_link.clone(),
                    height,
                ));
            }

            let expected_prev_height = height - 1;
            let prev_exists = bucket_entries
                .get(&expected_prev_height)
                .map(|links| links.contains(prev_link))
                .unwrap_or(false);

            if !prev_exists {
                return Err(BucketLogError::InvalidAppend(
                    current,
                    prev_link.clone(),
                    expected_prev_height,
                ));
            }
        } else {
            // If no previous link, this must be genesis (height 0)
            if height != 0 {
                // For non-genesis entries, we need a previous link
                return Err(BucketLogError::InvalidAppend(
                    current,
                    Link::default(), // placeholder for error
                    height,
                ));
            }
        }

        // Add the entry
        bucket_entries
            .entry(height)
            .or_insert_with(Vec::new)
            .push(current.clone());

        // Update max height
        let current_max = inner.max_heights.get(&id).copied();
        if current_max.is_none() || height > current_max.unwrap() {
            inner.max_heights.insert(id, height);
        }

        // Update link index
        inner
            .link_index
            .entry(id)
            .or_insert_with(HashMap::new)
            .entry(current)
            .or_insert_with(Vec::new)
            .push(height);

        Ok(())
    }

    async fn height(&self, id: Uuid) -> Result<u64, BucketLogError<Self::Error>> {
        let inner = self.inner.read().map_err(|e| {
            BucketLogError::Provider(MemoryBucketLogProviderError::Internal(format!(
                "failed to acquire read lock: {}",
                e
            )))
        })?;

        inner
            .max_heights
            .get(&id)
            .copied()
            .ok_or(BucketLogError::HeadNotFound(0))
    }

    async fn has(&self, id: Uuid, link: Link) -> Result<Vec<u64>, BucketLogError<Self::Error>> {
        let inner = self.inner.read().map_err(|e| {
            BucketLogError::Provider(MemoryBucketLogProviderError::Internal(format!(
                "failed to acquire read lock: {}",
                e
            )))
        })?;

        Ok(inner
            .link_index
            .get(&id)
            .and_then(|links| links.get(&link))
            .cloned()
            .unwrap_or_default())
    }

    async fn list_buckets(&self) -> Result<Vec<Uuid>, BucketLogError<Self::Error>> {
        let inner = self.inner.read().map_err(|e| {
            BucketLogError::Provider(MemoryBucketLogProviderError::Internal(format!(
                "failed to acquire read lock: {}",
                e
            )))
        })?;

        Ok(inner.entries.keys().copied().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroh_blobs::Hash;

    #[tokio::test]
    async fn test_genesis_append() {
        let provider = MemoryBucketLogProvider::new();
        let id = Uuid::new_v4();
        let link = Link::new(0x55, Hash::from_bytes([1; 32]));

        // Genesis append should succeed
        let result = provider
            .append(id, "test".to_string(), link.clone(), None, 0)
            .await;
        assert!(result.is_ok());

        // Check height
        let height = provider.height(id).await.unwrap();
        assert_eq!(height, 0);

        // Check heads
        let heads = provider.heads(id, 0).await.unwrap();
        assert_eq!(heads, vec![link]);
    }

    #[tokio::test]
    async fn test_conflict() {
        let provider = MemoryBucketLogProvider::new();
        let id = Uuid::new_v4();
        let link = Link::new(0x55, Hash::from_bytes([1; 32]));

        // First append succeeds
        provider
            .append(id, "test".to_string(), link.clone(), None, 0)
            .await
            .unwrap();

        // Same link at same height should conflict
        let result = provider.append(id, "test".to_string(), link, None, 0).await;
        assert!(matches!(result, Err(BucketLogError::Conflict)));
    }

    #[tokio::test]
    async fn test_invalid_append() {
        let provider = MemoryBucketLogProvider::new();
        let id = Uuid::new_v4();
        let link1 = Link::new(0x55, Hash::from_bytes([1; 32]));
        let link2 = Link::new(0x55, Hash::from_bytes([2; 32]));

        // Genesis
        provider
            .append(id, "test".to_string(), link1, None, 0)
            .await
            .unwrap();

        // Append with non-existent previous should fail
        let result = provider
            .append(id, "test".to_string(), link2.clone(), Some(link2), 1)
            .await;
        assert!(matches!(
            result,
            Err(BucketLogError::InvalidAppend(_, _, _))
        ));
    }

    #[tokio::test]
    async fn test_valid_chain() {
        let provider = MemoryBucketLogProvider::new();
        let id = Uuid::new_v4();
        let link1 = Link::new(0x55, Hash::from_bytes([1; 32]));
        let link2 = Link::new(0x55, Hash::from_bytes([2; 32]));

        // Genesis
        provider
            .append(id, "test".to_string(), link1.clone(), None, 0)
            .await
            .unwrap();

        // Valid append
        provider
            .append(id, "test".to_string(), link2.clone(), Some(link1), 1)
            .await
            .unwrap();

        // Check height
        let height = provider.height(id).await.unwrap();
        assert_eq!(height, 1);

        // Check has
        let heights = provider.has(id, link2).await.unwrap();
        assert_eq!(heights, vec![1]);
    }
}
