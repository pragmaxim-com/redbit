use bech32::{self, Hrp, Bech32m};
use serde::{Deserialize, Deserializer, Serializer};
use serde_with::{DeserializeAs, SerializeAs};
use crate::model_v1::ByteVecColumnSerde;

#[allow(dead_code)]
pub struct Base58;

const ADDRESSES: &[&str] = &[
    "37btjrVyb4KDXBNC4haBVPCrro8AQPHwvCMp3RFhhSVWwfFmZ6wwzSK6JK1hY6wHNmtrpTf1kdbva8TCneM2YsiXT7mrzT21EacHnPpz5YyUdj64na",
    "addr1vpu5vlrf4xkxv2qpwngf6cjhtw542ayty80v8dyr49rf5eg0yu80w",
    "stake1vpu5vlrf4xkxv2qpwngf6cjhtw542ayty80v8dyr49rf5egfu2p0u",
    "addr_vk1w0l2sr2zgfm26ztc6nl9xy8ghsk5sh6ldwemlpmp9xylzy4dtf7st80zhd",
    "stake_vk1px4j0r2fk7ux5p23shz8f3y5y2qam7s954rgf3lg5merqcj6aetsft99wu",
    "script1cda3khwqv60360rp5m7akt50m6ttapacs8rqhn5w342z7r35m37",
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
        bs58::decode(s)
            .with_check(None)
            .into_vec()
            .map_err(serde::de::Error::custom)
    }
}

#[allow(dead_code)]
pub struct Bech32;

impl ByteVecColumnSerde for Bech32 {
    fn decoded_example() -> Vec<u8> {
        bech32::decode(&Self::encoded_example()).unwrap().1
    }
    fn encoded_example() -> String {
        "addr1vpu5vlrf4xkxv2qpwngf6cjhtw542ayty80v8dyr49rf5eg0yu80w".to_string()
    }
}

impl SerializeAs<Vec<u8>> for Bech32 {
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

impl<'de> DeserializeAs<'de, Vec<u8>> for Bech32 {
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
pub struct BaseOrBech;

impl ByteVecColumnSerde for BaseOrBech {
    fn decoded_example() -> Vec<u8> {
        bech32::decode(&Self::encoded_example()).unwrap().1
    }
    fn encoded_example() -> String {
        "stake1vpu5vlrf4xkxv2qpwngf6cjhtw542ayty80v8dyr49rf5egfu2p0u".to_string()
    }
}

impl SerializeAs<Vec<u8>> for BaseOrBech {
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

impl<'de> DeserializeAs<'de, Vec<u8>> for BaseOrBech {
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


#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    use serde_with::serde_as;
    use crate::model_v1::serde_json;

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
    struct CardanoWrap(
        #[serde_as(as = "BaseOrBech")] Vec<u8>
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
