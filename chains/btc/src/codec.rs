use bech32::{Fe32, hrp, segwit};
use bitcoin::WitnessVersion;
use redbit::ByteVecColumnSerde;
use serde::{Deserialize, Deserializer, Serializer};
use serde_with::{DeserializeAs, SerializeAs};

#[allow(dead_code)]
pub struct Base58;

const ADDRESSES: &[&str] = &[
    "bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq",
    "1MNr16FTvjhTAw9GBNxhfirmPt9KzSvgMw",
    "3J98t1WpEZ73CNmQviecrnyiWrnqRhWNLy",
    "1BvBMSEYstWetqTFn5Au4m4GFg7xJaNVN2",
    "sp1qqffj92fjdv6yjspqhlm06e9p3r59zd3sghuwrqg2w8vu3v349pg5sq60g7xquly89u0a54r9sayzhjcpqcgeqa8qqkzuukp6c7c5wfhgscujd5rs",
];

impl ByteVecColumnSerde for Base58 {
    fn decoded_example() -> Vec<u8> {
        bs58::decode(Self::encoded_example()).into_vec().unwrap()
    }

    fn encoded_example() -> String {
        ADDRESSES[0].to_string()
    }

    fn next_value(value: &[u8]) -> Vec<u8> {
        let current = bs58::encode(value).into_string();
        let idx = ADDRESSES.iter().position(|&a| a == current);

        let next_addr = match idx {
            Some(i) => ADDRESSES[(i + 1) % ADDRESSES.len()],
            None => ADDRESSES[0],
        };

        bs58::decode(next_addr).into_vec().unwrap()
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
                "Base58 decode error: {} (input: {}) - ensure this is a valid legacy Bitcoin address (Base58Check, 25 bytes including version and checksum)",
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
                    "Unsupported witness program length: {} (bytes: {:x?}) - expected 20 bytes (P2WPKH) or 32 bytes (P2TR). If you see 25 bytes, it's likely a Base58Check-encoded legacy address payload.",
                    source.len(),
                    source
                )));
            }
        };
        let encoded = segwit::encode(hrp::BC, version, source).map_err(|e| {
            serde::ser::Error::custom(format!("Bech32 encode error: {} (bytes: {:x?}) - check witness program version and length", e, source))
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
                "Bech32 decode error: {} (input: {}) - ensure this is a valid Bech32m address with correct HRP and witness version",
                e, s
            ))
        })
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

pub const TAG_SEGWIT: u8 = 0xB0;
pub const TAG_OP_RETURN: u8 = 0xF0;
pub const TAG_NON_ADDR: u8 = 0xFF;


fn ascii_eq_icase(a: u8, b: u8) -> bool { a == b || a ^ 0x20 == b } // ASCII only.

fn has_prefix_icase(s: &str, p: &[u8]) -> bool {
    let b = s.as_bytes();
    if b.len() < p.len() { return false; }
    for i in 0..p.len() {
        if !ascii_eq_icase(b[i], p[i]) { return false; }
    }
    true
}

fn looks_bech32_addr(s: &str) -> bool {
    // Accept bc1 / tb1 / bcrt1 (case-insensitive)
    has_prefix_icase(s, b"bc1") || has_prefix_icase(s, b"tb1") || has_prefix_icase(s, b"bcrt1")
}

fn encode_tagged_segwit<S: Serializer>(src: &[u8], ser: S) -> Result<S::Ok, S::Error> {
    // src = [TAG_SEGWIT, ver, program...]
    let ver = src[1];
    let prog = &src[2..];
    if ver > 16 {
        return Err(serde::ser::Error::custom("invalid segwit version"));
    }
    if prog.len() != 20 && prog.len() != 32 {
        return Err(serde::ser::Error::custom("invalid segwit program length"));
    }
    let wv = WitnessVersion::try_from(ver)
        .map_err(|e| serde::ser::Error::custom(format!("WitnessVersion error: {e}")))?;
    let enc = segwit::encode(hrp::BC, Fe32::from(wv), prog)
        .map_err(|e| serde::ser::Error::custom(format!("Bech32 encode error: {e}")))?;
    ser.serialize_str(&enc)
}

fn base58_serialize<S: Serializer>(src: &[u8], ser: S) -> Result<S::Ok, S::Error> {
    // src = [0x00|0x05 || 20B]
    match src.first() {
        Some(0x00) | Some(0x05) => ser.serialize_str(&bs58::encode(src).with_check().into_string()),
        Some(v) => Err(serde::ser::Error::custom(format!(
            "21 bytes but unknown legacy version 0x{v:02x} (expected 0x00 or 0x05)"
        ))),
        None => unreachable!(),
    }
}

// ---- Serialize --------------------------------------------------------------

