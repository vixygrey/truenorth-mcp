//! Ordered JSON codec for the future mutable cockpit format.
//!
//! This module is intentionally not connected to the active YAML-backed cockpit. It
//! defines the JSON data contract and rejects duplicate object keys before a
//! `serde_json::Value` is materialized, because the standard value deserializer keeps
//! only the final occurrence.

use std::fmt;

use serde::Deserialize;
use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Number, Value};
use thiserror::Error;

/// An ordered JSON object suitable for a cockpit document.
pub type CockpitObject = Map<String, Value>;

/// A JSON cockpit document failed to satisfy its format contract.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum CockpitJsonError {
    /// The input is malformed or contains a duplicate object key.
    #[error("invalid cockpit JSON: {detail}")]
    Parse {
        /// The parser diagnostic.
        detail: String,
    },

    /// The parsed JSON document was not an object.
    #[error("cockpit JSON document root must be an object")]
    NonObjectRoot,

    /// The document could not be serialized.
    #[error("could not serialize cockpit JSON: {detail}")]
    Serialize {
        /// The serializer diagnostic.
        detail: String,
    },
}

/// Parse a JSON cockpit document, preserving object insertion order and rejecting
/// duplicate keys at every nesting level before creating the value tree.
pub fn parse_object(input: &str) -> Result<CockpitObject, CockpitJsonError> {
    let mut deserializer = serde_json::Deserializer::from_str(input);
    let value = RejectingValue::deserialize(&mut deserializer)
        .map_err(|error| CockpitJsonError::Parse {
            detail: error.to_string(),
        })?
        .0;
    deserializer
        .end()
        .map_err(|error| CockpitJsonError::Parse {
            detail: error.to_string(),
        })?;

    match value {
        Value::Object(object) => Ok(object),
        _ => Err(CockpitJsonError::NonObjectRoot),
    }
}

/// Serialize an ordered cockpit object as formatted JSON with a trailing newline.
pub fn to_string(object: &CockpitObject) -> Result<String, CockpitJsonError> {
    serde_json::to_string_pretty(object)
        .map(|json| format!("{json}\n"))
        .map_err(|error| CockpitJsonError::Serialize {
            detail: error.to_string(),
        })
}

struct RejectingValue(Value);

impl<'de> Deserialize<'de> for RejectingValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_any(RejectingValueVisitor)
    }
}

struct RejectingValueVisitor;

impl<'de> Visitor<'de> for RejectingValueVisitor {
    type Value = RejectingValue;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a valid JSON value")
    }

    fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(RejectingValue(Value::Bool(value)))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(RejectingValue(Value::Number(Number::from(value))))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(RejectingValue(Value::Number(Number::from(value))))
    }

    fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Number::from_f64(value)
            .map(|number| RejectingValue(Value::Number(number)))
            .ok_or_else(|| E::custom("JSON numbers must be finite"))
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(RejectingValue(Value::String(value.to_string())))
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(RejectingValue(Value::String(value)))
    }

    fn visit_none<E>(self) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(RejectingValue(Value::Null))
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(RejectingValue(Value::Null))
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut values = Vec::new();
        while let Some(value) = sequence.next_element::<RejectingValue>()? {
            values.push(value.0);
        }
        Ok(RejectingValue(Value::Array(values)))
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut object = CockpitObject::new();
        while let Some(key) = map.next_key::<String>()? {
            if object.contains_key(&key) {
                return Err(<A::Error as de::Error>::custom(format!(
                    "duplicate object key `{key}`"
                )));
            }
            let value = map.next_value::<RejectingValue>()?.0;
            object.insert(key, value);
        }
        Ok(RejectingValue(Value::Object(object)))
    }
}

#[cfg(test)]
mod tests {
    use super::{CockpitJsonError, parse_object, to_string};

    #[test]
    fn preserves_unknown_null_nested_values_and_insertion_order() {
        let mut document = parse_object(
            r#"{"z":1,"unknown":{"nested":null,"values":[true,{"extra":"value"}]},"a":2}"#,
        )
        .expect("parse ordered object");
        document.insert("later".to_string(), serde_json::json!(3));

        let keys: Vec<_> = document.keys().map(String::as_str).collect();
        assert_eq!(keys, ["z", "unknown", "a", "later"]);
        assert!(document["unknown"]["nested"].is_null());
        assert_eq!(document["unknown"]["values"][1]["extra"], "value");

        let round_tripped = parse_object(&to_string(&document).expect("serialize ordered object"))
            .expect("reparse ordered object");
        assert_eq!(round_tripped, document);
    }

    #[test]
    fn rejects_duplicate_keys_at_every_object_depth() {
        for input in [
            r#"{"same":1,"same":2}"#,
            r#"{"nested":{"same":1,"same":2}}"#,
            r#"{"items":[{"same":1,"same":2}]}"#,
        ] {
            let error = parse_object(input).expect_err("duplicate keys must fail");
            assert!(matches!(error, CockpitJsonError::Parse { .. }));
            assert!(error.to_string().contains("duplicate object key `same`"));
        }
    }

    #[test]
    fn rejects_malformed_input_non_object_roots_and_out_of_range_numbers() {
        assert!(matches!(
            parse_object("{\"unterminated\": [}"),
            Err(CockpitJsonError::Parse { .. })
        ));
        assert_eq!(parse_object("[]"), Err(CockpitJsonError::NonObjectRoot));
        assert!(matches!(
            parse_object("{\"number\": 1e9999}"),
            Err(CockpitJsonError::Parse { .. })
        ));
    }
}
