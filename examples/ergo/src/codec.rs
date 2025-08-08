use ergo_lib::ergotree_ir::chain::address::{AddressEncoder, NetworkPrefix};
use redbit::ByteVecColumnSerde;
use serde::{Deserialize, Deserializer, Serializer};
use serde_with::{DeserializeAs, SerializeAs};

#[allow(dead_code)]
pub struct Base58;

impl ByteVecColumnSerde for Base58 {
    fn decoded_example() -> Vec<u8> {
        bs58::decode(Self::encoded_example()).into_vec().unwrap()
    }
    fn encoded_example() -> String {
        "9eYPzx6nogBjex83aiGemfdj579qxD3TPRiPRNHyLZRG8S7rLuQ".to_string()
    }
}

pub const MAINNET: NetworkPrefix = NetworkPrefix::Mainnet;
pub const EMPTY_ADDR_SENTINEL: &str = "EMPTY"; // choose something safe

impl SerializeAs<Vec<u8>> for Base58 {
    #[inline]
    fn serialize_as<S>(source: &Vec<u8>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if source.is_empty() {
            return serializer.serialize_str(EMPTY_ADDR_SENTINEL);
        }

        let address = AddressEncoder::unchecked_parse_address_from_bytes(source)
            .map_err(serde::ser::Error::custom)?;
        let s = AddressEncoder::encode_address_as_string(MAINNET, &address);
        serializer.serialize_str(&s)
    }
}

impl<'de> DeserializeAs<'de, Vec<u8>> for Base58 {
    #[inline]
    fn deserialize_as<D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        if s == EMPTY_ADDR_SENTINEL {
            return Ok(Vec::new());
        }

        let address = AddressEncoder::unchecked_parse_address_from_str(&s)
            .map_err(serde::de::Error::custom)?;
        let bytes = AddressEncoder::encode_address_as_bytes(MAINNET, &address);
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_with::serde_as;
    use serde::{Serialize, Deserialize};
    use crate::model::serde_json;

    #[serde_as]
    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct ErgoWrap(
        #[serde_as(as = "Base58")] Vec<u8>
    );

    fn roundtrip_json<T>(value: &T) -> T
    where
        T: Serialize + for<'de> Deserialize<'de> + PartialEq + std::fmt::Debug,
    {
        let json = serde_json::to_string(value).unwrap();
        serde_json::from_str(&json).unwrap()
    }

    #[test]
    fn test_ergo_base58_roundtrip() {
        // Wrap it and test roundtrip
        let original = ErgoWrap(Base58::decoded_example());
        assert_eq!(roundtrip_json(&original), original);

        // Ensure that serializing returns the same base58 string
        let json = serde_json::to_string(&original).unwrap();
        assert_eq!(json, format!("\"{}\"", Base58::encoded_example()));
    }

}
