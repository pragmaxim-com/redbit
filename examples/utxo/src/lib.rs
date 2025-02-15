pub use redbit::*;
use std::fmt::Debug;
use redb::{Key, TypeName, Value};
use std::borrow::Cow;
use std::cmp::Ordering;

type Amount = u64;
type Datum = String;
type Address = String;

#[derive(Redbit, Debug)]
pub struct Utxo {
    #[pk]
    pub id: UtxoPointer,
    #[column]
    pub amount: Amount,
    #[column(index)]
    pub datum: Datum,
    #[column(index, dictionary)]
    pub address: Address,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UtxoPointer {
    pub block_height: u32,
    pub tx_index: u16,
    pub utxo_index: u16,
}

impl Key for UtxoPointer {
    fn compare(data1: &[u8], data2: &[u8]) -> Ordering {
        data1.cmp(data2)
    }
}

impl Value for UtxoPointer {
    type SelfType<'a> = UtxoPointer  where Self: 'a;
    type AsBytes<'a> = Cow<'a, [u8]> where Self: 'a;

    fn fixed_width() -> Option<usize> {
        Some(8)
    }

    fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
        where Self: 'a,
    {
        UtxoPointer {
            block_height: u32::from_le_bytes(data[0..4].try_into().unwrap()),
            tx_index: u16::from_le_bytes(data[4..6].try_into().unwrap()),
            utxo_index: u16::from_le_bytes(data[6..8].try_into().unwrap()),
        }
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a>
        where Self: 'b,
    {
        let mut bytes = [0u8; 8];
        bytes[0..4].copy_from_slice(&value.block_height.to_le_bytes());
        bytes[4..6].copy_from_slice(&value.tx_index.to_le_bytes());
        bytes[6..8].copy_from_slice(&value.utxo_index.to_le_bytes());
        Cow::Owned(bytes.to_vec())
    }

    fn type_name() -> TypeName {
        TypeName::new("redbit::UtxoPointer")
    }
}