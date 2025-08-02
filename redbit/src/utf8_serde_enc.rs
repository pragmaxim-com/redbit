use serde::{Serializer, Deserialize, Deserializer};
use std::str;

#[allow(dead_code)]
pub struct Utf8;

impl<'a> serde_with::SerializeAs<Vec<u8>> for Utf8 {
    #[inline]
    fn serialize_as<S>(source: &Vec<u8>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match str::from_utf8(source) {
            Ok(s) => serializer.serialize_str(s),
            Err(_) => Err(serde::ser::Error::custom("Bytes cannot be UTF-8 encoded")),
        }
    }
}

impl<'de> serde_with::DeserializeAs<'de, Vec<u8>> for Utf8 {
    #[inline]
    fn deserialize_as<D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(s.into_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_with::serde_as;
    use serde::{Serialize, Deserialize};

    #[serde_as]
    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Utf8Wrap(
        #[serde_as(as = "Utf8")] Vec<u8>
    );

    #[test]
    fn test_utf8_roundtrip() {
        let original = Utf8Wrap("test".as_bytes().to_vec());
        let serialized = serde_json::to_string(&original).unwrap();
        assert_eq!(serialized, "\"test\"");
        let deserialized: Utf8Wrap = serde_json::from_str(&serialized).unwrap();
        assert_eq!(original, deserialized);
    }
}
