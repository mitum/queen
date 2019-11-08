#![allow(unused_imports)]

use std::net::TcpStream;
use std::time::{Instant, Duration};
use std::thread;
use std::io::{Write, Read};

use queen::nson::msg;
use queen::nson::Message;
use queen::util::message::{write_socket, read_socket, read_socket_no_aead};
use queen::crypto::{Method, Aead};
use queen::dict::*;
use queen::port::{Port, Connector};
use queen::net::{NetStream, Listen, Addr};


fn main() {
    let crypto = (Method::Aes256Gcm, "hahaha".to_string());

    let port = Port::connect(Connector::Net(Addr::tcp("127.0.0.1:8888").unwrap(), Some(crypto)), msg!{}).unwrap();

    loop {
        port.send("aaa", msg!{"hello": "world"}, None);
    }

    thread::sleep(Duration::from_secs(60 * 10));
}