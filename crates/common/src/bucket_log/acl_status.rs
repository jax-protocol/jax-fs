use serde::{Deserialize, Serialize};

/// Effective ACL status for a bucket, derived by reducing the ACL event log (last event wins).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BucketAclStatus {
    /// Newly shared with us — not yet approved or ignored
    Pending,
    /// Approved or self-created — sync normally
    Active,
    /// User explicitly doesn't want this bucket
    Ignored,
    /// User voluntarily left the bucket
    Left,
    /// Owner removed our key from the bucket's shares
    Kicked,
}

impl BucketAclStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Active => "active",
            Self::Ignored => "ignored",
            Self::Left => "left",
            Self::Kicked => "kicked",
        }
    }

    /// Whether this status represents a terminal state where syncing should stop.
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Ignored | Self::Left | Self::Kicked)
    }
}

/// Error returned when parsing an unknown ACL status string.
#[derive(Debug, Clone, thiserror::Error)]
#[error("unknown bucket ACL status: {0}")]
pub struct ParseBucketAclStatusError(String);

impl std::str::FromStr for BucketAclStatus {
    type Err = ParseBucketAclStatusError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(Self::Pending),
            "active" => Ok(Self::Active),
            "ignored" => Ok(Self::Ignored),
            "left" => Ok(Self::Left),
            "kicked" => Ok(Self::Kicked),
            _ => Err(ParseBucketAclStatusError(s.to_string())),
        }
    }
}

impl std::fmt::Display for BucketAclStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_roundtrip() {
        for status in [
            BucketAclStatus::Pending,
            BucketAclStatus::Active,
            BucketAclStatus::Ignored,
            BucketAclStatus::Left,
            BucketAclStatus::Kicked,
        ] {
            let s = status.to_string();
            let parsed: BucketAclStatus = s.parse().unwrap();
            assert_eq!(parsed, status);
        }
    }

    #[test]
    fn terminal_states() {
        assert!(!BucketAclStatus::Pending.is_terminal());
        assert!(!BucketAclStatus::Active.is_terminal());
        assert!(BucketAclStatus::Ignored.is_terminal());
        assert!(BucketAclStatus::Left.is_terminal());
        assert!(BucketAclStatus::Kicked.is_terminal());
    }

    #[test]
    fn serde_roundtrip() {
        for status in [
            BucketAclStatus::Pending,
            BucketAclStatus::Active,
            BucketAclStatus::Ignored,
            BucketAclStatus::Left,
            BucketAclStatus::Kicked,
        ] {
            let json = serde_json::to_string(&status).unwrap();
            let parsed: BucketAclStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, status);
        }
    }
}
