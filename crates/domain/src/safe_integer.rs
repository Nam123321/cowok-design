use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::ops::Deref;

#[cfg(feature = "ts")]
use ts_rs::TS;

use crate::error::HarnessError;

pub const MIN_SAFE_INTEGER: i64 = -9_007_199_254_740_991;
pub const MAX_SAFE_INTEGER: i64 = 9_007_199_254_740_991;

/// SafeJsInteger guarantees integer values stay strictly within JavaScript's
/// Number.MIN_SAFE_INTEGER and Number.MAX_SAFE_INTEGER bounds across JSON wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(
    feature = "ts",
    ts(
        export,
        export_to = "../../../packages/shared-types/src/generated/",
        type = "number"
    )
)]
pub struct SafeJsInteger(i64);

impl SafeJsInteger {
    pub const ZERO: Self = Self(0);

    pub fn new(val: i64) -> Result<Self, HarnessError> {
        Self::try_from(val)
    }

    pub fn as_i64(&self) -> i64 {
        self.0
    }
}

impl Deref for SafeJsInteger {
    type Target = i64;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl TryFrom<i64> for SafeJsInteger {
    type Error = HarnessError;

    fn try_from(val: i64) -> Result<Self, Self::Error> {
        if (MIN_SAFE_INTEGER..=MAX_SAFE_INTEGER).contains(&val) {
            Ok(Self(val))
        } else {
            Err(HarnessError::UnsafeInteger { value: val })
        }
    }
}

impl From<i32> for SafeJsInteger {
    fn from(val: i32) -> Self {
        Self(val as i64)
    }
}

impl From<u32> for SafeJsInteger {
    fn from(val: u32) -> Self {
        Self(val as i64)
    }
}

impl From<i16> for SafeJsInteger {
    fn from(val: i16) -> Self {
        Self(val as i64)
    }
}

impl From<u16> for SafeJsInteger {
    fn from(val: u16) -> Self {
        Self(val as i64)
    }
}

impl From<i8> for SafeJsInteger {
    fn from(val: i8) -> Self {
        Self(val as i64)
    }
}

impl From<u8> for SafeJsInteger {
    fn from(val: u8) -> Self {
        Self(val as i64)
    }
}

impl fmt::Display for SafeJsInteger {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Serialize for SafeJsInteger {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_i64(self.0)
    }
}

struct SafeJsIntegerVisitor;

impl<'de> Visitor<'de> for SafeJsIntegerVisitor {
    type Value = SafeJsInteger;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("an integer between -9007199254740991 and 9007199254740991")
    }

    fn visit_i64<E>(self, value: i64) -> Result<SafeJsInteger, E>
    where
        E: de::Error,
    {
        SafeJsInteger::try_from(value).map_err(|_| {
            de::Error::custom(format!(
                "integer {} exceeds JavaScript safe bounds [{}, {}]",
                value, MIN_SAFE_INTEGER, MAX_SAFE_INTEGER
            ))
        })
    }

    fn visit_u64<E>(self, value: u64) -> Result<SafeJsInteger, E>
    where
        E: de::Error,
    {
        if value <= MAX_SAFE_INTEGER as u64 {
            Ok(SafeJsInteger(value as i64))
        } else {
            Err(de::Error::custom(format!(
                "unsigned integer {} exceeds JavaScript safe maximum {}",
                value, MAX_SAFE_INTEGER
            )))
        }
    }
}

impl<'de> Deserialize<'de> for SafeJsInteger {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_i64(SafeJsIntegerVisitor)
    }
}
