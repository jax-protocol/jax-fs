use serde::{Deserialize, Serialize};
use sqlx::encode::IsNull;
use sqlx::error::BoxDynError;
use sqlx::sqlite::{SqliteArgumentValue, SqliteTypeInfo, SqliteValueRef};
use sqlx::{Decode, Encode, Sqlite, Type};
use uuid::Uuid;

/// Database-compatible UUID wrapper with sqlx Encode/Decode
#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq, Hash)]
#[serde(transparent)]
pub struct DUuid(Uuid);

impl DUuid {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for DUuid {
    fn default() -> Self {
        Self::new()
    }
}

impl From<DUuid> for Uuid {
    fn from(val: DUuid) -> Self {
        val.0
    }
}

impl From<Uuid> for DUuid {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl std::ops::Deref for DUuid {
    type Target = Uuid;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Decode<'_, Sqlite> for DUuid {
    fn decode(value: SqliteValueRef<'_>) -> Result<Self, BoxDynError> {
        let s = <String as Decode<Sqlite>>::decode(value)?;
        let uuid = Uuid::parse_str(&s)?;
        Ok(Self(uuid))
    }
}

impl Encode<'_, Sqlite> for DUuid {
    fn encode_by_ref(
        &self,
        args: &mut Vec<SqliteArgumentValue<'_>>,
    ) -> Result<IsNull, BoxDynError> {
        args.push(SqliteArgumentValue::Text(self.0.to_string().into()));
        Ok(IsNull::No)
    }
}

impl Type<Sqlite> for DUuid {
    fn compatible(ty: &SqliteTypeInfo) -> bool {
        <String as Type<Sqlite>>::compatible(ty)
    }

    fn type_info() -> SqliteTypeInfo {
        <String as Type<Sqlite>>::type_info()
    }
}

impl std::fmt::Display for DUuid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
