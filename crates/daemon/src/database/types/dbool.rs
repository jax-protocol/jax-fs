use serde::{Deserialize, Serialize};
use sqlx::encode::IsNull;
use sqlx::error::BoxDynError;
use sqlx::sqlite::{SqliteArgumentValue, SqliteTypeInfo, SqliteValueRef};
use sqlx::{Decode, Encode, Sqlite, Type};

/// Database-compatible bool wrapper (SQLite stores as INTEGER)
#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq, Hash, Default)]
#[serde(transparent)]
pub struct DBool(bool);

impl From<DBool> for bool {
    fn from(val: DBool) -> Self {
        val.0
    }
}

impl From<bool> for DBool {
    fn from(b: bool) -> Self {
        Self(b)
    }
}

impl std::ops::Deref for DBool {
    type Target = bool;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Decode<'_, Sqlite> for DBool {
    fn decode(value: SqliteValueRef<'_>) -> Result<Self, BoxDynError> {
        let i = <i64 as Decode<Sqlite>>::decode(value)?;
        Ok(Self(i != 0))
    }
}

impl Encode<'_, Sqlite> for DBool {
    fn encode_by_ref(
        &self,
        args: &mut Vec<SqliteArgumentValue<'_>>,
    ) -> Result<IsNull, BoxDynError> {
        args.push(SqliteArgumentValue::Int(if self.0 { 1 } else { 0 }));
        Ok(IsNull::No)
    }
}

impl Type<Sqlite> for DBool {
    fn compatible(ty: &SqliteTypeInfo) -> bool {
        <i64 as Type<Sqlite>>::compatible(ty)
    }

    fn type_info() -> SqliteTypeInfo {
        <i64 as Type<Sqlite>>::type_info()
    }
}
