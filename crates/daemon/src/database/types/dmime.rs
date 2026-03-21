use sqlx::encode::IsNull;
use sqlx::error::BoxDynError;
use sqlx::sqlite::{SqliteArgumentValue, SqliteTypeInfo, SqliteValueRef};
use sqlx::{Decode, Encode, Sqlite, Type};

/// Database-compatible MIME type wrapper with sqlx Encode/Decode.
/// Stored as string in SQLite (e.g. "image/jpeg").
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct DMime(mime::Mime);

impl From<mime::Mime> for DMime {
    fn from(m: mime::Mime) -> Self {
        Self(m)
    }
}

impl From<DMime> for mime::Mime {
    fn from(val: DMime) -> Self {
        val.0
    }
}

impl std::ops::Deref for DMime {
    type Target = mime::Mime;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Decode<'_, Sqlite> for DMime {
    fn decode(value: SqliteValueRef<'_>) -> Result<Self, BoxDynError> {
        let s = <String as Decode<Sqlite>>::decode(value)?;
        let m: mime::Mime = s.parse().map_err(|e: mime::FromStrError| e.to_string())?;
        Ok(Self(m))
    }
}

impl Encode<'_, Sqlite> for DMime {
    fn encode_by_ref(
        &self,
        args: &mut Vec<SqliteArgumentValue<'_>>,
    ) -> Result<IsNull, BoxDynError> {
        args.push(SqliteArgumentValue::Text(self.0.to_string().into()));
        Ok(IsNull::No)
    }
}

impl Type<Sqlite> for DMime {
    fn compatible(ty: &SqliteTypeInfo) -> bool {
        <String as Type<Sqlite>>::compatible(ty)
    }

    fn type_info() -> SqliteTypeInfo {
        <String as Type<Sqlite>>::type_info()
    }
}

impl std::fmt::Display for DMime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
