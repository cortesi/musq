/// Conversions between `bstr` types and SQL types.
use crate::{
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

impl Type for BString
where
    [u8]: Type,
{
    fn type_info() -> sqlite::SqliteDataType {
        <&[u8] as Type>::type_info()
    }

    fn compatible(ty: &sqlite::SqliteDataType) -> bool {
        <&[u8] as Type>::compatible(ty)
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

impl<'q> Encode<'q> for &'q BStr
where
    &'q [u8]: Encode<'q>,
{
    fn encode_by_ref(&self, buf: &mut ArgumentBuffer<'q>) -> IsNull {
        <&[u8] as Encode>::encode(self.as_bytes(), buf)
    }
}

impl<'q> Encode<'q> for BString
where
    Vec<u8>: Encode<'q>,
{
    fn encode_by_ref(&self, buf: &mut ArgumentBuffer<'q>) -> IsNull {
        <Vec<u8> as Encode>::encode(self.as_bytes().to_vec(), buf)
    }
}
