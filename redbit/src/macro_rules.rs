
#[macro_export]
macro_rules! impl_copy_owned_value_identity {
        ($t:ty) => {
            impl DbKey for $t
            where
                <$t as redb::Value>::SelfType<'static>: Copy + Send + 'static,
            {
                type Unit = <$t as redb::Value>::SelfType<'static>;
                #[inline]
                fn to_unit<'a>(v: <$t as redb::Value>::SelfType<'a>) -> Self::Unit
                where
                    Self: 'a,
                {
                    v
                }
                #[inline]
                fn from_unit<'a>(u: Self::Unit) -> <$t as redb::Value>::SelfType<'a>
                where
                    Self: 'a,
                {
                    u
                }
                #[inline]
                fn to_unit_ref<'a>(v: &<$t as redb::Value>::SelfType<'a>) -> Self::Unit
                where
                    Self: 'a,
                {
                    *v
                }
                #[inline]
                fn as_value_from_unit<'a>(u: &'a Self::Unit) -> <$t as redb::Value>::SelfType<'a>
                where
                    Self: 'a,
                {
                    *u
                }
            }
        };
    }

// Redb Key/Value

#[macro_export]
macro_rules! impl_redb_newtype_array {
    ($New:ident, $N:expr) => {
        impl redb::Value for $New {
            type SelfType<'a> = $New where Self: 'a;
            type AsBytes<'a> = &'a [u8; $N] where Self: 'a;

            fn fixed_width() -> Option<usize> { Some($N) }

            fn from_bytes<'a>(data: &'a [u8]) -> $New
            where Self: 'a {
                $New(data.try_into().expect("slice length mismatch"))
            }

            fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> &'a [u8; $N]
            where Self: 'a, Self: 'b {
                &value.0
            }

            fn type_name() -> redb::TypeName {
                redb::TypeName::new(concat!("[u8;", stringify!($N), "]"))
            }
        }

        impl redb::Key for $New {
            fn compare(data1: &[u8], data2: &[u8]) -> std::cmp::Ordering {
                data1.cmp(data2)
            }
        }
    };
}

#[macro_export]
macro_rules! impl_redb_newtype_vec {
    ($New:ident) => {
        impl redb::Value for $New {
            type SelfType<'a> = $New where Self: 'a;
            type AsBytes<'a> = &'a [u8] where Self: 'a;

            fn fixed_width() -> Option<usize> { None }

            fn from_bytes<'a>(data: &'a [u8]) -> $New
            where
                Self: 'a,
            {
                $New(data.to_vec())
            }

            fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> &'a [u8]
            where
                Self: 'a,
                Self: 'b,
            {
                value.0.as_ref()
            }

            fn type_name() -> redb::TypeName {
                redb::TypeName::new("Vec<u8>")
            }
        }

        impl redb::Key for $New {
            fn compare(data1: &[u8], data2: &[u8]) -> std::cmp::Ordering {
                data1.cmp(data2)
            }
        }
    };
}

#[macro_export]
macro_rules! impl_redb_newtype_integer {
    ($New:ident, $Int:ty) => {
        impl redb::Value for $New {
            type SelfType<'a> = $New where Self: 'a;
            type AsBytes<'a> = [u8; std::mem::size_of::<$Int>()] where Self: 'a;

            fn fixed_width() -> Option<usize> {
                Some(std::mem::size_of::<$Int>())
            }

            fn from_bytes<'a>(data: &'a [u8]) -> $New
            where Self: 'a {
                $New(<$Int>::from_le_bytes(data.try_into().expect("slice length mismatch")))
            }

            fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> [u8; std::mem::size_of::<$Int>()]
            where Self: 'a, Self: 'b {
                value.0.to_le_bytes()
            }

            fn type_name() -> redb::TypeName {
                redb::TypeName::new(stringify!($Int))
            }
        }

        impl redb::Key for $New {
            fn compare(data1: &[u8], data2: &[u8]) -> std::cmp::Ordering {
                let v1 = <$Int>::from_le_bytes(data1.try_into().expect("slice length mismatch"));
                let v2 = <$Int>::from_le_bytes(data2.try_into().expect("slice length mismatch"));
                v1.cmp(&v2)
            }
        }
    };
}

