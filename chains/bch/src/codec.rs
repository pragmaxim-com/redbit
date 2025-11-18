use redbit::ByteVecColumnSerde;
use serde::{Deserialize, Deserializer, Serializer};
use serde_with::{DeserializeAs, SerializeAs};

// For Bitcoin Cash we primarily target legacy Base58Check (P2PKH/P2SH).
// CashAddr is not implemented here; unknown formats deserialize as an error.

#[allow(dead_code)]
pub struct Base58;

impl ByteVecColumnSerde for Base58 {
    fn decoded_example() -> Vec<u8> {
        bs58::decode(Self::encoded_example()).with_check(None).into_vec().unwrap()
    }

    fn encoded_example() -> String {
        // Deterministic valid Base58Check: BCH P2PKH version 0x00 + 20B payload
        let mut payload = Vec::with_capacity(21);
        payload.push(0x00);
        payload.extend(std::iter::repeat(0x11u8).take(20));
        bs58::encode(payload).with_check().into_string()
    }

    fn next_value(value: &[u8]) -> Vec<u8> {
        let mut v = value.to_vec();
        if v.len() >= 2 {
            let last = v.len() - 1;
            v[last] = v[last].wrapping_add(1);
        } else if v.is_empty() {
            v.push(0x00);
        } else {
            v.push(0x01);
        }
        v
    }
}

impl SerializeAs<Vec<u8>> for Base58 {
    fn serialize_as<S>(source: &Vec<u8>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&bs58::encode(source).with_check().into_string())
    }
}

impl<'de> DeserializeAs<'de, Vec<u8>> for Base58 {
    fn deserialize_as<D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        bs58::decode(&s)
            .with_check(None)
            .into_vec()
            .map_err(|e| serde::de::Error::custom(format!(
                "Base58 decode error: {} (input: {}) - expected valid Bitcoin Cash legacy address (Base58Check)",
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
        // Deterministic valid Base58Check: BCH P2PKH version 0x00 + 20B payload
        let mut payload = Vec::with_capacity(21);
        payload.push(0x00);
        payload.extend(std::iter::repeat(0x22u8).take(20));
        bs58::encode(payload).with_check().into_string()
    }
}

pub const TAG_OP_RETURN: u8 = 0xF0;
pub const TAG_NON_ADDR: u8 = 0xFF;

fn base58_serialize<S: Serializer>(src: &[u8], ser: S) -> Result<S::Ok, S::Error> {
    // For BCH legacy: version 0x00 (P2PKH) or 0x05 (P2SH) + 20B
    match src.first() {
        Some(0x00) | Some(0x05) => ser.serialize_str(&bs58::encode(src).with_check().into_string()),
        Some(v) => Err(serde::ser::Error::custom(format!(
            "21 bytes but unknown BCH legacy version 0x{v:02x} (expected 0x00 or 0x05)"
        ))),
        None => unreachable!(),
    }
}

impl SerializeAs<Vec<u8>> for BaseOrBech {
    fn serialize_as<S>(source: &Vec<u8>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Legacy Base58Check: [ver(0x00|0x05) || 20B] == 21 bytes
        if source.len() == 21 {
            return base58_serialize(source, serializer);
        }
        // We do not support CashAddr here; unknown layouts rejected
        Err(serde::ser::Error::custom(format!(
            "unsupported address-bytes layout len={} (expected [ver,payload20] for BCH legacy)",
            source.len()
        )))
    }
}

impl<'de> DeserializeAs<'de, Vec<u8>> for BaseOrBech {
    fn deserialize_as<D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        if let Ok(vec) = bs58::decode(&s).with_check(None).into_vec() {
            if vec.len() == 21 && matches!(vec.first(), Some(0x00) | Some(0x05)) {
                return Ok(vec);
            }
            return Err(serde::de::Error::custom(format!(
                "BCH Base58 payload must be 21 bytes and version 0x00/0x05, got len {}",
                vec.len()
            )));
        }
        Err(serde::de::Error::custom(format!(
            "Invalid Bitcoin Cash address: {s} (CashAddr not supported here; use legacy Base58)"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_v1::serde_json;
    use serde::{Deserialize, Serialize};
    use serde_with::serde_as;

    #[serde_as]
    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Base58Wrap(#[serde_as(as = "Base58")] Vec<u8>);

    #[serde_as]
    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct BchWrap(#[serde_as(as = "BaseOrBech")] Vec<u8>);

    fn roundtrip_json<T>(value: &T) -> T
    where
        T: Serialize + for<'de> Deserialize<'de> + PartialEq + std::fmt::Debug,
    {
        let json = serde_json::to_string(value).unwrap();
        serde_json::from_str(&json).unwrap()
    }

    #[test]
    fn test_p2pkh_roundtrip() {
        // BCH P2PKH uses version 0x00
        let mut payload = vec![0x00];
        payload.extend(vec![0x11; 20]);
        let original = Base58Wrap(payload.clone());
        let bch = BchWrap(payload.clone());
        assert_eq!(roundtrip_json(&original), original);
        assert_eq!(roundtrip_json(&bch), bch);
        assert_eq!(original.0, bch.0);
    }

    #[test]
    fn test_p2sh_roundtrip() {
        // BCH P2SH uses version 0x05
        let mut payload = vec![0x05];
        payload.extend(vec![0x22; 20]);
        let original = Base58Wrap(payload.clone());
        let bch = BchWrap(payload.clone());
        assert_eq!(roundtrip_json(&original), original);
        assert_eq!(roundtrip_json(&bch), bch);
        assert_eq!(original.0, bch.0);
    }
}
