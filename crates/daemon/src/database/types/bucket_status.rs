use serde::{Deserialize, Serialize};
use sqlx::encode::IsNull;
use sqlx::error::BoxDynError;
use sqlx::sqlite::{SqliteArgumentValue, SqliteTypeInfo, SqliteValueRef};
use sqlx::{Decode, Encode, Sqlite, Type};

/// Bucket sync status enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BucketStatus {
    /// Newly shared with us — accept manifest/log but skip blob downloads
    Pending,
    /// Approved — sync normally
    Active,
    /// Rejected or removed — do not sync
    Ignored,
}

impl BucketStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            BucketStatus::Pending => "pending",
            BucketStatus::Active => "active",
            BucketStatus::Ignored => "ignored",
        }
    }
}

/// Error returned when parsing an unknown bucket status string.
#[derive(Debug, Clone, thiserror::Error)]
#[error("unknown bucket status: {0}")]
pub struct ParseBucketStatusError(String);

impl std::str::FromStr for BucketStatus {
    type Err = ParseBucketStatusError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(BucketStatus::Pending),
            "active" => Ok(BucketStatus::Active),
            "ignored" => Ok(BucketStatus::Ignored),
            _ => Err(ParseBucketStatusError(s.to_string())),
        }
    }
}

impl std::fmt::Display for BucketStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl Decode<'_, Sqlite> for BucketStatus {
    fn decode(value: SqliteValueRef<'_>) -> Result<Self, BoxDynError> {
        let s = <String as Decode<Sqlite>>::decode(value)?;
        Ok(s.parse()?)
    }
}

impl Encode<'_, Sqlite> for BucketStatus {
    fn encode_by_ref(
        &self,
        args: &mut Vec<SqliteArgumentValue<'_>>,
    ) -> Result<IsNull, BoxDynError> {
        args.push(SqliteArgumentValue::Text(self.as_str().into()));
        Ok(IsNull::No)
    }
}

impl Type<Sqlite> for BucketStatus {
    fn compatible(ty: &SqliteTypeInfo) -> bool {
        <String as Type<Sqlite>>::compatible(ty)
    }

    fn type_info() -> SqliteTypeInfo {
        <String as Type<Sqlite>>::type_info()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_statuses() {
        assert_eq!(
            "pending".parse::<BucketStatus>().unwrap(),
            BucketStatus::Pending
        );
        assert_eq!(
            "active".parse::<BucketStatus>().unwrap(),
            BucketStatus::Active
        );
        assert_eq!(
            "ignored".parse::<BucketStatus>().unwrap(),
            BucketStatus::Ignored
        );
    }

    #[test]
    fn parse_unknown_status_returns_error() {
        assert!("unknown".parse::<BucketStatus>().is_err());
        assert!("".parse::<BucketStatus>().is_err());
        assert!("ACTIVE".parse::<BucketStatus>().is_err());
    }

    #[test]
    fn display_roundtrip() {
        for status in [
            BucketStatus::Pending,
            BucketStatus::Active,
            BucketStatus::Ignored,
        ] {
            let s = status.to_string();
            let parsed: BucketStatus = s.parse().unwrap();
            assert_eq!(parsed, status);
        }
    }

    #[test]
    fn as_str_matches_display() {
        for status in [
            BucketStatus::Pending,
            BucketStatus::Active,
            BucketStatus::Ignored,
        ] {
            assert_eq!(status.as_str(), status.to_string());
        }
    }

    #[test]
    fn serde_roundtrip() {
        for status in [
            BucketStatus::Pending,
            BucketStatus::Active,
            BucketStatus::Ignored,
        ] {
            let json = serde_json::to_string(&status).unwrap();
            let parsed: BucketStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, status);
        }
    }
}
