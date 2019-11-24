use std::time::Duration;
use std::thread;

use nson::{msg, MessageId};

use queen::{Queen, Rpc, Connector, Node};
use queen::crypto::Method;
use queen::net::Addr;

use super::get_free_addr;

#[test]
fn connect_queen() {
    let queen = Queen::new(MessageId::new(), (), None).unwrap();

    let rpc1 = Rpc::new(
        MessageId::new(),
        Connector::Queen(queen.clone(), msg!{}),
        msg!{"user": "test-user", "pass": "test-pass"},
        2
        ).unwrap();

    rpc1.add("hello", None, |_message| {
        // println!("{:?}", message);
        msg!{"hehehe": "lalala"}
    });

    let rpc2 = Rpc::new(
        MessageId::new(),
        Connector::Queen(queen, msg!{}),
        msg!{"user": "test-user", "pass": "test-pass"},
        2
        ).unwrap();

    let res = rpc2.call("hello", None, msg!{"hello": "owlrd"}, Some(<Duration>::from_secs(2)));
    assert!(res.is_ok());
}

#[test]
fn connect_node() {
    let queen = Queen::new(MessageId::new(), (), None).unwrap();

    let crypto = (Method::Aes256Gcm, "sep-centre".to_string());
    let addr = get_free_addr();

    let crypto2 = crypto.clone();
    let addr2 = addr.clone();
    thread::spawn(move || {
        let mut node = Node::new(
            queen,
            2,
            vec![Addr::tcp(&addr2).unwrap()],
            Some(crypto2)
        ).unwrap();

        node.run().unwrap();
    });

    let rpc1 = Rpc::new(
        MessageId::new(),
        Connector::Net(Addr::tcp(&addr).unwrap(),Some(crypto.clone())),
        msg!{"user": "test-user", "pass": "test-pass"},
        2
        ).unwrap();

    rpc1.add("hello", None, |_message| {
        // println!("{:?}", message);
        msg!{"hehehe": "lalala"}
    });

    let rpc2 = Rpc::new(
        MessageId::new(),
        Connector::Net(Addr::tcp(&addr).unwrap(),Some(crypto.clone())),
        msg!{"user": "test-user", "pass": "test-pass"},
        2
        ).unwrap();

    let res = rpc2.call("hello", None, msg!{"hello": "owlrd"}, Some(<Duration>::from_secs(2)));
    assert!(res.is_ok());
}

#[test]
fn connect_mulit_node() {
    let queen = Queen::new(MessageId::new(), (), None).unwrap();

    let crypto = (Method::Aes256Gcm, "sep-centre".to_string());
    let addr1 = get_free_addr();
    let addr2 = get_free_addr();

    let queen2 = queen.clone();
    let crypto2 = crypto.clone();
    let addr = addr1.clone();
    thread::spawn(move || {
        let mut node = Node::new(
            queen2,
            2,
            vec![Addr::tcp(&addr).unwrap()],
            Some(crypto2)
        ).unwrap();

        node.run().unwrap();
    });

    let crypto2 = crypto.clone();
    let addr = addr2.clone();
    thread::spawn(move || {
        let mut node = Node::new(
            queen,
            2,
            vec![Addr::tcp(&addr).unwrap()],
            Some(crypto2)
        ).unwrap();

        node.run().unwrap();
    });

    let rpc1 = Rpc::new(
        MessageId::new(),
         Connector::Net(Addr::tcp(&addr1).unwrap(),Some(crypto.clone())),
        msg!{"user": "test-user", "pass": "test-pass"},
        2
        ).unwrap();

    rpc1.add("hello", None, |_message| {
        // println!("{:?}", message);
        msg!{"hehehe": "lalala"}
    });

    let rpc2 = Rpc::new(
        MessageId::new(),
        Connector::Net(Addr::tcp(&addr2).unwrap(),Some(crypto)),
        msg!{"user": "test-user", "pass": "test-pass"},
        2
        ).unwrap();

    let res = rpc2.call("hello", None, msg!{"hello": "owlrd"}, Some(Duration::from_secs(2)));
    assert!(res.is_ok());
}
