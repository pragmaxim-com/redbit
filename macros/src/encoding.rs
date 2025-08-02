pub fn serde_encoding(binary_encoding: &str) -> &str {
    match binary_encoding {
        "hex"    => "serde_with::hex::Hex",
        "btc_addr"    => "crate::serde_enc::Btc",
        "base64" => "serde_with::base64::Base64",
        "base58" => "crate::serde_enc::Base58",
        "bech32" => "crate::serde_enc::Bech32",
        _ => panic!(
            "Unknown encoding '{}'. Expected 'hex', 'base64', 'base58', 'bech32' or 'btc'.",
            binary_encoding
        ),
    }
}
