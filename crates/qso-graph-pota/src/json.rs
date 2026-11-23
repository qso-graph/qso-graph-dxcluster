// SPDX-License-Identifier: GPL-3.0-or-later
//! Validate all objects, including those in ignored members (QGDXC-POTA-0002).

use crate::{Error, MAX_ENTRIES};
use serde_core::de::{self, DeserializeSeed, Deserializer, MapAccess, SeqAccess, Visitor};
use serde_json::value::RawValue;
use std::{collections::BTreeMap, fmt};

const DEPTH: &str = "JSON depth limit";
const DUPLICATE: &str = "duplicate JSON member";
const SIZE: &str = "entry limit";

type Object<'a> = BTreeMap<String, &'a RawValue>;
pub(super) type Entry<'a> = Option<Object<'a>>;

pub(super) fn read(bytes: &[u8]) -> Result<Vec<Entry<'_>>, Error> {
    let text = std::str::from_utf8(bytes).map_err(|_| Error::Utf8)?;
    if !text
        .trim_start_matches([' ', '\r', '\t', '\n'])
        .starts_with('[')
    {
        serde_json::from_str::<&RawValue>(text).map_err(|_| Error::Json)?;
        return Err(Error::NotArray);
    }
    let mut decoder = serde_json::Deserializer::from_str(text);
    let entries = Snapshot.deserialize(&mut decoder).map_err(|e| {
        let message = e.to_string();
        if message.starts_with(DEPTH) {
            Error::Depth
        } else if message.starts_with(DUPLICATE) {
            Error::DuplicateMember
        } else if message.starts_with(SIZE) {
            Error::TooManyEntries
        } else {
            Error::Json
        }
    })?;
    decoder.end().map_err(|_| Error::Json)?;
    Ok(entries)
}

struct Snapshot;

impl<'de> DeserializeSeed<'de> for Snapshot {
    type Value = Vec<Entry<'de>>;

    fn deserialize<D: Deserializer<'de>>(self, decoder: D) -> Result<Self::Value, D::Error> {
        decoder.deserialize_seq(self)
    }
}

impl<'de> Visitor<'de> for Snapshot {
    type Value = Vec<Entry<'de>>;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a spot array")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut array: A) -> Result<Self::Value, A::Error> {
        let mut entries = Vec::new();
        while let Some(value) = array.next_element::<&RawValue>()? {
            if entries.len() == MAX_ENTRIES {
                return Err(de::Error::custom(SIZE));
            }
            entries.push(inspect(value, 2).map_err(de::Error::custom)?);
        }
        Ok(entries)
    }
}

fn inspect(value: &RawValue, depth: u8) -> Result<Entry<'_>, serde_json::Error> {
    let mut decoder = serde_json::Deserializer::from_str(value.get());
    match value.get().as_bytes()[0] {
        b'{' => decoder.deserialize_map(Nested(depth)),
        b'[' => decoder.deserialize_seq(Nested(depth)),
        b'"' => serde_json::from_str::<String>(value.get()).map(|_| None),
        _ => Ok(None),
    }
}

struct Nested(u8);

impl<'de> Visitor<'de> for Nested {
    type Value = Entry<'de>;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a JSON container")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut array: A) -> Result<Self::Value, A::Error> {
        if self.0 > 16 {
            return Err(de::Error::custom(DEPTH));
        }
        while let Some(value) = array.next_element::<&RawValue>()? {
            inspect(value, self.0 + 1).map_err(de::Error::custom)?;
        }
        Ok(None)
    }

    fn visit_map<A: MapAccess<'de>>(self, mut object: A) -> Result<Self::Value, A::Error> {
        if self.0 > 16 {
            return Err(de::Error::custom(DEPTH));
        }
        let mut members = BTreeMap::new();
        while let Some(key) = object.next_key::<String>()? {
            if members.contains_key(&key) {
                return Err(de::Error::custom(DUPLICATE));
            }
            let value = object.next_value::<&RawValue>()?;
            inspect(value, self.0 + 1).map_err(de::Error::custom)?;
            members.insert(key, value);
        }
        Ok(Some(members))
    }
}
