use std::thread;
use std::net::TcpStream;
use std::time::Duration;
use std::io::ErrorKind::WouldBlock;

use queen::{Node, node::NodeConfig};
use queen::nson::{msg, Message, message_id::MessageId};
use queen::error::ErrorCode;
use queen::util::{write_socket, read_socket};
use queen::crypto::{Method, Aead};

use super::get_free_addr;

#[test]
fn duplicate() {
    let addr = get_free_addr();

    let addr2 = addr.clone();
    thread::spawn(move || {
        let mut config = NodeConfig::new();

        config.add_tcp(addr2).unwrap();
        config.set_aead_key("queen");

        let mut node = Node::bind(config, ()).unwrap();

        node.run().unwrap();
    });

    thread::sleep(Duration::from_secs(1));

    // client 1
    let mut socket = TcpStream::connect(&addr).unwrap();
    let mut aead = Aead::new(&Method::default(), b"queen");

    // client2
    let mut socket2 = TcpStream::connect(&addr).unwrap();
    let mut aead2 = Aead::new(&Method::default(), b"queen");

    // client3
    let mut socket3 = TcpStream::connect(addr).unwrap();
    let mut aead3 = Aead::new(&Method::default(), b"queen");

    // client 1 auth
    let msg = msg!{
        "_chan": "_auth",
        "username": "aaa",
        "password": "bbb",
        "_ptid": MessageId::with_string("5932a005b4b4b4ac168cd9e4").unwrap()
    };

    let data = msg.to_vec().unwrap();
    write_socket(&mut socket, &mut aead, data).unwrap();
    let read_data = read_socket(&mut socket, &mut aead).unwrap();
    let recv = Message::from_slice(&read_data).unwrap();

    assert!(recv.get_i32("ok").unwrap() == 0);


    // client 2 auth
    let msg = msg!{
        "_chan": "_auth",
        "username": "aaa",
        "password": "bbb",
        "_ptid": MessageId::with_string("5932a005b4b4b4ac168cd9e5").unwrap()
    };

    let data = msg.to_vec().unwrap();
    write_socket(&mut socket2, &mut aead2, data).unwrap();
    let read_data = read_socket(&mut socket2, &mut aead2).unwrap();
    let recv = Message::from_slice(&read_data).unwrap();

    assert!(recv.get_i32("ok").unwrap() == 0);

    // client 3 auth
    let msg = msg!{
        "_chan": "_auth",
        "username": "aaa",
        "password": "bbb",
        "_ptid": MessageId::with_string("5932a005b4b4b4ac168cd9e5").unwrap()
    };

    let data = msg.to_vec().unwrap();
    write_socket(&mut socket3, &mut aead3, data).unwrap();
    let read_data = read_socket(&mut socket3, &mut aead3).unwrap();
    let recv = Message::from_slice(&read_data).unwrap();

    assert!(ErrorCode::has_error(&recv) == Some(ErrorCode::DuplicatePortId));
}