impl SerializeAs<Vec<u8>> for BaseOrBech {
    fn serialize_as<S>(source: &Vec<u8>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Tagged segwit: [TAG_SEGWIT, ver, program]
        if source.len() >= 3 && source[0] == TAG_SEGWIT {
            return encode_tagged_segwit(source, serializer);
        }

        // Legacy Base58Check: [ver(0x00|0x05) || 20B] == 21 bytes
        if source.len() == 21 {
            return base58_serialize(source, serializer);
        }

        Err(serde::ser::Error::custom(format!(
            "unsupported address-bytes layout len={} (expected [TAG,ver,program] or [ver,payload20])",
            source.len()
        )))
    }
}

// ---- Deserialize ------------------------------------------------------------

impl<'de> DeserializeAs<'de, Vec<u8>> for BaseOrBech {
    fn deserialize_as<D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        use core::str::FromStr;

        let s = String::deserialize(deserializer)?;

        if looks_bech32_addr(&s) {
            // Robust: parse via bitcoin crate (BIP173/350). Only accept segwit payloads here.
            type Unchecked = bitcoin::address::NetworkUnchecked;
            let a_unchecked: bitcoin::Address<Unchecked> =
                bitcoin::Address::<Unchecked>::from_str(&s)
                    .map_err(|e| serde::de::Error::custom(format!("Bech32 parse failed: {e}")))?;
            let a = a_unchecked.assume_checked();
            let wp = a.witness_program()
                .ok_or_else(|| serde::de::Error::custom("expected segwit witness program"))?;

            let ver = wp.version().to_num() as u8;
            let prog = wp.program().as_bytes();
            if prog.len() != 20 && prog.len() != 32 {
                return Err(serde::de::Error::custom("invalid segwit program length"));
            }

            let mut out = Vec::with_capacity(2 + prog.len());
            out.push(TAG_SEGWIT);
            out.push(ver);
            out.extend_from_slice(prog);
            return Ok(out);
        }

        // Legacy Base58Check only: produce the exact 21B [ver||20]
        if let Ok(vec) = bs58::decode(&s).with_check(None).into_vec() {
            if vec.len() == 21 && matches!(vec.first(), Some(0x00) | Some(0x05)) {
                return Ok(vec);
            }
            return Err(serde::de::Error::custom(format!(
                "Base58 payload must be 21 bytes (ver+20), got {}", vec.len()
            )));
        }

