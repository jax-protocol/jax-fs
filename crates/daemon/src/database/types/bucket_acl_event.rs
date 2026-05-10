use serde::{Deserialize, Serialize};
use sqlx::encode::IsNull;
use sqlx::error::BoxDynError;
use sqlx::sqlite::{SqliteArgumentValue, SqliteTypeInfo, SqliteValueRef};
use sqlx::{Decode, Encode, Sqlite, Type};

use common::bucket_log::BucketAclStatus;

/// ACL event types recorded in the bucket_acl_log table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BucketAclEvent {
    /// Remote peer shared this bucket with us
    Shared,
    /// User explicitly approved the bucket
    Approved,
    /// User doesn't want this bucket
    Ignored,
    /// User voluntarily left the bucket
    Left,
    /// Our key was removed from the bucket's shares
    Kicked,
}

impl BucketAclEvent {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Shared => "shared",
            Self::Approved => "approved",
            Self::Ignored => "ignored",
            Self::Left => "left",
            Self::Kicked => "kicked",
        }
    }

    /// Map this event to the effective ACL status it produces.
    pub fn to_status(self) -> BucketAclStatus {
        match self {
            Self::Shared => BucketAclStatus::Pending,
            Self::Approved => BucketAclStatus::Active,
            Self::Ignored => BucketAclStatus::Ignored,
            Self::Left => BucketAclStatus::Left,
            Self::Kicked => BucketAclStatus::Kicked,
        }
    }
}

/// Error returned when parsing an unknown ACL event string.
#[derive(Debug, Clone, thiserror::Error)]
#[error("unknown bucket ACL event: {0}")]
pub struct ParseBucketAclEventError(String);

impl std::str::FromStr for BucketAclEvent {
    type Err = ParseBucketAclEventError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "shared" => Ok(Self::Shared),
            "approved" => Ok(Self::Approved),
            "ignored" => Ok(Self::Ignored),
            "left" => Ok(Self::Left),
            "kicked" => Ok(Self::Kicked),
            _ => Err(ParseBucketAclEventError(s.to_string())),
        }
    }
}

impl std::fmt::Display for BucketAclEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl Decode<'_, Sqlite> for BucketAclEvent {
    fn decode(value: SqliteValueRef<'_>) -> Result<Self, BoxDynError> {
        let s = <String as Decode<Sqlite>>::decode(value)?;
        Ok(s.parse()?)
    }
}

impl Encode<'_, Sqlite> for BucketAclEvent {
    fn encode_by_ref(
        &self,
        args: &mut Vec<SqliteArgumentValue<'_>>,
    ) -> Result<IsNull, BoxDynError> {
        args.push(SqliteArgumentValue::Text(self.as_str().into()));
        Ok(IsNull::No)
    }
}

impl Type<Sqlite> for BucketAclEvent {
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
    fn parse_roundtrip() {
        for event in [
            BucketAclEvent::Shared,
            BucketAclEvent::Approved,
            BucketAclEvent::Ignored,
            BucketAclEvent::Left,
            BucketAclEvent::Kicked,
        ] {
            let s = event.to_string();
            let parsed: BucketAclEvent = s.parse().unwrap();
            assert_eq!(parsed, event);
        }
    }

    #[test]
    fn event_to_status_mapping() {
        assert_eq!(BucketAclEvent::Shared.to_status(), BucketAclStatus::Pending);
        assert_eq!(
            BucketAclEvent::Approved.to_status(),
            BucketAclStatus::Active
        );
        assert_eq!(
            BucketAclEvent::Ignored.to_status(),
            BucketAclStatus::Ignored
        );
        assert_eq!(BucketAclEvent::Left.to_status(), BucketAclStatus::Left);
        assert_eq!(BucketAclEvent::Kicked.to_status(), BucketAclStatus::Kicked);
    }

    #[test]
    fn serde_roundtrip() {
        for event in [
            BucketAclEvent::Shared,
            BucketAclEvent::Approved,
            BucketAclEvent::Ignored,
            BucketAclEvent::Left,
            BucketAclEvent::Kicked,
        ] {
            let json = serde_json::to_string(&event).unwrap();
            let parsed: BucketAclEvent = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, event);
        }
    }
}
