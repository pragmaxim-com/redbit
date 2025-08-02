use bech32::{self, Hrp, Bech32m};
use serde::{Deserialize, Deserializer, Serializer};
use serde_with::{DeserializeAs, SerializeAs};

#[allow(dead_code)]
pub struct CardanoBase58;

impl SerializeAs<Vec<u8>> for CardanoBase58 {
    fn serialize_as<S>(source: &Vec<u8>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&bs58::encode(source).with_check().into_string())
    }
}

impl<'de> DeserializeAs<'de, Vec<u8>> for CardanoBase58 {
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

#[allow(dead_code)]
pub struct CardanoBech32;

impl SerializeAs<Vec<u8>> for CardanoBech32 {
    fn serialize_as<S>(source: &Vec<u8>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let hrp = Hrp::parse("addr").map_err(serde::ser::Error::custom)?;
        let encoded = bech32::encode::<Bech32m>(hrp, source)
            .map_err(serde::ser::Error::custom)?;
        serializer.serialize_str(&encoded)
    }
}

impl<'de> DeserializeAs<'de, Vec<u8>> for CardanoBech32 {
    fn deserialize_as<D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let (_hrp, data) = bech32::decode(&s).map_err(serde::de::Error::custom)?;
        Ok(data)
    }
}

#[allow(dead_code)]
pub struct Cardano;

impl SerializeAs<Vec<u8>> for Cardano {
    fn serialize_as<S>(source: &Vec<u8>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if source.len() >= 20 && source.len() <= 32 {
            let hrp = Hrp::parse("addr").map_err(serde::ser::Error::custom)?;
            let encoded = bech32::encode::<Bech32m>(hrp, source)
                .map_err(serde::ser::Error::custom)?;
            return serializer.serialize_str(&encoded);
        }

        serializer.serialize_str(&bs58::encode(source).with_check().into_string())
    }
}

impl<'de> DeserializeAs<'de, Vec<u8>> for Cardano {
    fn deserialize_as<D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;

        if let Ok((_hrp, data)) = bech32::decode(&s) {
            return Ok(data);
        }

        if let Ok(vec) = bs58::decode(&s).with_check(None).into_vec() {
            return Ok(vec);
        }

        Err(serde::de::Error::custom("invalid Cardano address format"))
    }
}



fn default_bytes() -> Vec<u8> {
    (0..64).map(|i| i as u8).collect()
}

pub fn cardano_base58_legacy_payload() -> Vec<u8> {
    let random_bytes = default_bytes();
    random_bytes[..28].to_vec() // Legacy Byron-style payload length varies, using 28 bytes here
}

pub fn cardano_bech32_addr_payload() -> Vec<u8> {
    let random_bytes = default_bytes();
    random_bytes[..28].to_vec() // Shelley addresses are usually 28 bytes
}

pub fn cardano_bech32_stake_payload() -> Vec<u8> {
    let random_bytes = default_bytes();
    random_bytes[..28].to_vec() // Stake key hashes are also typically 28 bytes
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    use serde_with::serde_as;

    #[serde_as]
    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Base58Wrap(
        #[serde_as(as = "CardanoBase58")] Vec<u8>
    );

    #[serde_as]
    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Bech32Wrap(
        #[serde_as(as = "CardanoBech32")] Vec<u8>
    );

    #[serde_as]
    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct CardanoWrap(
        #[serde_as(as = "Cardano")] Vec<u8>
    );

    fn roundtrip_json<T>(value: &T) -> T
    where
        T: Serialize + for<'de> Deserialize<'de> + PartialEq + std::fmt::Debug,
    {
        let json = serde_json::to_string(value).unwrap();
        serde_json::from_str(&json).unwrap()
    }

    #[test]
    fn test_legacy_base58_roundtrip() {
        let payload = vec![0x11; 28];
        let original = Base58Wrap(payload.clone());
        let cardano = CardanoWrap(payload.clone());
        assert_eq!(roundtrip_json(&original), original);
        assert_eq!(roundtrip_json(&cardano), cardano);
        assert_eq!(original.0, cardano.0);
    }

    #[test]
    fn test_bech32_addr_roundtrip() {
        let payload = vec![0x22; 28];
        let original = Bech32Wrap(payload.clone());
        let cardano = CardanoWrap(payload.clone());
        assert_eq!(roundtrip_json(&original), original);
        assert_eq!(roundtrip_json(&cardano), cardano);
        assert_eq!(original.0, cardano.0);
    }

    #[test]
    fn test_bech32_stake_roundtrip() {
        let payload = vec![0x33; 28];
        let original = Bech32Wrap(payload.clone());
        let cardano = CardanoWrap(payload.clone());
        assert_eq!(roundtrip_json(&original), original);
        assert_eq!(roundtrip_json(&cardano), cardano);
        assert_eq!(original.0, cardano.0);
    }
}