        Err(serde::de::Error::custom(format!(
            "Invalid Bitcoin address: {s} (neither Bech32 nor Base58Check)"
        )))
    }
}
//
// ----------- Tests -------------
//
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
    struct BtcWrap(#[serde_as(as = "BaseOrBech")] Vec<u8>);

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

    use crate::codec::TAG_SEGWIT;

    fn mk_program(len: usize, fill: u8) -> Vec<u8> {
        vec![fill; len]
    }

    fn mk_btc_bytes(ver: u8, program: &[u8]) -> Vec<u8> {
        // Tagged explicit segwit bytes: [TAG_SEGWIT, ver, program...]
        let mut v = Vec::with_capacity(2 + program.len());
        v.push(TAG_SEGWIT);
        v.push(ver);
        v.extend_from_slice(program);
        v
    }

    // Bech32Wrap encodes 20->v0, 32->v1. Only these cases match BtcWrap’s string.
    fn expect_same_bech32_text(ver: u8, prog_len: usize) -> bool {
        (ver == 0 && prog_len == 20) || (ver == 1 && prog_len == 32)
    }

    #[test]
    fn test_segwit_roundtrip_cases() {
        use serde_json;

        // (version, program_len, fill_byte)
        let cases: &[(u8, usize, u8)] = &[
            (0, 20, 0x33), // v0 P2WPKH  -> both sides encode v0
            (0, 32, 0x44), // v0 P2WSH   -> BtcWrap=v0, Bech32Wrap=v1 (by design)
            (1, 32, 0x55), // v1 Taproot -> both sides encode v1
        ];

        for &(ver, len, fill) in cases {
            let program = mk_program(len, fill);
            let original = Bech32Wrap(program.clone()); // PROGRAM ONLY
            let btc = BtcWrap(mk_btc_bytes(ver, &program)); // [TAG, ver, program]

            // Each wrapper round-trips through its own serde (no I/O)
            assert_eq!(roundtrip_json(&original), original);
            assert_eq!(roundtrip_json(&btc), btc);

            // Serialize both to Bech32 strings
            let s_original = serde_json::to_string(&original).unwrap();
            let s_btc = serde_json::to_string(&btc).unwrap();

            if expect_same_bech32_text(ver, len) {
                // Same version on both paths → same text
                assert_eq!(s_original, s_btc, "mismatch for ver={ver}, len={len}");
            } else {
                // Different versions by design → different text, but cross-decode must agree on bytes
                assert_ne!(s_original, s_btc, "unexpected match for ver={ver}, len={len}");

                // Bech32 string from BtcWrap parses to the same PROGRAM in Bech32Wrap
                let back_prog: Bech32Wrap = serde_json::from_str(&s_btc).unwrap();
                assert_eq!(back_prog.0, program, "program mismatch for ver={ver}, len={len}");

                // Bech32 string from Bech32Wrap parses to tagged bytes with the version Bech32Wrap used
                let back_btc_from_original: BtcWrap = serde_json::from_str(&s_original).unwrap();
                let expected_ver_from_original = if len == 32 { 1u8 } else { 0u8 };
                assert_eq!(
                    back_btc_from_original.0,
                    mk_btc_bytes(expected_ver_from_original, &program),
                    "btc bytes mismatch when decoding original for ver={ver}, len={len}"
                );
            }
        }
    }

    #[test]
    fn reject_25_byte_legacy_payload() {
        // 25B (with checksum) must be rejected to avoid double-checksum bugs
        let bad = vec![0x00].into_iter().chain(std::iter::repeat(0x11).take(24)).collect::<Vec<_>>();
        let res = serde_json::to_string(&BtcWrap(bad));
        assert!(res.is_err());
    }
    // All lowercase bech32 for canonical display.
    const P2PKH: &str = "1QJVDzdqb1VpbDK7uDeyVXy9mR27CJiyhY";                       // legacy p2pkh
    const P2SH:  &str = "3J98t1WpEZ73CNmQviecrnyiWrnqRhWNLy";                       // legacy p2sh
    const WPKH:  &str = "bc1qvzvkjn4q3nszqxrv3nraga2r822xjty3ykvkuw";               // v0, 20 (from bitcoin crate tests)
    const WSH:   &str = "bc1qwqdg6squsna38e46795at95yu9atm8azzmyvckulcc7kytlcckxswvvzej"; // v0, 32 (from bitcoin crate tests)
    const TR:    &str = "bc1p5cyxnuxmeuwuvkwfem96lqzszd02n6xdcjrs20cac6yqjjwudpxqkedrcr"; // v1, 32 (from bitcoin crate tests / BIP-086)

    #[test]
    fn segwit_tagged_layout_is_unambiguous() {
        use crate::codec::TAG_SEGWIT;

        // Build [TAG, ver, program] for v0(20), v0(32), v1(32)
        let cases: &[(u8, usize, u8)] = &[(0, 20, 0x33), (0, 32, 0x44), (1, 32, 0x55)];

        let mut strings = std::collections::BTreeSet::<String>::new();

        for &(ver, len, fill) in cases {
            let program = vec![fill; len];
            let mut bytes = Vec::with_capacity(2 + len);
            bytes.push(TAG_SEGWIT);
            bytes.push(ver);
            bytes.extend_from_slice(&program);

            // 1) Own serde roundtrip
            let wrapped = BtcWrap(bytes.clone());
            assert_eq!(roundtrip_json(&wrapped), wrapped);

            // 2) Serialize to Bech32 string and ensure distinctness across cases
            let s = serde_json::to_string(&wrapped).unwrap();
            assert!(strings.insert(s.clone()), "duplicate encoding for ver={ver}, len={len}");

            // 3) Cross-decode that string via Bech32Wrap (program only)
            let bech_prog: Bech32Wrap = serde_json::from_str(&s).unwrap();
            assert_eq!(bech_prog.0, program, "program mismatch ver={ver}, len={len}");

            // 4) Cross-decode that string via Base58Wrap should fail
            let base58_res: Result<Base58Wrap, _> = serde_json::from_str(&s);
            assert!(base58_res.is_err(), "bech32 string decoded as base58 unexpectedly");
        }
    }

    #[test]
    fn legacy_layout_is_enforced_and_injective() {
        // Construct a valid legacy payload: [version || 20B payload]
        let mut p2pkh = vec![0x00];
        p2pkh.extend(std::iter::repeat(0x11).take(20));
        let p2pkh_w = BtcWrap(p2pkh.clone());
        assert_eq!(roundtrip_json(&p2pkh_w), p2pkh_w);

        let s1 = serde_json::to_string(&p2pkh_w).unwrap();

        // Different payload -> different string
        let mut p2pkh2 = vec![0x00];
        p2pkh2.extend(std::iter::repeat(0x12).take(20));
        let s2 = serde_json::to_string(&BtcWrap(p2pkh2)).unwrap();
        assert_ne!(s1, s2);

        // Wrong legacy version should error
        let mut bad = vec![0x07];
        bad.extend(std::iter::repeat(0x22).take(20));
        let e = serde_json::to_string(&BtcWrap(bad)).unwrap_err().to_string();
        assert!(e.contains("unknown legacy version"));
    }

    #[test]
    fn reject_checksum_form_25_bytes_and_bad_lengths() {
        // 25B legacy (with checksum) must not be accepted by serializer
        let bad_25 = {
            let mut v = Vec::with_capacity(25);
            v.push(0x00);
            v.extend(std::iter::repeat(0x42).take(24));
            v
        };
        assert!(serde_json::to_string(&BtcWrap(bad_25)).is_err());

        // Segwit tag with invalid length
        let mut bad_tag = vec![TAG_SEGWIT, 0];
        bad_tag.extend(std::iter::repeat(0xAA).take(21)); // invalid len (21)
        let err = serde_json::to_string(&BtcWrap(bad_tag)).unwrap_err().to_string();
        assert!(err.contains("invalid segwit program length"));

        // Segwit tag with invalid version (>16)
        let mut bad_ver = vec![TAG_SEGWIT, 17];
        bad_ver.extend(std::iter::repeat(0xBB).take(20));
        let err2 = serde_json::to_string(&BtcWrap(bad_ver)).unwrap_err().to_string();
        assert!(err2.contains("invalid segwit version"));
    }

    #[test]
    fn cross_decode_between_wrappers_behaves_as_specified() {
        // For v0 32B (WSH), Bech32Wrap encodes as v1 by design, so texts differ;
        // we only assert that cross-decode reproduces program and tagged bytes.

        // v0 32B tagged
        let program = vec![0x44; 32];
        let mut tagged = vec![TAG_SEGWIT, 0];
        tagged.extend_from_slice(&program);
        let btc = BtcWrap(tagged.clone());

        let s_btc = serde_json::to_string(&btc).unwrap(); // bc1q...
        let back_prog: Bech32Wrap = serde_json::from_str(&s_btc).unwrap();
        assert_eq!(back_prog.0, program);

        // program-only wrapper will encode as v1 (bc1p...), decode back as [TAG,1,program]
        let bech = Bech32Wrap(program.clone());
        let s_prog = serde_json::to_string(&bech).unwrap();
        assert_ne!(s_prog, s_btc);

        let back_tagged: BtcWrap = serde_json::from_str(&s_prog).unwrap();
        let mut expected = vec![TAG_SEGWIT, 1];
        expected.extend_from_slice(&program);
        assert_eq!(back_tagged.0, expected);
    }

    #[test]
    fn parity_with_bitcoin_address_roundtrip() {
        use bitcoin::{Address, Network};
        use std::str::FromStr;

        // early guard: all vectors must parse as Bitcoin addresses
        for s in [P2PKH, P2SH, WPKH, WSH, TR] {
            let _ = Address::from_str(s).expect("bad test vector").require_network(Network::Bitcoin).unwrap();
        }

        let addr_strs = [P2PKH, P2SH, WPKH, WSH, TR];

        for s in addr_strs {
            let addr = Address::from_str(s).unwrap().require_network(Network::Bitcoin).unwrap();

            // Build DB bytes as indexer would
            let db_bytes = {
                match addr.address_type() {
                    Some(bitcoin::address::AddressType::P2pkh) => {
                        let mut v = vec![0x00];
                        v.extend_from_slice(addr.pubkey_hash().unwrap().as_ref());
                        v
                    }
                    Some(bitcoin::address::AddressType::P2sh) => {
                        let mut v = vec![0x05];
                        v.extend_from_slice(addr.script_hash().unwrap().as_ref());
                        v
                    }
                    _ => {
                        let wp = addr.witness_program().unwrap();
                        let ver = wp.version().to_num() as u8;
                        let prog = wp.program().as_bytes();
                        let mut v = vec![TAG_SEGWIT, ver];
                        v.extend_from_slice(prog);
                        v
                    }
                }
            };

            // Deserialize string with BaseOrBech and compare
            let parsed: BtcWrap = serde_json::from_str(&format!("\"{s}\"")).unwrap();
            assert_eq!(parsed.0, db_bytes, "db bytes mismatch for {s}");

            // Serialize back to text equals s (canonical)
            let out = serde_json::to_string(&parsed).unwrap();
            assert_eq!(out, format!("\"{s}\""));
        }
    }

    #[test]
    fn roundtrip_known_legacy_and_segwit_strings() {
        use bitcoin::{Address, Network};
        use std::str::FromStr;

        // guard: vectors must parse
        for s in [P2PKH, P2SH, WPKH, WSH, TR] {
            let _ = Address::from_str(s).expect("bad test vector").require_network(Network::Bitcoin).unwrap();
        }

        let addrs = [P2PKH, P2SH, WPKH, WSH, TR];

        for s in addrs {
            let json = format!("\"{s}\"");
            let val: BtcWrap = serde_json::from_str(&json).unwrap(); // string -> bytes
            let back = serde_json::to_string(&val).unwrap();         // bytes -> string
            assert_eq!(back, json, "string roundtrip mismatch for {s}");
        }
    }
}
