use crate::ByteVecColumnSerde;
use base64::{engine::general_purpose, Engine as _};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_with::SerializeAs;

pub struct Base64;

impl ByteVecColumnSerde for Base64 {
    fn decoded_example() -> Vec<u8> {
        general_purpose::STANDARD.decode(Self::encoded_example()).unwrap()
    }
    fn encoded_example() -> String {
        "YQ==".to_string()
    }
}

impl<T> SerializeAs<T> for Base64
where
    T: AsRef<[u8]>,
{
    fn serialize_as<S>(source: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        general_purpose::STANDARD.encode(source).serialize(serializer)
    }
}

impl<'de> serde_with::DeserializeAs<'de, Vec<u8>> for Base64 {
    fn deserialize_as<D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        general_purpose::STANDARD.decode(s).map_err(serde::de::Error::custom)
    }
}


#[allow(dead_code)]
#[derive(Copy, Clone, Debug, Default)]
pub struct Hex;

impl ByteVecColumnSerde for Hex {
    fn decoded_example() -> Vec<u8> {
        hex::decode(Self::encoded_example()).unwrap()
    }
    fn encoded_example() -> String {
        "61".to_string()
    }
}

impl<T> SerializeAs<T> for Hex
where
    T: AsRef<[u8]>,
{
    fn serialize_as<S>(source: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&hex::encode(source))
    }
}

impl<'de> serde_with::DeserializeAs<'de, Vec<u8>> for Hex {
    fn deserialize_as<D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        hex::decode(s).map_err(serde::de::Error::custom)
    }
}


#[allow(dead_code)]
#[derive(Copy, Clone, Debug, Default)]
pub struct Utf8;

impl Utf8 {
    fn increment_last_scalar(s: &str) -> String {
        if s.is_empty() {
            return "\u{1}".to_string();
        }

        let mut chars: Vec<char> = s.chars().collect();
        let last_cp = chars.pop().unwrap() as u32;
        let mut candidate = last_cp.wrapping_add(1);
        while candidate <= 0x10_FFFF {
            if let Some(c) = std::char::from_u32(candidate) {
                chars.push(c);
                return chars.into_iter().collect();
            }
            candidate = candidate.wrapping_add(1);
        }
        chars.push('\u{1}');
        chars.into_iter().collect()
    }
}

impl ByteVecColumnSerde for Utf8 {
    fn decoded_example() -> Vec<u8> {
        Self::encoded_example().as_bytes().to_vec()
    }
    fn encoded_example() -> String {
        "a".to_string()
    }
    fn next_value(value: &[u8]) -> Vec<u8> {
        let s = match String::from_utf8(value.to_vec()) {
            Ok(s) => s,
            Err(_) => String::from_utf8_lossy(value).into_owned(),
        };
        Self::increment_last_scalar(&s).into_bytes()
    }
}

impl SerializeAs<Vec<u8>> for Utf8 {
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
    use serde::{Deserialize, Serialize};
    use serde_with::serde_as;


    #[serde_as]
    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Utf8Wrap(
        #[serde_as(as = "Utf8")] Vec<u8>
    );


    #[serde_as]
    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Base64Wrap(
        #[serde_as(as = "Base64")] Vec<u8>
    );


    #[serde_as]
    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct HexWrap(
        #[serde_as(as = "Hex")] Vec<u8>
    );


    #[test]
    fn test_utf8_roundtrip() {
        let original = Utf8Wrap("test".as_bytes().to_vec());
        let serialized = serde_json::to_string(&original).unwrap();
        assert_eq!(serialized, "\"test\"");
        let deserialized: Utf8Wrap = serde_json::from_str(&serialized).unwrap();
        assert_eq!(original, deserialized);
    }


    #[test]
    fn test_base64_roundtrip() {
        let original = Base64Wrap("a".as_bytes().to_vec());
        let serialized = serde_json::to_string(&original).unwrap();
        // Base64 of "a" is "YQ=="
        assert_eq!(serialized, "\"YQ==\"");
        let deserialized: Base64Wrap = serde_json::from_str(&serialized).unwrap();
        assert_eq!(original, deserialized);


        // also check the example helpers
        assert_eq!(Base64::decoded_example(), b"a".to_vec());
        assert_eq!(Base64::encoded_example(), "YQ==".to_string());
    }


    #[test]
    fn test_hex_roundtrip() {
        let original = HexWrap("a".as_bytes().to_vec());
        let serialized = serde_json::to_string(&original).unwrap();
        // hex of 'a' (0x61) is "61"
        assert_eq!(serialized, "\"61\"");
        let deserialized: HexWrap = serde_json::from_str(&serialized).unwrap();
        assert_eq!(original, deserialized);


        // also check the example helpers
        assert_eq!(Hex::decoded_example(), b"a".to_vec());
        assert_eq!(Hex::encoded_example(), "61".to_string());
    }
}