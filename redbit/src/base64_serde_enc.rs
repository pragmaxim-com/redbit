use base64::{engine::general_purpose, Engine as _};
use serde::{Serialize, Serializer, Deserialize, Deserializer};
use serde_with::SerializeAs;
use crate::ByteVecColumnSerde;

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