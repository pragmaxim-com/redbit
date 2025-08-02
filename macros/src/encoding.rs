pub fn serde_encoding(binary_encoding: &str) -> &str {
    match binary_encoding {
        "hex"            => "serde_with::hex::Hex",
        "base64"         => "serde_with::base64::Base64",
        "btc_addr"       => "redbit::btc_serde_enc::Btc",
        "btc_base58"     => "redbit::btc_serde_enc::BtcBase58",
        "btc_bech32"     => "redbit::btc_serde_enc::BtcBech32",
        "cardano_addr"   => "redbit::cardano_serde_enc::Cardano",
        "cardano_base58" => "redbit::cardano_serde_enc::CardanoBase58",
        "cardano_bech32" => "redbit::cardano_serde_enc::CardanoBech32",

        _ => panic!(
            "Unknown encoding '{}'. Expected 'hex', 'base64', 'btc_base58', 'btc_bech32', 'btc_addr', 'cardano_base58', 'cardano_bech32', 'cardano_addr'.",
            binary_encoding
        ),
    }
}
