use std::fmt::Display;
use redbit::utoipa;

#[derive(Clone, Eq, Ord, PartialEq, PartialOrd, Debug, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct Hash(pub String);
impl Default for Hash {
    fn default() -> Self {
        Hash("foo-hash".to_string())
    }
}
impl Display for Hash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Eq, Ord, PartialEq, PartialOrd, Debug, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct Address(pub String);
impl Default for Address {
    fn default() -> Self {
        Address("foo-address".to_string())
    }
}
impl Display for Address {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Eq, Ord, PartialEq, PartialOrd, Debug, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct PolicyId(pub String);
impl Default for PolicyId {
    fn default() -> Self {
        PolicyId("foo-policy".to_string())
    }
}
impl Display for PolicyId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Eq, Ord, PartialEq, PartialOrd, Debug, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct AssetName(pub String);
impl Default for AssetName {
    fn default() -> Self {
        AssetName("foo-asset".to_string())
    }
}
impl Display for AssetName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Eq, Ord, PartialEq, PartialOrd, Debug, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct Datum(pub String);
impl Default for Datum {
    fn default() -> Self {
        Datum("foo-datum".to_string())
    }
}
impl Display for Datum {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
