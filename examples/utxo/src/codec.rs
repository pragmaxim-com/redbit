use crate::{AssetPointer, BlockPointer, TxPointer, UtxoPointer};

use redb::{Key, TypeName, Value};
use std::borrow::Cow;
use std::cmp::Ordering;
use std::convert::TryInto;

impl Key for BlockPointer {
    fn compare(data1: &[u8], data2: &[u8]) -> Ordering {
        data1.cmp(data2)
    }
}

impl Value for BlockPointer {
    type SelfType<'a>
        = BlockPointer
    where
        Self: 'a;
    type AsBytes<'a>
        = Cow<'a, [u8]>
    where
        Self: 'a;

    fn fixed_width() -> Option<usize> {
        Some(4)
    }

    fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
    where
        Self: 'a,
    {
        BlockPointer { height: u32::from_be_bytes(data[0..4].try_into().unwrap()) }
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a>
    where
        Self: 'b,
    {
        let mut buf = [0u8; 4];
        buf[0..4].copy_from_slice(&value.height.to_be_bytes());
        Cow::Owned(buf.to_vec())
    }

    fn type_name() -> TypeName {
        TypeName::new("redbit::BlockPointer")
    }
}

impl Key for TxPointer {
    fn compare(data1: &[u8], data2: &[u8]) -> Ordering {
        data1.cmp(data2)
    }
}

impl Value for TxPointer {
    type SelfType<'a>
        = TxPointer
    where
        Self: 'a;
    type AsBytes<'a>
        = Cow<'a, [u8]>
    where
        Self: 'a;

    fn fixed_width() -> Option<usize> {
        Some(6)
    }

    fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
    where
        Self: 'a,
    {
        TxPointer {
            block_pointer: BlockPointer { height: u32::from_be_bytes(data[0..4].try_into().unwrap()) },
            tx_index: u16::from_be_bytes(data[4..6].try_into().unwrap()),
        }
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a>
    where
        Self: 'b,
    {
        let mut buf = [0u8; 6];
        buf[0..4].copy_from_slice(&value.block_pointer.height.to_be_bytes());
        buf[4..6].copy_from_slice(&value.tx_index.to_be_bytes());
        Cow::Owned(buf.to_vec())
    }

    fn type_name() -> TypeName {
        TypeName::new("redbit::TxPointer")
    }
}

impl Key for UtxoPointer {
    fn compare(data1: &[u8], data2: &[u8]) -> Ordering {
        data1.cmp(data2)
    }
}

impl Value for UtxoPointer {
    type SelfType<'a>
        = UtxoPointer
    where
        Self: 'a;
    type AsBytes<'a>
        = Cow<'a, [u8]>
    where
        Self: 'a;

    fn fixed_width() -> Option<usize> {
        Some(8)
    }

    fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
    where
        Self: 'a,
    {
        UtxoPointer {
            tx_pointer: TxPointer {
                block_pointer: BlockPointer { height: u32::from_be_bytes(data[0..4].try_into().unwrap()) },
                tx_index: u16::from_be_bytes(data[4..6].try_into().unwrap()),
            },
            utxo_index: u16::from_be_bytes(data[6..8].try_into().unwrap()),
        }
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a>
    where
        Self: 'b,
    {
        let mut buf = [0u8; 8];
        buf[0..4].copy_from_slice(&value.tx_pointer.block_pointer.height.to_be_bytes());
        buf[4..6].copy_from_slice(&value.tx_pointer.tx_index.to_be_bytes());
        buf[6..8].copy_from_slice(&value.utxo_index.to_be_bytes());
        Cow::Owned(buf.to_vec())
    }

    fn type_name() -> TypeName {
        TypeName::new("redbit::UtxoPointer")
    }
}

impl Key for AssetPointer {
    fn compare(data1: &[u8], data2: &[u8]) -> Ordering {
        data1.cmp(data2)
    }
}

impl Value for AssetPointer {
    type SelfType<'a>
        = AssetPointer
    where
        Self: 'a;
    type AsBytes<'a>
        = Cow<'a, [u8]>
    where
        Self: 'a;

    fn fixed_width() -> Option<usize> {
        Some(10)
    }

    fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
    where
        Self: 'a,
    {
        AssetPointer {
            utxo_pointer: UtxoPointer {
                tx_pointer: TxPointer {
                    block_pointer: BlockPointer { height: u32::from_be_bytes(data[0..4].try_into().unwrap()) },
                    tx_index: u16::from_be_bytes(data[4..6].try_into().unwrap()),
                },
                utxo_index: u16::from_be_bytes(data[6..8].try_into().unwrap()),
            },
            asset_index: u16::from_be_bytes(data[8..10].try_into().unwrap()),
        }
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a>
    where
        Self: 'b,
    {
        let mut buf = [0u8; 10];
        buf[0..4].copy_from_slice(&value.utxo_pointer.tx_pointer.block_pointer.height.to_be_bytes());
        buf[4..6].copy_from_slice(&value.utxo_pointer.tx_pointer.tx_index.to_be_bytes());
        buf[6..8].copy_from_slice(&value.utxo_pointer.utxo_index.to_be_bytes());
        buf[8..10].copy_from_slice(&value.asset_index.to_be_bytes());
        Cow::Owned(buf.to_vec())
    }

    fn type_name() -> TypeName {
        TypeName::new("redbit::UtxoPointer")
    }
}