#[macro_export]
macro_rules! impl_redb_newtype_binary {
    ($New:ident) => {
        impl redb::Value for $New {
            type SelfType<'a> = $New where Self: 'a;
            type AsBytes<'a> = std::borrow::Cow<'a, [u8]> where Self: 'a;

            fn fixed_width() -> Option<usize> {
                Some(<$New as BinaryCodec>::size())
            }

            fn from_bytes<'a>(data: &'a [u8]) -> $New
            where Self: 'a {
                <$New as BinaryCodec>::from_le_bytes(data)
            }

            fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a>
            where Self: 'a, Self: 'b {
                value.as_le_bytes_cow()
            }

            fn type_name() -> redb::TypeName {
                redb::TypeName::new(stringify!($New))
            }
        }

        impl redb::Key for $New {
            fn compare(data1: &[u8], data2: &[u8]) -> std::cmp::Ordering {
                let a = <$New as BinaryCodec>::from_le_bytes(data1);
                let b = <$New as BinaryCodec>::from_le_bytes(data2);
                a.cmp(&b)
            }
        }
    };
}

#[macro_export]
macro_rules! impl_redb_newtype_bincode {
    ($New:ident) => {
        impl redb::Value for $New {
            type SelfType<'a> = $New where Self: 'a;
            // Bincode encoding allocates; expose owned bytes.
            type AsBytes<'a> = Vec<u8> where Self: 'a;

            fn fixed_width() -> Option<usize> { None }

            fn from_bytes<'a>(data: &'a [u8]) -> $New
            where Self: 'a {
                bincode::decode_from_slice::<$New, _>(data, bincode::config::standard())
                    .unwrap()
                    .0
            }

            fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Vec<u8>
            where Self: 'a, Self: 'b {
                bincode::encode_to_vec(value, bincode::config::standard()).unwrap()
            }

            fn type_name() -> redb::TypeName {
                redb::TypeName::new(std::any::type_name::<$New>())
            }
        }

        impl redb::Key for $New {
            fn compare(data1: &[u8], data2: &[u8]) -> std::cmp::Ordering {
                let v1 = bincode::decode_from_slice::<$New, _>(data1, bincode::config::standard())
                    .unwrap().0;
                let v2 = bincode::decode_from_slice::<$New, _>(data2, bincode::config::standard())
                    .unwrap().0;
                v1.cmp(&v2)
            }
        }
    };
}

// CacheKey implementations

#[macro_export]
macro_rules! impl_cachekey_array {
    ($T:ty, $N:expr) => {
        impl CacheKey for $T {
            type CK = [u8; $N];

            #[inline]
            fn cache_key<'a>(v: &<$T as redb::Value>::SelfType<'a>) -> Self::CK
            where
                $T: 'a,
            {
                v.0
            }
        }
    };
}

/// Generates CacheKey impl for integer newtypes.
/// Example: pub struct BlockHeight(pub u64);
#[macro_export]
macro_rules! impl_cachekey_integer {
    ($T:ty, $Int:ty) => {
        impl CacheKey for $T {
            type CK = $Int;

            #[inline]
            fn cache_key<'a>(v: &<$T as redb::Value>::SelfType<'a>) -> Self::CK
            where
                $T: 'a,
            {
                v.0
            }
        }
    };
}

/// Generates CacheKey impl for types implementing BinaryCodec.
/// Example: pub struct BlockPtr(pub u64);
#[macro_export]
macro_rules! impl_cachekey_binary {
    ($T:ty) => {
        impl CacheKey for $T {
            type CK = Vec<u8>;

            #[inline]
            fn cache_key<'a>(v: &<$T as redb::Value>::SelfType<'a>) -> Self::CK
            where
                $T: 'a,
            {
                <$T as BinaryCodec>::as_le_bytes(v)
            }
        }
    };
}

/// Generates CacheKey impl for types encoded with bincode.
/// Example: pub struct MyStruct { ... } (must impl bincode::Encode + Decode)
#[macro_export]
macro_rules! impl_cachekey_bincode {
    ($T:ty) => {
        impl CacheKey for $T {
            type CK = Vec<u8>;

            #[inline]
            fn cache_key<'a>(v: &<$T as redb::Value>::SelfType<'a>) -> Self::CK
            where
                $T: 'a,
            {
                bincode::encode_to_vec(v, bincode::config::standard()).unwrap()
            }
        }
    };
}

/// Generates CacheKey impl for Vec<u8> newtypes.
/// Example: pub struct Blob(pub Vec<u8>);
#[macro_export]
macro_rules! impl_cachekey_vec {
    ($T:ty) => {
        impl CacheKey for $T {
            type CK = Vec<u8>;

            #[inline]
            fn cache_key<'a>(v: &<$T as redb::Value>::SelfType<'a>) -> Self::CK
            where
                $T: 'a,
            {
                v.0.clone()
            }
        }
    };
}
