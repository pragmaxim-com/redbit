use bech32::{hrp, segwit};
use serde::{Deserialize, Deserializer, Serializer};
use serde_with::{DeserializeAs, SerializeAs};

#[allow(dead_code)]
pub struct Base58;

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
        bs58::decode(s)
            .with_check(None)
            .into_vec()
            .map_err(serde::de::Error::custom)
    }
}

//
// Bech32 / Bech32m encoding for Bitcoin SegWit addresses
#[allow(dead_code)]
pub struct Bech32;

impl SerializeAs<Vec<u8>> for Bech32 {
    fn serialize_as<S>(source: &Vec<u8>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let hrp = hrp::BC;

        let version = match source.len() {
            20 => segwit::VERSION_0, // P2WPKH
            32 => {
                // Prefer Taproot for 32-byte program
                segwit::VERSION_1
            }
            _ => {
                return Err(serde::ser::Error::custom(format!(
                    "unsupported witness program length {}",
                    source.len()
                )))
            }
        };

        let encoded = segwit::encode(hrp, version, source)
            .map_err(serde::ser::Error::custom)?;
        serializer.serialize_str(&encoded)
    }
}

impl<'de> DeserializeAs<'de, Vec<u8>> for Bech32 {
    fn deserialize_as<D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let (_hrp, _version, program) = segwit::decode(&s)
            .map_err(serde::de::Error::custom)?;
        Ok(program)
    }
}

#[allow(dead_code)]
pub struct Btc;

impl SerializeAs<Vec<u8>> for Btc {
    fn serialize_as<S>(source: &Vec<u8>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if source.first() == Some(&0x00) || source.first() == Some(&0x05) {
            // Legacy Base58Check
            return serializer.serialize_str(&bs58::encode(source).with_check().into_string());
        }

        // Assume it's a witness program → Bech32 encoding
        let hrp = hrp::BC;

        let version = match source.len() {
            20 => segwit::VERSION_0, // P2WPKH
            32 => {
                // Prefer Taproot (v1) for 32-byte program
                segwit::VERSION_1
            }
            _ => {
                return Err(serde::ser::Error::custom(format!(
                    "unsupported Bitcoin address bytes length {}",
                    source.len()
                )))
            }
        };

        let encoded = segwit::encode(hrp, version, source)
            .map_err(serde::ser::Error::custom)?;
        serializer.serialize_str(&encoded)
    }
}

impl<'de> DeserializeAs<'de, Vec<u8>> for Btc {
    fn deserialize_as<D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;

        // Try Bech32 first
        if let Ok((_hrp, _version, program)) = segwit::decode(&s) {
            return Ok(program);
        }

        // Try Base58Check
        if let Ok(vec) = bs58::decode(&s).with_check(None).into_vec() {
            return Ok(vec);
        }

        Err(serde::de::Error::custom("invalid Bitcoin address format"))
    }
}
/// Helper to create a default byte vector
fn default_bytes() -> Vec<u8> {
    (0..64).map(|i| i as u8).collect()
}

/// Deterministic P2PKH payload (Base58Check, version 0x00)
pub fn base58_p2pkh_payload() -> Vec<u8> {
    let random_bytes = default_bytes();
    let mut payload = vec![0x00];
    payload.extend_from_slice(&random_bytes[..20]); // 20-byte hash160
    payload
}

/// Deterministic P2SH payload (Base58Check, version 0x05)
pub fn base58_p2sh_payload() -> Vec<u8> {
    let random_bytes = default_bytes();
    let mut payload = vec![0x05];
    payload.extend_from_slice(&random_bytes[..20]);
    payload
}

/// Deterministic P2WPKH witness program (Bech32, version 0, 20 bytes)
pub fn bech32_p2wpkh_payload() -> Vec<u8> {
    let random_bytes = default_bytes();
    random_bytes[..20].to_vec()
}

/// Deterministic P2WSH witness program (Bech32, version 0, 32 bytes)
pub fn bech32_p2wsh_payload() -> Vec<u8> {
    let random_bytes = default_bytes();
    random_bytes[..32].to_vec()
}

/// Deterministic Taproot witness program (Bech32m, version 1, 32 bytes)
pub fn bech32_p2tr_payload() -> Vec<u8> {
    let random_bytes = default_bytes();
    random_bytes[..32].to_vec()
}

//
// ----------- Tests -------------
//
#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    use serde_with::serde_as;

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
        #[serde_as(as = "Btc")] Vec<u8>
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
