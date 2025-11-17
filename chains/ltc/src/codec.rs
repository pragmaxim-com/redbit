use bech32::{Fe32, hrp, segwit};
use bitcoin::WitnessVersion;
use redbit::ByteVecColumnSerde;
use serde::{Deserialize, Deserializer, Serializer};
use serde_with::{DeserializeAs, SerializeAs};

#[allow(dead_code)]
pub struct Base58;

// Note: we purposely avoid hard-coding literal address strings because tests relying on
// ByteVecColumnSerde must have internally consistent examples. Instead, we generate
// valid Base58Check strings from known-good payloads at runtime.

impl ByteVecColumnSerde for Base58 {
    fn decoded_example() -> Vec<u8> {
        // Return the 21-byte legacy payload (ver || hash160) by decoding a valid Base58Check string
        bs58::decode(Self::encoded_example()).with_check(None).into_vec().unwrap()
    }

    fn encoded_example() -> String {
        // Construct a valid Litecoin P2PKH address string from a deterministic payload
        // version 0x30 (L), followed by 20 bytes 0x11
        let mut payload = Vec::with_capacity(21);
        payload.push(0x30);
        payload.extend(std::iter::repeat(0x11u8).take(20));
        bs58::encode(payload).with_check().into_string()
    }

    fn next_value(value: &[u8]) -> Vec<u8> {
        // Increment the last payload byte (keep version byte intact if present)
        let mut v = value.to_vec();
        if v.len() >= 2 {
            let last = v.len() - 1;
            v[last] = v[last].wrapping_add(1);
        } else if v.is_empty() {
            v.push(0x30);
        } else {
            // length == 1, add one payload byte
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
                "Base58 decode error: {} (input: {}) - ensure this is a valid legacy Litecoin address (Base58Check)",
                e, s
            )))
    }
}

#[allow(dead_code)]
pub struct Bech32;

impl Bech32 {
    pub fn decoded_example() -> Vec<u8> {
        segwit::decode(&String::from_utf8(Self::encoded_example()).unwrap()).map(|(_, _, program)| program).unwrap()
    }

    pub fn encoded_example() -> Vec<u8> {
        // v0, random 32B program example
        "0020d6d75fa3fa70078509fb1edbcc0afb81bbfba392bc1851c725be30cd82c11512".as_bytes().to_vec()
    }
}

impl SerializeAs<Vec<u8>> for Bech32 {
    fn serialize_as<S>(source: &Vec<u8>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let version = match source.len() {
            20 => segwit::VERSION_0,
            32 => segwit::VERSION_1,
            _ => {
                return Err(serde::ser::Error::custom(format!(
                    "Unsupported witness program length: {} (bytes: {:x?}) - expected 20 bytes (P2WPKH) or 32 bytes (P2TR)",
                    source.len(),
                    source
                )));
            }
        };
        // Use Litecoin HRP (ltc), this crate does not support network switching at serde-time
        let encoded = segwit::encode(hrp::Hrp::parse_unchecked("ltc"), version, source).map_err(|e| {
            serde::ser::Error::custom(format!("Bech32 encode error: {} (bytes: {:x?})", e, source))
        })?;
        serializer.serialize_str(&encoded)
    }
}

impl<'de> DeserializeAs<'de, Vec<u8>> for Bech32 {
    fn deserialize_as<D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        segwit::decode(&s).map(|(_, _, program)| program).map_err(|e| {
            serde::de::Error::custom(format!(
                "Bech32 decode error: {} (input: {}) - ensure this is a valid Litecoin Bech32 address",
                e, s
            ))
        })
    }
}

#[allow(dead_code)]
pub struct BaseOrBech;

impl ByteVecColumnSerde for BaseOrBech {
    fn decoded_example() -> Vec<u8> {
        // Produce the 21-byte payload from a valid Base58Check Litecoin address string
        bs58::decode(Self::encoded_example()).with_check(None).into_vec().unwrap()
    }

    fn encoded_example() -> String {
        // Construct a valid Litecoin P2PKH address string from deterministic payload
        let mut payload = Vec::with_capacity(21);
        payload.push(0x30); // P2PKH version for Litecoin
        payload.extend(std::iter::repeat(0x22u8).take(20));
        bs58::encode(payload).with_check().into_string()
    }
}

pub const TAG_SEGWIT: u8 = 0xB0;
pub const TAG_OP_RETURN: u8 = 0xF0;
pub const TAG_NON_ADDR: u8 = 0xFF;

fn ascii_eq_icase(a: u8, b: u8) -> bool { a == b || a ^ 0x20 == b }

fn has_prefix_icase(s: &str, p: &[u8]) -> bool {
    let b = s.as_bytes();
    if b.len() < p.len() { return false; }
    for i in 0..p.len() {
        if !ascii_eq_icase(b[i], p[i]) { return false; }
    }
    true
}

fn looks_bech32_addr(s: &str) -> bool {
    // Accept ltc1 / tltc1 (case-insensitive)
    has_prefix_icase(s, b"ltc1") || has_prefix_icase(s, b"tltc1")
}

