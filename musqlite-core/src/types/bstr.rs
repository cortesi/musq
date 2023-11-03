/// Conversions between `bstr` types and SQL types.
use crate::{
    database::Database,
    decode::Decode,
    encode::{Encode, IsNull},
    error::BoxDynError,
    sqlite,
    sqlite::ArgumentBuffer,
    types::Type,
    ValueRef,
};

#[doc(no_inline)]
pub use bstr::{BStr, BString, ByteSlice};

impl<DB> Type<DB> for BString
where
    DB: Database,
    [u8]: Type<DB>,
{
    fn type_info() -> sqlite::TypeInfo {
        <&[u8] as Type<DB>>::type_info()
    }

    fn compatible(ty: &sqlite::TypeInfo) -> bool {
        <&[u8] as Type<DB>>::compatible(ty)
    }
}

impl<'r> Decode<'r> for BString
where
    Vec<u8>: Decode<'r>,
{
    fn decode(value: ValueRef<'r>) -> Result<Self, BoxDynError> {
        <Vec<u8> as Decode>::decode(value).map(BString::from)
    }
}

impl<'q, DB: Database> Encode<'q, DB> for &'q BStr
where
    DB: Database,
    &'q [u8]: Encode<'q, DB>,
{
    fn encode_by_ref(&self, buf: &mut ArgumentBuffer<'q>) -> IsNull {
        <&[u8] as Encode<DB>>::encode(self.as_bytes(), buf)
    }
}

impl<'q, DB: Database> Encode<'q, DB> for BString
where
    DB: Database,
    Vec<u8>: Encode<'q, DB>,
{
    fn encode_by_ref(&self, buf: &mut ArgumentBuffer<'q>) -> IsNull {
        <Vec<u8> as Encode<DB>>::encode(self.as_bytes().to_vec(), buf)
    }
}
