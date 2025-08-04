use serde::{Serializer, Deserialize, Deserializer};
use crate::ByteVecColumnSerde;

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

impl<T> serde_with::SerializeAs<T> for Hex
where
    T: AsRef<[u8]>,
{
    #[inline]
    fn serialize_as<S>(source: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&hex::encode(source))
    }
}

impl<'de> serde_with::DeserializeAs<'de, Vec<u8>> for Hex {
    #[inline]
    fn deserialize_as<D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        hex::decode(s).map_err(serde::de::Error::custom)
    }
}