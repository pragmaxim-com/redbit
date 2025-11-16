#![feature(test)]
extern crate test;

pub mod query;
pub mod retry;
pub mod logger;
pub mod storage;
pub mod utils;
pub mod error;
pub mod rest;
pub mod codec;
mod macro_rules;

pub use axum;
pub use axum::body::Body;
pub use axum::extract;
pub use axum::http::StatusCode;
pub use axum::response::IntoResponse;
pub use axum::response::Response;
pub use axum_streams;
pub use bincode::{decode_from_slice, encode_to_vec, Decode, Encode};
pub use chrono;
pub use futures;
pub use futures::stream::{self, StreamExt};
pub use futures_util::stream::TryStreamExt;
pub use http;
pub use http::HeaderValue;
pub use indexmap;
pub use inventory;
pub use itertools::Either;
pub use lru::LruCache;
pub use macros::column;
pub use macros::entity;
pub use macros::pointer_key;
pub use macros::root_key;
pub use macros::Entity;
pub use macros::PointerKey;
pub use macros::RootKey;
pub use once_cell;
pub use query::*;
pub use rand;
pub use redb;
pub use redb::{
    AccessGuard, Database, Durability, Key, MultimapTable, MultimapTableDefinition, MultimapValue, ReadOnlyMultimapTable, ReadOnlyTable, ReadTransaction, ReadableDatabase, ReadableMultimapTable,
    ReadableTable, ReadableTableMetadata, Table, TableDefinition, TableError, TableStats, TransactionError, TypeName, Value, WriteTransaction,
};
pub use serde;
pub use serde::Deserialize;
pub use serde::Deserializer;
pub use serde::Serialize;
pub use serde::Serializer;
pub use serde_json;
pub use serde_json::json;
pub use serde_urlencoded;
pub use serde_with;
pub use std::any::type_name;
pub use std::cmp::Ordering;
pub use std::collections::HashMap;
pub use std::collections::HashSet;
pub use std::collections::VecDeque;
pub use std::fmt::Debug;
pub use std::pin::Pin;
pub use std::sync::Arc;
pub use std::sync::Weak;
pub use std::thread;
pub use std::time::Duration;
pub use std::time::Instant;
pub use rest::{RequestState, ErrorResponse, MaybeJson, AppJson, FilterOp};
pub use error::{AppError, ParsePointerError};
pub use storage::context::{ReadTxContext, ToReadField, ToWriteField, TxContext, WriteTxContext};
pub use storage::init::{Storage, DbDef, StorageOwner};
pub use storage::partitioning::{BytesPartitioner, KeyPartitioner, Partitioning, ValuePartitioner, Xxh3Partitioner};
pub use storage::table_dict::DictFactory;
pub use storage::table_dict_read::ShardedReadOnlyDictTable;
pub use storage::table_dict_write::DictTable;
pub use storage::table_index::IndexFactory;
pub use storage::table_index_read::ShardedReadOnlyIndexTable;
pub use storage::table_plain::PlainFactory;
pub use storage::table_plain_read::ShardedReadOnlyPlainTable;
pub use storage::table_writer::ShardedTableWriter;
pub use storage::table_writer_api::{FlushFuture, RedbitTableDefinition, ShardedTableReader, StartFuture, StopFuture, TaskResult, TableInfo, ReadTableLike, WriteComponentRef, WriteTableLike, WriterLike};
pub use storage::tx_fsm::TxFSM;
pub use urlencoding;
pub use utoipa;
pub use utoipa::openapi;
pub use utoipa::openapi::extensions::ExtensionsBuilder;
pub use utoipa::openapi::schema::*;
pub use utoipa::IntoParams;
pub use utoipa::PartialSchema;
pub use utoipa::ToSchema;
pub use utoipa_axum;
pub use utoipa_axum::router::OpenApiRouter;
pub use utoipa_swagger_ui;

use std::borrow::{Borrow, Cow};
use std::hash::Hash;
use std::ops::Add;

pub trait ColInnerType { type Repr; }

pub trait IndexedPointer: Copy {
    type Index: Copy + Ord + Add<Output = Self::Index> + Default + Into<u128>;
    fn index(&self) -> Self::Index;
    fn next_index(&self) -> Self;
    fn nth_index(&self, n: usize) -> Self;
    fn rollback_or_init(&self, n: u32) -> Self;
}

pub trait RootPointer: IndexedPointer + Copy {
    fn total_index(&self) -> u128;
    fn is_pointer(&self) -> bool;
    fn from_many(indexes: &[Self::Index]) -> Vec<Self>;
    fn depth(&self) -> usize;
}

