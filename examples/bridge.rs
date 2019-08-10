use std::collections::HashSet;

use queen::port::*;
use nson::msg;

fn main() {
    let mut chans = HashSet::new();

    chans.insert("aaa".to_owned());

    let config = BridgeConfig {
        addr1: queen::net::Addr::tcp("127.0.0.1:8888").unwrap(),
        auth_msg1: msg!{},
        hmac_key1: None,
        addr2: queen::net::Addr::tcp("127.0.0.1:8889").unwrap(),
        auth_msg2: msg!{},
        white_list: chans,
        hmac_key2: Some("queen".to_string())
    };

    let mut bridge = Bridge::connect(config);

    bridge.run().unwrap();
}