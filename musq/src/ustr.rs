use std::borrow::Borrow;
use std::fmt::{self, Debug, Display, Formatter};
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::sync::Arc;

// U meaning micro
// a micro-string is either a reference-counted string or a static string
// this guarantees these are cheap to clone everywhere
#[derive(Clone, Eq)]
pub enum UStr {
    Static(&'static str),
    Shared(Arc<str>),
}

impl Deref for UStr {
    type Target = str;

    fn deref(&self) -> &str {
        match self {
            UStr::Static(s) => s,
            UStr::Shared(s) => s,
        }
    }
}

impl Hash for UStr {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Forward the hash to the string representation of this
        // A derive(Hash) encodes the enum discriminant
        (**self).hash(state);
    }
}

impl Borrow<str> for UStr {
    fn borrow(&self) -> &str {
        self
    }
}

impl PartialEq<UStr> for UStr {
    fn eq(&self, other: &UStr) -> bool {
        (**self).eq(&**other)
    }
}

impl From<&'static str> for UStr {
    fn from(s: &'static str) -> Self {
        UStr::Static(s)
    }
}

impl From<String> for UStr {
    fn from(s: String) -> Self {
        UStr::Shared(s.into())
    }
}

impl Debug for UStr {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.pad(self)
    }
}

impl Display for UStr {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.pad(self)
    }
}

// manual impls because otherwise things get a little screwy with lifetimes

impl<'de> serde::Deserialize<'de> for UStr {
    fn deserialize<D>(deserializer: D) -> Result<Self, <D as serde::Deserializer<'de>>::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(String::deserialize(deserializer)?.into())
    }
}

impl serde::Serialize for UStr {
    fn serialize<S>(
        &self,
        serializer: S,
    ) -> Result<<S as serde::Serializer>::Ok, <S as serde::Serializer>::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self)
    }
}