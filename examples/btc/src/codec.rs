use bech32::{hrp, segwit};
use redbit::ByteVecColumnSerde;
use serde::{Deserialize, Deserializer, Serializer};
use serde_with::{DeserializeAs, SerializeAs};

#[allow(dead_code)]
pub struct Base58;

impl ByteVecColumnSerde for Base58 {
    fn decoded_example() -> Vec<u8> {
        bs58::decode(Self::encoded_example()).with_check(None).into_vec().unwrap()
    }

    fn encoded_example() -> String {
        "1MNr16FTvjhTAw9GBNxhfirmPt9KzSvgMw".to_string()
    }
}

impl SerializeAs<Vec<u8>> for Base58 {
    #[inline]
    fn serialize_as<S>(source: &Vec<u8>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&bs58::encode(source).with_check().into_string())
    }
}

impl<'de> DeserializeAs<'de, Vec<u8>> for Base58 {
    #[inline]
    fn deserialize_as<D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        bs58::decode(&s)
            .with_check(None)
            .into_vec()
            .map_err(|e| serde::de::Error::custom(format!(
                "Base58 decode error: {} (input: {}) - ensure this is a valid legacy Bitcoin address (Base58Check, 25 bytes including version and checksum)",
                e, s
            )))
    }
}

#[allow(dead_code)]
pub struct Bech32;

impl Bech32 {
    pub fn decoded_example() -> Vec<u8> {
        segwit::decode(&String::from_utf8(Self::encoded_example()).unwrap())
            .map(|(_, _, program)| program)
            .unwrap()
    }

    pub fn encoded_example() -> Vec<u8> {
        "0020d6d75fa3fa70078509fb1edbcc0afb81bbfba392bc1851c725be30cd82c11512".as_bytes().to_vec()
    }}

impl SerializeAs<Vec<u8>> for Bech32 {
    #[inline]
    fn serialize_as<S>(source: &Vec<u8>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let version = match source.len() {
            20 => segwit::VERSION_0,
            32 => segwit::VERSION_1,
            _ => {
                return Err(serde::ser::Error::custom(format!(
                    "Unsupported witness program length: {} (bytes: {:x?}) - expected 20 bytes (P2WPKH) or 32 bytes (P2TR). If you see 25 bytes, it's likely a Base58Check-encoded legacy address payload.",
                    source.len(), source
                )))
            }
        };
        let encoded = segwit::encode(hrp::BC, version, source)
            .map_err(|e| serde::ser::Error::custom(format!(
                "Bech32 encode error: {} (bytes: {:x?}) - check witness program version and length",
                e, source
            )))?;
        serializer.serialize_str(&encoded)
    }
}

impl<'de> DeserializeAs<'de, Vec<u8>> for Bech32 {
    #[inline]
    fn deserialize_as<D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        segwit::decode(&s)
            .map(|(_, _, program)| program)
            .map_err(|e| serde::de::Error::custom(format!(
                "Bech32 decode error: {} (input: {}) - ensure this is a valid Bech32m address with correct HRP and witness version",
                e, s
            )))
    }
}

#[allow(dead_code)]
pub struct BaseOrBech;

impl ByteVecColumnSerde for BaseOrBech {
    fn decoded_example() -> Vec<u8> {
        bs58::decode(Self::encoded_example()).with_check(None).into_vec().unwrap()
    }

    fn encoded_example() -> String {
        "1MNr16FTvjhTAw9GBNxhfirmPt9KzSvgMw".to_string()
    }
}

impl SerializeAs<Vec<u8>> for BaseOrBech {
    #[inline]
    fn serialize_as<S>(source: &Vec<u8>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match source.len() {
            20 => {
                let encoded = segwit::encode(hrp::BC, segwit::VERSION_0, source)
                    .map_err(|e| serde::ser::Error::custom(format!("Bech32 encode error: {}", e)))?;
                serializer.serialize_str(&encoded)
            }
            32 => {
                let encoded = segwit::encode(hrp::BC, segwit::VERSION_1, source)
                    .map_err(|e| serde::ser::Error::custom(format!("Bech32 encode error: {}", e)))?;
                serializer.serialize_str(&encoded)
            }
            21 | 25 if matches!(source.first(), Some(0x00) | Some(0x05)) => {
                serializer.serialize_str(&bs58::encode(source).with_check().into_string())
            }
            _ => serializer.serialize_str(&bs58::encode(source).with_check().into_string()),
        }
    }
}

