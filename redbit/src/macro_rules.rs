
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

#[macro_export]
macro_rules! impl_utoipa_partial_schema {
    ($Struct:ty, $schema_type:expr, $schema_example:expr, $schema_extensions:expr) => {
        impl utoipa::PartialSchema for $Struct {
            fn schema() -> utoipa::openapi::RefOr<utoipa::openapi::schema::Schema> {
                rest::schema($schema_type, $schema_example, $schema_extensions)
            }
        }
    };
}

#[macro_export]
macro_rules! impl_utoipa_to_schema {
    // No parent: just push this type's schema
    ($Struct:ty) => {
        impl utoipa::ToSchema for $Struct {
            fn schemas(schemas: &mut Vec<(String, utoipa::openapi::RefOr<utoipa::openapi::schema::Schema>)>) {
                schemas.push((
                    stringify!($Struct).to_string(),
                    < $Struct as utoipa::PartialSchema>::schema()
                ));
            }
        }
    };

    // With parent: push this schema, then forward to parent
    ($Struct:ty, $Parent:ty) => {
        impl utoipa::ToSchema for $Struct {
            fn schemas(schemas: &mut Vec<(String, utoipa::openapi::RefOr<utoipa::openapi::schema::Schema>)>) {
                schemas.push((
                    stringify!($Struct).to_string(),
                    < $Struct as utoipa::PartialSchema>::schema()
                ));
                < $Parent as utoipa::ToSchema>::schemas(schemas);
            }
        }
    };
}

#[macro_export]
macro_rules! impl_binary_codec {
    // Single-field newtype
    ($Struct:ident, $Inner:ty) => {
        impl BinaryCodec for $Struct {
            fn from_le_bytes(bytes: &[u8]) -> Self {
                let arr: [u8; std::mem::size_of::<$Inner>()] = bytes.try_into().expect("invalid byte length");
                Self(<$Inner>::from_le_bytes(arr))
            }
            fn as_le_bytes(&self) -> Vec<u8> {
                self.0.to_le_bytes().to_vec()
            }
            fn size() -> usize {
                std::mem::size_of::<$Inner>()
            }
        }
    };

    // Composite: parent + index
    ($Struct:ident, $Parent:ty, $Index:ty, $parent_field:ident, $index_field:ident) => {
        impl BinaryCodec for $Struct {
            fn from_le_bytes(bytes: &[u8]) -> Self {
                let parent_size = <$Parent as BinaryCodec>::size();
                const index_size: usize = std::mem::size_of::<$Index>();
                assert_eq!(bytes.len(), parent_size + index_size, "invalid byte length");
                let (parent_bytes, index_bytes) = bytes.split_at(parent_size);
                let parent = <$Parent as BinaryCodec>::from_le_bytes(parent_bytes);
                let index_arr: [u8; index_size] = index_bytes.try_into().unwrap();
                let index = <$Index>::from_le_bytes(index_arr);
                $Struct { $parent_field: parent, $index_field: index }
            }
            fn as_le_bytes(&self) -> Vec<u8> {
                let mut buf = self.$parent_field.as_le_bytes();
                buf.extend_from_slice(&self.$index_field.to_le_bytes());
                buf
            }
            fn size() -> usize {
                <$Parent as BinaryCodec>::size() + std::mem::size_of::<$Index>()
            }
        }
    };
}

/// Implements IndexedPointer for either a single-field newtype or a composite struct.
#[macro_export]
macro_rules! impl_indexed_pointer {
    // Single-field newtype
    ($Struct:ident, $Index:ty) => {
        impl IndexedPointer for $Struct {
            type Index = $Index;

            fn index(&self) -> Self::Index { self.0 }
            fn next_index(&self) -> Self { $Struct(self.0 + 1) }
            fn nth_index(&self, n: usize) -> Self { $Struct(self.0 + n as $Index) }
            fn rollback_or_init(&self, n: u32) -> Self {
                let prev_index = self.0.checked_sub(n).unwrap_or(0);
                $Struct(prev_index)
            }
        }
    };

    // Composite struct: parent + index
    ($Struct:ident, $Index:ty, $parent_field:ident, $index_field:ident) => {
        impl IndexedPointer for $Struct {
            type Index = $Index;

            fn index(&self) -> Self::Index { self.$index_field }
            fn next_index(&self) -> Self {
                $Struct {
                    $parent_field: self.$parent_field.clone(),
                    $index_field: self.$index_field + 1,
                }
            }
            fn nth_index(&self, n: usize) -> Self {
                $Struct {
                    $parent_field: self.$parent_field.clone(),
                    $index_field: self.$index_field + n as $Index,
                }
            }
            fn rollback_or_init(&self, n: u32) -> Self {
                $Struct {
                    $parent_field: self.$parent_field.rollback_or_init(n),
                    $index_field: 0,
                }
            }
        }
    };
}

#[macro_export]
macro_rules! impl_root_pointer {
    ($Struct:ident, $Index:ty) => {
        impl RootPointer for $Struct {
            fn total_index(&self) -> u128 {
                self.index().into()
            }

            fn is_pointer(&self) -> bool {
                false
            }

            fn from_many(pks: &[$Index]) -> Vec<Self> {
                pks.iter().map(|idx| $Struct(*idx)).collect()
            }

            fn depth(&self) -> usize {
                0
            }
        }
    };
}

/// Implements ChildPointer for structs with a parent field and an index field.
#[macro_export]
macro_rules! impl_child_pointer {
    ($Struct:ident, $Parent:ty, $parent_field:ident, $Index:ty, $index_field:ident) => {
        impl ChildPointer for $Struct {
            type Parent = $Parent;

            fn is_pointer(&self) -> bool {
                true
            }

            fn parent(&self) -> Self::Parent {
                self.$parent_field
            }

            fn from_parent($parent_field: Self::Parent, $index_field: $Index) -> Self {
                $Struct {
                    $parent_field,
                    $index_field,
                }
            }

            fn total_index(&self) -> u128 {
                let parent_total = self.parent().total_index();
                let idx: u128 = self.index().into();
                parent_total * 3 + idx
            }

            fn depth(&self) -> usize {
                1 + self.parent().depth()
            }
        }
    };
}

/// Implements TryFrom<String> + FromStr for a root pointer (no parent).
#[macro_export]
macro_rules! impl_tryfrom_pointer {
    ($Struct:ident, $Index:ty) => {
        impl TryFrom<String> for $Struct {
            type Error = ParsePointerError;

            fn try_from(s: String) -> Result<Self, Self::Error> {
                if s.contains('-') {
                    return Err(ParsePointerError::Format);
                }
                let idx = s.parse::<$Index>()?;
                Ok($Struct(idx))
            }
        }

        impl std::str::FromStr for $Struct {
            type Err = ParsePointerError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Self::try_from(s.to_string())
            }
        }
    };
    ($Struct:ident, $Parent:ty, $parent_field:ident, $Index:ty, $index_field:ident) => {
        impl TryFrom<String> for $Struct {
            type Error = ParsePointerError;

            fn try_from(s: String) -> Result<Self, Self::Error> {
                let mut parts = s.rsplitn(2, '-');
                let idx_str    = parts.next().ok_or(ParsePointerError::Format)?;
                let parent_str = parts.next().ok_or(ParsePointerError::Format)?;
                let parent     = parent_str.parse::<$Parent>()?;
                let idx        = idx_str.parse::<$Index>()?;
                Ok($Struct {
                    $parent_field: parent,
                    $index_field: idx,
                })
            }
        }

        impl core::str::FromStr for $Struct {
            type Err = ParsePointerError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Self::try_from(s.to_string())
            }
        }
    };
}
