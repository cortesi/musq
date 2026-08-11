use std::{
    path::{Path, PathBuf},
    result::Result as StdResult,
};

use crate::{
    SqliteDataType, Value,
    decode::Decode,
    encode::Encode,
    error::{DecodeError, EncodeError},
};

impl Encode for Path {
    fn encode(&self) -> Result<Value, EncodeError> {
        self.to_str()
            .ok_or_else(|| EncodeError::Conversion("path is not valid UTF-8".into()))?
            .encode()
    }
}

impl Encode for PathBuf {
    fn encode(&self) -> Result<Value, EncodeError> {
        self.as_path().encode()
    }
}

impl<'r> Decode<'r> for PathBuf {
    fn decode(value: &'r Value) -> StdResult<Self, DecodeError> {
        compatible!(value, SqliteDataType::Text);
        Ok(Self::from(value.text()?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_round_trip() {
        for expected in [
            PathBuf::new(),
            PathBuf::from("relative/path"),
            PathBuf::from("/absolute/path"),
        ] {
            let encoded = expected.encode().unwrap();
            assert_eq!(PathBuf::decode(&encoded).unwrap(), expected);
            assert_eq!(
                expected.as_path().encode().unwrap().text().unwrap(),
                encoded.text().unwrap()
            );
        }
    }

    #[test]
    fn optional_paths_round_trip() {
        let expected = Some(PathBuf::from("path"));
        let encoded = expected.encode().unwrap();
        assert_eq!(Option::<PathBuf>::decode(&encoded).unwrap(), expected);
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_paths_are_rejected() {
        use std::{ffi::OsString, os::unix::ffi::OsStringExt};

        let path = PathBuf::from(OsString::from_vec(vec![0xff]));
        assert!(path.encode().is_err());
        assert!(path.as_path().encode().is_err());
    }
}