fn encode_tagged_segwit<S: Serializer>(src: &[u8], ser: S) -> Result<S::Ok, S::Error> {
    // src = [TAG_SEGWIT, ver, program]
    let ver = src[1];
    let prog = &src[2..];
    if ver > 16 { return Err(serde::ser::Error::custom("invalid segwit version")); }
    if prog.len() != 20 && prog.len() != 32 { return Err(serde::ser::Error::custom("invalid segwit program length")); }
    let wv = WitnessVersion::try_from(ver)
        .map_err(|e| serde::ser::Error::custom(format!("WitnessVersion error: {e}")))?;
    // HRP: "ltc"
    let enc = segwit::encode(hrp::Hrp::parse_unchecked("ltc"), Fe32::from(wv), prog)
        .map_err(|e| serde::ser::Error::custom(format!("Bech32 encode error: {e}")))?;
    ser.serialize_str(&enc)
}

fn base58_serialize<S: Serializer>(src: &[u8], ser: S) -> Result<S::Ok, S::Error> {
    // For Litecoin legacy: version 0x30 (P2PKH) or 0x32 (P2SH) + 20B
    match src.first() {
        Some(0x30) | Some(0x32) => ser.serialize_str(&bs58::encode(src).with_check().into_string()),
        Some(v) => Err(serde::ser::Error::custom(format!(
            "21 bytes but unknown Litecoin legacy version 0x{v:02x} (expected 0x30 or 0x32)"
        ))),
        None => unreachable!(),
    }
}

impl SerializeAs<Vec<u8>> for BaseOrBech {
    fn serialize_as<S>(source: &Vec<u8>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if source.len() >= 3 && source[0] == TAG_SEGWIT {
            return encode_tagged_segwit(source, serializer);
        }
        if source.len() == 21 {
            return base58_serialize(source, serializer);
        }
        Err(serde::ser::Error::custom(format!(
            "unsupported address-bytes layout len={} (expected [TAG,ver,program] or [ltc_ver,payload20])",
            source.len()
        )))
    }
}

impl<'de> DeserializeAs<'de, Vec<u8>> for BaseOrBech {
    fn deserialize_as<D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        use core::str::FromStr;
        let s = String::deserialize(deserializer)?;
        if looks_bech32_addr(&s) {
            // Parse via bech32::segwit to support Litecoin HRPs (ltc/tltc)
            let (hrp, ver, prog) = bech32::segwit::decode(&s)
                .map_err(|e| serde::de::Error::custom(format!("Bech32 parse failed: {e}")))?;
            let _ = hrp; // HRP not stored in index bytes
            if prog.len() != 20 && prog.len() != 32 {
                return Err(serde::de::Error::custom("invalid segwit program length"));
            }
            let ver = match ver.to_u8() {
                n if n <= 16 => n as u8,
                _ => return Err(serde::de::Error::custom("invalid segwit version")),
            };
            let mut out = Vec::with_capacity(2 + prog.len());
            out.push(TAG_SEGWIT);
            out.push(ver);
            out.extend_from_slice(&prog);
            return Ok(out);
        }
        if let Ok(vec) = bs58::decode(&s).with_check(None).into_vec() {
            if vec.len() == 21 && matches!(vec.first(), Some(0x30) | Some(0x32)) {
                return Ok(vec);
            }
            return Err(serde::de::Error::custom(format!(
                "Litecoin Base58 payload must be 21 bytes and version 0x30/0x32, got len {}",
                vec.len()
            )));
        }
        Err(serde::de::Error::custom(format!(
            "Invalid Litecoin address: {s} (neither Bech32 nor Base58Check)"
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
    struct Bech32Wrap(#[serde_as(as = "Bech32")] Vec<u8>);

    #[serde_as]
    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct LtcWrap(#[serde_as(as = "BaseOrBech")] Vec<u8>);

    fn roundtrip_json<T>(value: &T) -> T
    where
        T: Serialize + for<'de> Deserialize<'de> + PartialEq + std::fmt::Debug,
    {
        let json = serde_json::to_string(value).unwrap();
        serde_json::from_str(&json).unwrap()
    }

    #[test]
    fn test_p2pkh_roundtrip() {
        // Litecoin P2PKH uses version 0x30
        let mut payload = vec![0x30];
        payload.extend(vec![0x11; 20]);
        let original = Base58Wrap(payload.clone());
        let ltc = LtcWrap(payload.clone());
        assert_eq!(roundtrip_json(&original), original);
        assert_eq!(roundtrip_json(&ltc), ltc);
        assert_eq!(original.0, ltc.0);
    }

    #[test]
    fn test_p2sh_roundtrip() {
        // Litecoin P2SH uses version 0x32
        let mut payload = vec![0x32];
        payload.extend(vec![0x22; 20]);
        let original = Base58Wrap(payload.clone());
        let ltc = LtcWrap(payload.clone());
        assert_eq!(roundtrip_json(&original), original);
        assert_eq!(roundtrip_json(&ltc), ltc);
        assert_eq!(original.0, ltc.0);
    }
}