#[test]
fn port_to_port() {
    let addr = get_free_addr();

    let addr2 = addr.clone();
    thread::spawn(move || {
        let mut config = NodeConfig::new();

        config.add_tcp(addr2).unwrap();
        config.set_aead_key("queen");

        let mut node = Node::bind(config, ()).unwrap();

        node.run().unwrap();
    });

    thread::sleep(Duration::from_secs(1));

    // client 1
    let mut socket = TcpStream::connect(&addr).unwrap();
    let mut aead = Aead::new(&Method::default(), b"queen");

    // client2
    let mut socket2 = TcpStream::connect(&addr).unwrap();
    let mut aead2 = Aead::new(&Method::default(), b"queen");

    // client3
    let mut socket3 = TcpStream::connect(addr).unwrap();
    let mut aead3 = Aead::new(&Method::default(), b"queen");

    // client 1 auth
    let msg = msg!{
        "_chan": "_auth",
        "username": "aaa",
        "password": "bbb",
        "_ptid": MessageId::with_string("5932a005b4b4b4ac168cd9e4").unwrap()
    };

    let data = msg.to_vec().unwrap();
    write_socket(&mut socket, &mut aead, data).unwrap();
    let read_data = read_socket(&mut socket, &mut aead).unwrap();
    let recv = Message::from_slice(&read_data).unwrap();

    assert!(recv.get_i32("ok").unwrap() == 0);

    // client 2 auth
    let msg = msg!{
        "_chan": "_auth",
        "username": "aaa",
        "password": "bbb",
        "_ptid": MessageId::with_string("5932a005b4b4b4ac168cd9e5").unwrap()
    };

    let data = msg.to_vec().unwrap();
    write_socket(&mut socket2, &mut aead2, data).unwrap();
    let read_data = read_socket(&mut socket2, &mut aead2).unwrap();
    let recv = Message::from_slice(&read_data).unwrap();

    assert!(recv.get_i32("ok").unwrap() == 0);

    // client 3 auth
    let msg = msg!{
        "_chan": "_auth",
        "username": "aaa",
        "password": "bbb",
        "_ptid": MessageId::with_string("5932a005b4b4b4ac168cd9e6").unwrap()
    };

    let data = msg.to_vec().unwrap();
    write_socket(&mut socket3, &mut aead3, data).unwrap();
    let read_data = read_socket(&mut socket3, &mut aead3).unwrap();
    let recv = Message::from_slice(&read_data).unwrap();

    assert!(recv.get_i32("ok").unwrap() == 0);

    // client 1 attach
    let msg = msg!{
        "_chan": "_atta",
        "_valu": "aaa"
    };

    let data = msg.to_vec().unwrap();
    write_socket(&mut socket, &mut aead, data).unwrap();
    let read_data = read_socket(&mut socket, &mut aead).unwrap();
    let recv = Message::from_slice(&read_data).unwrap();

    assert!(recv.get_i32("ok").unwrap() == 0);

    // client 2 attach
    let msg = msg!{
        "_chan": "_atta",
        "_valu": "aaa"
    };

    let data = msg.to_vec().unwrap();
    write_socket(&mut socket2, &mut aead2, data).unwrap();
    let read_data = read_socket(&mut socket2, &mut aead2).unwrap();
    let recv = Message::from_slice(&read_data).unwrap();

    assert!(recv.get_i32("ok").unwrap() == 0);

    // client 3 try send
    let msg = msg!{
        "_chan": "aaa",
        "hello": "world",
        "_ack": 123,
        "_to": MessageId::with_string("5932a005b4b4b4ac168cd9e7").unwrap()
    };

    let data = msg.to_vec().unwrap();
    write_socket(&mut socket3, &mut aead3, data).unwrap();
    let read_data = read_socket(&mut socket3, &mut aead3).unwrap();
    let recv = Message::from_slice(&read_data).unwrap();

    assert!(ErrorCode::has_error(&recv) == Some(ErrorCode::TargetPortIdNotExist));

    // client 3 try send
    let msg = msg!{
        "_chan": "aaa",
        "hello": "world",
        "_ack": 123,
        "_to": MessageId::with_string("5932a005b4b4b4ac168cd9e4").unwrap()
    };

    let data = msg.to_vec().unwrap();
    write_socket(&mut socket3, &mut aead3, data).unwrap();
    let read_data = read_socket(&mut socket3, &mut aead3).unwrap();
    let recv = Message::from_slice(&read_data).unwrap();

    println!("{:?}", recv);
    assert!(recv.get_i32("ok").unwrap() == 0);

    // client 1 try recv
    let data = read_socket(&mut socket, &mut aead).unwrap();
    let recv = Message::from_slice(&data).unwrap();
    assert!(recv.get_str("hello").unwrap() == "world");

    //client 2 try recv
    socket2.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
    let recv = read_socket(&mut socket2, &mut aead2);
    match recv {
       Ok(recv) => panic!("{:?}", recv),
       Err(err) => {
            if let WouldBlock = err.kind() {
                // pass
            } else {
                panic!("{:?}", err);
            }        
       }
    }
}