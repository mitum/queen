#![allow(unused)]
use std::net::TcpStream;
use std::io::{Read, Write};


use queen::{Queen, Context};
use queen::nson::msg;
use queen::nson::Message;

use queen_log;
use log::LevelFilter;
use log::{debug, error, info, warn, trace};

fn main() {
    let mut stream = TcpStream::connect("127.0.0.1:8888").unwrap();

    let msg = msg!{
        "event": "node:hand",
        "username": "aaa",
        "password": "bbb",
        "su": true
    };

    msg.encode(&mut stream).unwrap();

    let recv = Message::decode(&mut stream).unwrap();

    println!("{:?}", recv);

    let msg = msg!{
        "event": "node:attach",
        "value": "net:listen"
    };

    msg.encode(&mut stream).unwrap();

    let recv = Message::decode(&mut stream).unwrap();

    println!("{:?}", recv);

    let msg = msg!{
        "event": "node:attach",
        "value": "pub:hello"
    };

    msg.encode(&mut stream).unwrap();

    let recv = Message::decode(&mut stream).unwrap();

    println!("{:?}", recv);

    // let msg = msg!{
    //     "e": "s:d",
    //     "v": "p:hello"
    // };

    // msg.encode(&mut stream).unwrap();

    // let recv = Message::decode(&mut stream).unwrap();

    // println!("{:?}", recv);

    loop {
        let recv = Message::decode(&mut stream).unwrap();
        println!("{:?}", recv);
    }
}