impl<'de> DeserializeAs<'de, Vec<u8>> for BaseOrBech {
    fn deserialize_as<D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;

        if let Ok((_hrp, _version, program)) = segwit::decode(&s) {
            return Ok(program);
        }

        if let Ok(vec) = bs58::decode(&s).with_check(None).into_vec() {
            return Ok(vec);
        }

        Err(serde::de::Error::custom(format!(
            "Invalid Bitcoin address format: {} - could not decode as Bech32 or Base58Check. Expected formats: P2WPKH/P2TR Bech32 or legacy P2PKH/P2SH Base58Check (25 bytes).",
            s
        )))
    }
}

//
// ----------- Tests -------------
//
#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    use serde_with::serde_as;
    use crate::model::serde_json;

    #[serde_as]
    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Base58Wrap(
        #[serde_as(as = "Base58")] Vec<u8>
    );

    #[serde_as]
    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Bech32Wrap(
        #[serde_as(as = "Bech32")] Vec<u8>
    );

    #[serde_as]
    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct BtcWrap(
        #[serde_as(as = "BaseOrBech")] Vec<u8>
    );

    // Helper to test roundtrip JSON encoding/decoding
    fn roundtrip_json<T>(value: &T) -> T
    where
        T: Serialize + for<'de> Deserialize<'de> + PartialEq + std::fmt::Debug,
    {
        let json = serde_json::to_string(value).unwrap();
        serde_json::from_str(&json).unwrap()
    }

    #[test]
    fn test_p2pkh_roundtrip() {
        // P2PKH: version byte 0x00 + 20-byte hash160(pubkey)
        let mut payload = vec![0x00];
        payload.extend(vec![0x11; 20]);
        let original = Base58Wrap(payload.clone());
        let btc = BtcWrap(payload.clone());

        assert_eq!(roundtrip_json(&original), original);
        assert_eq!(roundtrip_json(&btc), btc);
        assert_eq!(original.0, btc.0);
    }

    #[test]
    fn test_p2sh_roundtrip() {
        // P2SH: version byte 0x05 + 20-byte hash160(script)
        let mut payload = vec![0x05];
        payload.extend(vec![0x22; 20]);
        let original = Base58Wrap(payload.clone());
        let btc = BtcWrap(payload.clone());

        assert_eq!(roundtrip_json(&original), original);
        assert_eq!(roundtrip_json(&btc), btc);
        assert_eq!(original.0, btc.0);
    }

    #[test]
    fn test_p2wpkh_roundtrip() {
        // P2WPKH: witness program length 20 bytes, v0
        let program = vec![0x33; 20];
        let original = Bech32Wrap(program.clone());
        let btc = BtcWrap(program.clone());

        assert_eq!(roundtrip_json(&original), original);
        assert_eq!(roundtrip_json(&btc), btc);
        assert_eq!(original.0, btc.0);
    }

    #[test]
    fn test_p2wsh_roundtrip() {
        // P2WSH: witness program length 32 bytes, v0
        let program = vec![0x44; 32];
        let original = Bech32Wrap(program.clone());
        let btc = BtcWrap(program.clone());

        assert_eq!(roundtrip_json(&original), original);
        assert_eq!(roundtrip_json(&btc), btc);
        assert_eq!(original.0, btc.0);
    }

    #[test]
    fn test_p2tr_roundtrip() {
        // 32-byte Taproot program, will be encoded as v1 automatically
        let program = vec![0x55; 32];
        let original = Bech32Wrap(program.clone());
        let btc = BtcWrap(program.clone());

        assert_eq!(roundtrip_json(&original), original);
        assert_eq!(roundtrip_json(&btc), btc);
        assert_eq!(original.0, btc.0);
    }

}
