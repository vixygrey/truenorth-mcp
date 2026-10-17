//! YAML parsing policy for existing repository documents.
//!
//! The repository has historically used `serde_yaml` semantics. Keep that scalar
//! construction while the cockpit remains YAML-backed, rather than silently changing
//! values such as YAML 1.1 booleans during the safe parser replacement.

/// Deserialize a YAML document with the legacy `serde_yaml` scalar rules.
pub fn from_str<'de, T>(input: &'de str) -> saneyaml::Result<T>
where
    T: serde::Deserialize<'de>,
{
    saneyaml::LoadOptions::legacy_serde_yaml().from_str(input)
}

/// Deserialize UTF-8 YAML bytes with the legacy `serde_yaml` scalar rules.
pub fn from_slice<'de, T>(input: &'de [u8]) -> saneyaml::Result<T>
where
    T: serde::Deserialize<'de>,
{
    saneyaml::LoadOptions::legacy_serde_yaml().from_slice(input)
}

/// Serialize YAML with byte-compatible quoting for legacy readers.
pub fn to_string<T>(value: &T) -> saneyaml::Result<String>
where
    T: serde::Serialize + ?Sized,
{
    saneyaml::to_string_with_options(
        value,
        saneyaml::EmitOptions::structural().with_yaml_1_1_safe_strings(true),
    )
}

#[cfg(test)]
mod tests {
    use super::from_str;

    #[test]
    fn preserves_legacy_boolean_construction() {
        let value: saneyaml::Value = from_str("enabled: yes\n").expect("parse legacy YAML");

        assert_eq!(value["enabled"], saneyaml::Value::Bool(true));
    }
}