pub trait ChildPointer: IndexedPointer + Copy {
    type Parent: IndexedPointer + Copy;
    fn total_index(&self) -> u128;
    fn is_pointer(&self) -> bool;
    fn parent(&self) -> Self::Parent;
    fn from_parent(parent: Self::Parent, index: Self::Index) -> Self;
    fn depth(&self) -> usize;
}

pub trait ForeignKey<CH>: Copy
where
    CH: ChildPointer + Copy,
    CH::Parent: IndexedPointer + Copy,
{
    fn fk_range(&self) -> (CH, CH);
}

impl<CH> ForeignKey<CH> for CH::Parent
where
    CH: ChildPointer + Copy,
    CH::Parent: IndexedPointer + Copy,
{
    fn fk_range(&self) -> (CH, CH) {
        (CH::from_parent(*self, CH::Index::default()), CH::from_parent(self.next_index(), CH::Index::default()))
    }
}

pub trait Sampleable: Default + Sized + Clone {
    fn next_value(&self) -> Self;
    fn nth_value(&self, n: usize) -> Self {
        let mut value = self.clone(); // convert &Self → Self
        let mut n = n;
        while n > 0 {
            value = value.next_value();
            n -= 1;
        }
        value
    }
    fn sample_many_from(n: usize, from: usize) -> Vec<Self> {
        let start = from.saturating_mul(n); // O(1)
        let mut values = Vec::with_capacity(n);
        let mut value = Self::default().nth_value(start); // O(start)
        for _ in 0..n {
            values.push(value.clone());
            value = value.next_value(); // O(1) per step
        }
        values
    }
    fn seed_nth_with_index_zero(from: usize) -> Self {
        Self::default().nth_value(from)
    }
    fn sample_many_from_seed_index_only(n: usize, seed: &Self) -> Vec<Self> {
        let mut out = Vec::with_capacity(n);
        let mut v = seed.clone();
        for _ in 0..n {
            out.push(v.clone());
            v = v.next_value();
        }
        out
    }
}

macro_rules! impl_sampleable_for_primitive {
    ($($t:ty),*) => {
        $(
            impl Sampleable for $t {
                fn next_value(&self) -> Self {
                    self.wrapping_add(1)
                }
            }
        )*
    };
}

impl_sampleable_for_primitive!(u8, u16, u32, u64, usize, i8, i16, i32, i64, isize);

pub trait UrlEncoded {
    fn url_encode(&self) -> String;
}

pub trait DbKey: Key + Copy + 'static
where
    Self: Borrow<<Self as Value>::SelfType<'static>> {
    type Unit: Copy + Send + 'static;

    fn to_unit_ref<'a>(v: &Self::SelfType<'a>) -> Self::Unit
    where
        Self: 'a;

    fn as_value_from_unit<'a>(u: &'a Self::Unit) -> Self::SelfType<'a>
    where
        Self: 'a;

    fn to_unit<'a>(v: Self::SelfType<'a>) -> Self::Unit
    where
        Self: 'a;

    fn from_unit<'a>(u: Self::Unit) -> Self::SelfType<'a>
    where
        Self: 'a;

}

pub trait DbVal: Key + 'static
where for<'a> Self: Borrow<<Self as Value>::SelfType<'a>>
{
}

impl<T> DbVal for T
where
    T: Key + 'static,
    for<'a> T: Borrow<<T as Value>::SelfType<'a>>,
{
}
pub trait CacheKey: DbVal
where for<'a> Self: Borrow<<Self as Value>::SelfType<'a>>,
{
    type CK: Eq + Hash + Clone;
    fn cache_key<'a>(v: &Self::SelfType<'a>) -> Self::CK
    where
        Self: 'a;
}

pub trait BinaryCodec {
    fn from_le_bytes(bytes: &[u8]) -> Self;
    fn as_le_bytes(&self) -> Vec<u8>;
    fn as_le_bytes_cow(&self) -> Cow<'_, [u8]> {
        Cow::Owned(self.as_le_bytes())
    }
    fn size() -> usize;
}

pub trait ByteVecColumnSerde {
    fn decoded_example() -> Vec<u8>;
    fn encoded_example() -> String;
    fn next_value(value: &[u8]) -> Vec<u8> {
        let mut vec = value.to_owned();
        if let Some(last) = vec.last_mut() {
            *last = last.wrapping_add(1);
        } else {
            vec.push(1);
        }
        vec
    }
}

pub struct StructInfo {
    pub name: &'static str,
    pub root: bool,
    pub routes_fn: fn() -> OpenApiRouter<RequestState>,
    pub db_defs: fn() -> Vec<DbDef>,
}

inventory::collect!(StructInfo);
