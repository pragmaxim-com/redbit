pub fn serde_encoding(binary_encoding: &str) -> &str {
    match binary_encoding {
        "hex"       => "serde_with::hex::Hex",
        "base64"    => "serde_with::base64::Base64",
        "btc_addr"  => "redbit::serde_enc::Btc",
        "base58"    => "redbit::serde_enc::Base58",
        "bech32"    => "redbit::serde_enc::Bech32",
        _ => panic!(
            "Unknown encoding '{}'. Expected 'hex', 'base64', 'base58', 'bech32' or 'btc_addr'.",
            binary_encoding
        ),
    }
}
