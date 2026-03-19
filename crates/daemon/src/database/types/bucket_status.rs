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

impl std::str::FromStr for BucketStatus {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "pending" => BucketStatus::Pending,
            "active" => BucketStatus::Active,
            "ignored" => BucketStatus::Ignored,
            _ => BucketStatus::Pending,
        })
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
        Ok(s.parse().unwrap())
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
