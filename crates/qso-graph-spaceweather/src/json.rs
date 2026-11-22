// SPDX-License-Identifier: GPL-3.0-or-later
//! JSON validation with bounded depth and unique keys (QGDXC-SWX-0002).

use crate::Reason;
use serde_core::de::{self, DeserializeSeed, Deserializer, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Value};
use std::fmt;

const DEPTH: &str = "JSON depth limit";
const DUPLICATE: &str = "duplicate JSON member";

pub(super) fn read(bytes: &[u8]) -> Result<Value, Reason> {
    let text = std::str::from_utf8(bytes).map_err(|_| Reason::Utf8)?;
    let mut decoder = serde_json::Deserializer::from_str(text);
    let value = Json(1).deserialize(&mut decoder).map_err(|error| {
        let message = error.to_string();
        if message.starts_with(DEPTH) {
            Reason::Depth
        } else if message.starts_with(DUPLICATE) {
            Reason::DuplicateMember
        } else {
            Reason::Json
        }
    })?;
    decoder.end().map_err(|_| Reason::Json)?;
    Ok(value)
}

struct Json(u8);

impl<'de> DeserializeSeed<'de> for Json {
    type Value = Value;

    fn deserialize<D: Deserializer<'de>>(self, decoder: D) -> Result<Value, D::Error> {
        decoder.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for Json {
    type Value = Value;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a JSON value")
    }

    fn visit_unit<E>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_bool<E>(self, value: bool) -> Result<Value, E> {
        Ok(Value::Bool(value))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Value, E> {
        Ok(value.into())
    }

    fn visit_i64<E>(self, value: i64) -> Result<Value, E> {
        Ok(value.into())
    }

    fn visit_f64<E>(self, value: f64) -> Result<Value, E> {
        Ok(value.into())
    }

    fn visit_str<E>(self, value: &str) -> Result<Value, E> {
        Ok(value.into())
    }

    fn visit_string<E>(self, value: String) -> Result<Value, E> {
        Ok(value.into())
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut array: A) -> Result<Value, A::Error> {
        if self.0 > 16 {
            return Err(de::Error::custom(DEPTH));
        }
        let mut items = Vec::new();
        while let Some(item) = array.next_element_seed(Self(self.0 + 1))? {
            items.push(item);
        }
        Ok(Value::Array(items))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut object: A) -> Result<Value, A::Error> {
        if self.0 > 16 {
            return Err(de::Error::custom(DEPTH));
        }
        let mut entries = Map::new();
        while let Some(key) = object.next_key::<String>()? {
            if entries.contains_key(&key) {
                return Err(de::Error::custom(DUPLICATE));
            }
            entries.insert(key, object.next_value_seed(Self(self.0 + 1))?);
        }
        Ok(Value::Object(entries))
    }
}
