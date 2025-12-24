//! Entry state types for the MinIO+SQLite blob store.
//!
//! Unlike the FsStore which supports inline data in the database,
//! this store always stores data in MinIO, simplifying the state model.

use serde::{Deserialize, Serialize};

/// State of a blob entry in the store.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntryState {
    /// Blob is complete and available.
    Complete {
        /// Size of the blob in bytes.
        size: u64,
    },
    /// Blob is partially downloaded.
    Partial {
        /// Size of the blob if known (validated from BAO).
        size: Option<u64>,
    },
}

impl EntryState {
    /// Create a new complete entry state.
    pub fn complete(size: u64) -> Self {
        Self::Complete { size }
    }

    /// Create a new partial entry state.
    pub fn partial(size: Option<u64>) -> Self {
        Self::Partial { size }
    }

    /// Check if the entry is complete.
    pub fn is_complete(&self) -> bool {
        matches!(self, Self::Complete { .. })
    }

    /// Check if the entry is partial.
    pub fn is_partial(&self) -> bool {
        matches!(self, Self::Partial { .. })
    }

    /// Get the size if known.
    pub fn size(&self) -> Option<u64> {
        match self {
            Self::Complete { size } => Some(*size),
            Self::Partial { size } => *size,
        }
    }

    /// Get the state as a string for SQLite storage.
    pub fn state_str(&self) -> &'static str {
        match self {
            Self::Complete { .. } => "complete",
            Self::Partial { .. } => "partial",
        }
    }

    /// Parse state from SQLite storage.
    pub fn from_db(state: &str, size: Option<i64>) -> Self {
        match state {
            "complete" => Self::Complete {
                size: size.unwrap_or(0) as u64,
            },
            "partial" => Self::Partial {
                size: size.map(|s| s as u64),
            },
            _ => Self::Partial { size: None },
        }
    }
}

/// Whether a blob needs outboard data for BAO streaming.
///
/// Blobs smaller than or equal to one chunk group (16 KiB) don't need outboard.
pub fn needs_outboard(size: u64) -> bool {
    // 16 KiB is one chunk group in BAO
    const CHUNK_GROUP_SIZE: u64 = 16 * 1024;
    size > CHUNK_GROUP_SIZE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entry_state_complete() {
        let state = EntryState::complete(1024);
        assert!(state.is_complete());
        assert!(!state.is_partial());
        assert_eq!(state.size(), Some(1024));
        assert_eq!(state.state_str(), "complete");
    }

    #[test]
    fn test_entry_state_partial() {
        let state = EntryState::partial(Some(2048));
        assert!(!state.is_complete());
        assert!(state.is_partial());
        assert_eq!(state.size(), Some(2048));
        assert_eq!(state.state_str(), "partial");

        let state_unknown = EntryState::partial(None);
        assert_eq!(state_unknown.size(), None);
    }

    #[test]
    fn test_from_db() {
        let complete = EntryState::from_db("complete", Some(1024));
        assert!(complete.is_complete());
        assert_eq!(complete.size(), Some(1024));

        let partial = EntryState::from_db("partial", Some(2048));
        assert!(partial.is_partial());
        assert_eq!(partial.size(), Some(2048));
    }

    #[test]
    fn test_needs_outboard() {
        assert!(!needs_outboard(0));
        assert!(!needs_outboard(1024));
        assert!(!needs_outboard(16 * 1024)); // Exactly one chunk group
        assert!(needs_outboard(16 * 1024 + 1));
        assert!(needs_outboard(1024 * 1024));
    }
}
