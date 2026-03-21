use sqlx::encode::IsNull;
use sqlx::error::BoxDynError;
use sqlx::sqlite::{SqliteArgumentValue, SqliteTypeInfo, SqliteValueRef};
use sqlx::{Decode, Encode, Sqlite, Type};

use common::linked_data::Hash;

/// Database-compatible BLAKE3 hash wrapper with sqlx Encode/Decode.
/// Stored as hex string in SQLite.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct DBlake3(common::linked_data::Hash);

impl From<common::linked_data::Hash> for DBlake3 {
    fn from(hash: common::linked_data::Hash) -> Self {
        Self(hash)
    }
}

impl From<DBlake3> for common::linked_data::Hash {
    fn from(val: DBlake3) -> Self {
        val.0
    }
}

impl std::ops::Deref for DBlake3 {
    type Target = Hash;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Decode<'_, Sqlite> for DBlake3 {
    fn decode(value: SqliteValueRef<'_>) -> Result<Self, BoxDynError> {
        let s = <String as Decode<Sqlite>>::decode(value)?;
        let hash: Hash = s
            .parse()
            .map_err(|e: <Hash as std::str::FromStr>::Err| e.to_string())?;
        Ok(Self(hash))
    }
}

impl Encode<'_, Sqlite> for DBlake3 {
    fn encode_by_ref(
        &self,
        args: &mut Vec<SqliteArgumentValue<'_>>,
    ) -> Result<IsNull, BoxDynError> {
        args.push(SqliteArgumentValue::Text(self.0.to_string().into()));
        Ok(IsNull::No)
    }
}

impl Type<Sqlite> for DBlake3 {
    fn compatible(ty: &SqliteTypeInfo) -> bool {
        <String as Type<Sqlite>>::compatible(ty)
    }

    fn type_info() -> SqliteTypeInfo {
        <String as Type<Sqlite>>::type_info()
    }
}

impl std::fmt::Display for DBlake3 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
