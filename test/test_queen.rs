use std::time::Duration;
use std::thread;

use nson::{msg, MessageId};

use queen::{Queen, Hook, Client};
use queen::dict::*;
use queen::error::{ErrorCode, Error};

#[test]
fn conn() {
    let queen = Queen::new(MessageId::new(), ()).unwrap();

    for _ in 0..10000 {
        let _stream = queen.connect(msg!{}, None, None).unwrap();
    }

    struct MyHook;

    impl Hook for MyHook {
        fn accept(&self, _conn: &Client) -> bool {
            false
        }
    }

    let queen = Queen::new(MessageId::new(), MyHook).unwrap();

    let ret = queen.connect(msg!{}, None, None);
    assert!(ret.is_err());

    match ret {
        Ok(_) => unreachable!(),
        Err(err) => {
            assert!(matches!(err, Error::ConnectionRefused(_)));
        }
    }
}

#[test]
fn connect() {
    let queen = Queen::new(MessageId::new(), ()).unwrap();

    let stream1 = queen.connect(msg!{}, None, None).unwrap();
    let stream2 = queen.connect(msg!{}, None, None).unwrap();

    let _ = stream1.send(msg!{
        CHAN: PING
    });

    let _ = stream2.send(msg!{
        CHAN: PING
    });

    thread::sleep(Duration::from_millis(100));

    assert!(stream1.recv().unwrap().get_i32(OK).unwrap() == 0);
    assert!(stream2.recv().unwrap().get_i32(OK).unwrap() == 0);

    let _ = stream1.send(msg!{
        "aaa": "bbb"
    });

    let _ = stream2.send(msg!{
        CHAN: 123
    });

    thread::sleep(Duration::from_millis(100));

    let recv = stream1.recv().unwrap();

    assert!(ErrorCode::has_error(&recv) == Some(ErrorCode::CannotGetChanField));

    let recv = stream2.recv().unwrap();

    assert!(ErrorCode::has_error(&recv) == Some(ErrorCode::CannotGetChanField));
}

#[test]
fn auth() {
    let queen = Queen::new(MessageId::new(), ()).unwrap();

    let stream1 = queen.connect(msg!{}, None, None).unwrap();

    let _ = stream1.send(msg!{
        CHAN: ATTACH
    });

    thread::sleep(Duration::from_millis(100));

    let recv = stream1.recv().unwrap();

    assert!(ErrorCode::has_error(&recv) == Some(ErrorCode::Unauthorized));

    let _ = stream1.send(msg!{
        CHAN: AUTH
    });

    thread::sleep(Duration::from_millis(100));

    let recv = stream1.recv().unwrap();

    assert!(recv.get_i32(OK).unwrap() == 0);
    assert!(recv.get_message_id(CLIENT_ID).is_ok());
    assert!(recv.get(LABEL).is_none());

    let _ = stream1.send(msg!{
        CHAN: AUTH,
        LABEL: msg!{
            "aa": "bb"
        }
    });

    thread::sleep(Duration::from_millis(100));

    let recv = stream1.recv().unwrap();

    assert!(recv.get_i32(OK).unwrap() == 0);
    assert!(recv.get_message_id(CLIENT_ID).is_ok());
    assert!(recv.get_message(LABEL).unwrap() == &msg!{"aa": "bb"});

    let client_id = MessageId::new();

    let _ = stream1.send(msg!{
        CHAN: AUTH,
        CLIENT_ID: client_id.clone()
    });

    thread::sleep(Duration::from_millis(100));

    let recv = stream1.recv().unwrap();

    assert!(recv.get_i32(OK).unwrap() == 0);
    assert!(recv.get_message_id(CLIENT_ID).unwrap() == &client_id);
    assert!(recv.get_message(LABEL).unwrap() == &msg!{"aa": "bb"});

    let _ = stream1.send(msg!{
        CHAN: AUTH,
        LABEL: msg!{
            "cc": "dd"
        }
    });

    thread::sleep(Duration::from_millis(100));

    let recv = stream1.recv().unwrap();

    assert!(recv.get_i32(OK).unwrap() == 0);
    assert!(recv.get_message_id(CLIENT_ID).is_ok());
    assert!(recv.get_message(LABEL).unwrap() == &msg!{"cc": "dd"});

    // stream2
    let stream2 = queen.connect(msg!{}, None, None).unwrap();
    let _ = stream2.send(msg!{
        CHAN: AUTH,
        CLIENT_ID: client_id.clone()
    });

    thread::sleep(Duration::from_millis(100));

    let recv = stream2.recv().unwrap();

    assert!(ErrorCode::has_error(&recv) == Some(ErrorCode::DuplicateClientId));

    let _ = stream2.send(msg!{
        CHAN: MINE,
    });

    thread::sleep(Duration::from_millis(100));

    let recv = stream2.recv().unwrap();

    assert!(recv.get_i32(OK).unwrap() == 0);

    let value = recv.get_message(VALUE).unwrap();

    assert!(value.get_bool(AUTH).unwrap() == false);
    assert!(value.get_bool(ROOT).unwrap() == false);
    assert!(value.get_message(CHANS).unwrap().is_empty());
    assert!(value.get_message_id(CLIENT_ID).unwrap() != &client_id);
    assert!(value.get_u64(SEND_MESSAGES).unwrap() == 1);
    assert!(value.get_u64(RECV_MESSAGES).unwrap() == 2);

    drop(stream1);

    // stream 2 auth again
    let _ = stream2.send(msg!{
        CHAN: AUTH,
        CLIENT_ID: client_id.clone()
    });

    thread::sleep(Duration::from_millis(100));

    let recv = stream2.recv().unwrap();

    assert!(recv.get_i32(OK).unwrap() == 0);
    assert!(recv.get_message_id(CLIENT_ID).unwrap() == &client_id);
}

#[test]
fn attach_detach() {
    let queen = Queen::new(MessageId::new(), ()).unwrap();

    let stream1 = queen.connect(msg!{}, None, None).unwrap();
    let stream2 = queen.connect(msg!{}, None, None).unwrap();

    // auth
    let _ = stream1.send(msg!{
        CHAN: AUTH
    });

    let _ = stream2.send(msg!{
        CHAN: AUTH
    });

    thread::sleep(Duration::from_millis(100));

    assert!(stream1.recv().unwrap().get_i32(OK).unwrap() == 0);
    assert!(stream2.recv().unwrap().get_i32(OK).unwrap() == 0);

    // try send
    let _ = stream1.send(msg!{
        CHAN: "aaa",
        "hello": "world",
        ACK: "123"
    });

    thread::sleep(Duration::from_millis(100));

    let recv = stream1.recv().unwrap();

    assert!(ErrorCode::has_error(&recv) == Some(ErrorCode::NoConsumers));
    assert!(recv.get(FROM).is_none());

    // attach
    let _ = stream2.send(msg!{
        CHAN: ATTACH,
        VALUE: "aaa"
    });

    thread::sleep(Duration::from_millis(100));

    // send
    let _ = stream1.send(msg!{
        CHAN: "aaa",
        "hello": "world",
        ACK: "123"
    });

    thread::sleep(Duration::from_millis(100));

    let recv = stream1.recv().unwrap();

    assert!(recv.get_i32(OK).unwrap() == 0);

    let recv = stream2.recv().unwrap();

    assert!(recv.get_i32(OK).unwrap() == 0);

    // recv
    let recv = stream2.recv().unwrap();

    assert!(recv.get_str(CHAN).unwrap() == "aaa");
    assert!(recv.get_str("hello").unwrap() == "world");
    assert!(recv.get(FROM).is_some());

    // detach
    let _ = stream2.send(msg!{
        CHAN: DETACH,
        VALUE: "aaa"
    });

    // send
    let _ = stream1.send(msg!{
        CHAN: "aaa",
        "hello": "world",
        ACK: "123"
    });

    thread::sleep(Duration::from_millis(100));

    let recv = stream2.recv().unwrap();

    assert!(recv.get_i32(OK).unwrap() == 0);

    let recv = stream1.recv().unwrap();

    assert!(ErrorCode::has_error(&recv) == Some(ErrorCode::NoConsumers));
}

#[test]
fn label() {
    let queen = Queen::new(MessageId::new(), ()).unwrap();

    let stream1 = queen.connect(msg!{}, None, None).unwrap();
    let stream2 = queen.connect(msg!{}, None, None).unwrap();
    let stream3 = queen.connect(msg!{}, None, None).unwrap();

    // auth
    let _ = stream1.send(msg!{
        CHAN: AUTH
    });

    let _ = stream2.send(msg!{
        CHAN: AUTH
    });

    let _ = stream3.send(msg!{
        CHAN: AUTH
    });

    thread::sleep(Duration::from_millis(100));

    assert!(stream1.recv().unwrap().get_i32(OK).unwrap() == 0);
    assert!(stream2.recv().unwrap().get_i32(OK).unwrap() == 0);
    assert!(stream3.recv().unwrap().get_i32(OK).unwrap() == 0);

    // attach
    let _ = stream1.send(msg!{
        CHAN: ATTACH,
        VALUE: "aaa"
    });

    let _ = stream2.send(msg!{
        CHAN: ATTACH,
        VALUE: "aaa",
        LABEL: "label1"
    });

    // send
    let _ = stream3.send(msg!{
        CHAN: "aaa",
        "hello": "world",
        ACK: "123"
    });

    thread::sleep(Duration::from_millis(100));

    assert!(stream1.recv().unwrap().get_i32(OK).unwrap() == 0);
    assert!(stream2.recv().unwrap().get_i32(OK).unwrap() == 0);
    assert!(stream3.recv().unwrap().get_i32(OK).unwrap() == 0);

    // recv
    let recv = stream1.recv().unwrap();

    assert!(recv.get_str(CHAN).unwrap() == "aaa");
    assert!(recv.get_str("hello").unwrap() == "world");
    assert!(recv.get(FROM).is_some());

    let recv = stream2.recv().unwrap();

    assert!(recv.get_str(CHAN).unwrap() == "aaa");
    assert!(recv.get_str("hello").unwrap() == "world");
    assert!(recv.get(FROM).is_some());

    // send with label
    let _ = stream3.send(msg!{
        CHAN: "aaa",
        "hello": "world",
        LABEL: "label1",
        ACK: "123"
    });

    thread::sleep(Duration::from_millis(100));

    assert!(stream3.recv().unwrap().get_i32(OK).unwrap() == 0);

    // recv
    assert!(stream1.recv().is_err());

    let recv = stream2.recv().unwrap();

    assert!(recv.get_str(CHAN).unwrap() == "aaa");
    assert!(recv.get_str("hello").unwrap() == "world");

    // detach
    let _ = stream2.send(msg!{
        CHAN: DETACH,
        VALUE: "aaa",
        LABEL: "label1"
    });

    thread::sleep(Duration::from_millis(100));

    assert!(stream2.recv().unwrap().get_i32(OK).unwrap() == 0);

    // send with label
    let _ = stream3.send(msg!{
        CHAN: "aaa",
        "hello": "world",
        LABEL: "label1",
        ACK: "123"
    });

    thread::sleep(Duration::from_millis(100));

    let recv = stream3.recv().unwrap();

    assert!(ErrorCode::has_error(&recv) == Some(ErrorCode::NoConsumers));

    // send
    let _ = stream3.send(msg!{
        CHAN: "aaa",
        "hello": "world",
        ACK: "123"
    });

    thread::sleep(Duration::from_millis(100));

    assert!(stream3.recv().unwrap().get_i32(OK).unwrap() == 0);

    // recv
    let recv = stream1.recv().unwrap();

    assert!(recv.get_str(CHAN).unwrap() == "aaa");
    assert!(recv.get_str("hello").unwrap() == "world");

    let recv = stream2.recv().unwrap();

    assert!(recv.get_str(CHAN).unwrap() == "aaa");
    assert!(recv.get_str("hello").unwrap() == "world");

    let stream4 = queen.connect(msg!{}, None, None).unwrap();

    // auth
    let _ = stream4.send(msg!{
        CHAN: AUTH
    });

    // attach
    let _ = stream4.send(msg!{
        CHAN: ATTACH,
        VALUE: "aaa",
        LABEL: "label1"
    });

    let _ = stream4.send(msg!{
        CHAN: ATTACH,
        VALUE: "aaa",
        LABEL: "label2"
    });

    thread::sleep(Duration::from_millis(100));

    assert!(stream4.recv().unwrap().get_i32(OK).unwrap() == 0);
    assert!(stream4.recv().unwrap().get_i32(OK).unwrap() == 0);
    assert!(stream4.recv().unwrap().get_i32(OK).unwrap() == 0);

    // send with label
    let _ = stream3.send(msg!{
        CHAN: "aaa",
        "hello": "world",
        LABEL: "label1",
        ACK: "123"
    });

    thread::sleep(Duration::from_millis(100));

    assert!(stream3.recv().unwrap().get_i32(OK).unwrap() == 0);

    // recv
    let recv = stream4.recv().unwrap();

    assert!(recv.get_str(CHAN).unwrap() == "aaa");
    assert!(recv.get_str("hello").unwrap() == "world");

    // detach
    let _ = stream4.send(msg!{
        CHAN: DETACH,
        VALUE: "aaa",
        LABEL: "label1"
    });

    thread::sleep(Duration::from_millis(100));

    assert!(stream4.recv().unwrap().get_i32(OK).unwrap() == 0);

    // send with label
    let _ = stream3.send(msg!{
        CHAN: "aaa",
        "hello": "world",
        LABEL: "label2",
        ACK: "123"
    });

    thread::sleep(Duration::from_millis(100));

    assert!(stream3.recv().unwrap().get_i32(OK).unwrap() == 0);

    // recv
    let recv = stream4.recv().unwrap();

    assert!(recv.get_str(CHAN).unwrap() == "aaa");
    assert!(recv.get_str("hello").unwrap() == "world");
}

#[test]
fn labels() {
    let queen = Queen::new(MessageId::new(), ()).unwrap();

    let stream1 = queen.connect(msg!{}, None, None).unwrap();
    let stream2 = queen.connect(msg!{}, None, None).unwrap();
    let stream3 = queen.connect(msg!{}, None, None).unwrap();

    // auth
    let _ = stream1.send(msg!{
        CHAN: AUTH
    });

    let _ = stream2.send(msg!{
        CHAN: AUTH
    });

    let _ = stream3.send(msg!{
        CHAN: AUTH
    });

    thread::sleep(Duration::from_millis(100));

    assert!(stream1.recv().unwrap().get_i32(OK).unwrap() == 0);
    assert!(stream2.recv().unwrap().get_i32(OK).unwrap() == 0);
    assert!(stream3.recv().unwrap().get_i32(OK).unwrap() == 0);

    // attach
    let _ = stream1.send(msg!{
        CHAN: ATTACH,
        VALUE: "aaa",
        LABEL: ["label1", "label2", "label3", "label1"]
    });

    let _ = stream2.send(msg!{
        CHAN: ATTACH,
        VALUE: "aaa",
        LABEL: ["label2", "label3", "label4"]
    });

    // send
    let _ = stream3.send(msg!{
        CHAN: "aaa",
        "hello": "world",
        ACK: "123"
    });

    thread::sleep(Duration::from_millis(100));

    assert!(stream1.recv().unwrap().get_i32(OK).unwrap() == 0);
    assert!(stream2.recv().unwrap().get_i32(OK).unwrap() == 0);
    assert!(stream3.recv().unwrap().get_i32(OK).unwrap() == 0);

    // recv
    let recv = stream1.recv().unwrap();

    assert!(recv.get_str(CHAN).unwrap() == "aaa");
    assert!(recv.get_str("hello").unwrap() == "world");
    assert!(recv.get(FROM).is_some());

    let recv = stream2.recv().unwrap();

    assert!(recv.get_str(CHAN).unwrap() == "aaa");
    assert!(recv.get_str("hello").unwrap() == "world");
    assert!(recv.get(FROM).is_some());

    // send with label
    let _ = stream3.send(msg!{
        CHAN: "aaa",
        "hello": "world",
        LABEL: "label5",
        ACK: "123"
    });

    thread::sleep(Duration::from_millis(100));

    let recv = stream3.recv().unwrap();

    assert!(ErrorCode::has_error(&recv) == Some(ErrorCode::NoConsumers));

    // send with label
    let _ = stream3.send(msg!{
        CHAN: "aaa",
        "hello": "world",
        LABEL: ["label5", "label6"],
        ACK: "123"
    });

    thread::sleep(Duration::from_millis(100));

    let recv = stream3.recv().unwrap();

    assert!(ErrorCode::has_error(&recv) == Some(ErrorCode::NoConsumers));

    // send with label
    let _ = stream3.send(msg!{
        CHAN: "aaa",
        "hello": "world",
        LABEL: "label1",
        ACK: "123"
    });

    thread::sleep(Duration::from_millis(100));

    assert!(stream3.recv().unwrap().get_i32(OK).unwrap() == 0);

    // recv
    let recv = stream1.recv().unwrap();

    assert!(recv.get_str(CHAN).unwrap() == "aaa");
    assert!(recv.get_str("hello").unwrap() == "world");

    assert!(stream2.recv().is_err());

    // send with label
    let _ = stream3.send(msg!{
        CHAN: "aaa",
        "hello": "world",
        LABEL: ["label1"],
        ACK: "123"
    });

    thread::sleep(Duration::from_millis(100));

    assert!(stream3.recv().unwrap().get_i32(OK).unwrap() == 0);

    // recv
    let recv = stream1.recv().unwrap();

    assert!(recv.get_str(CHAN).unwrap() == "aaa");
    assert!(recv.get_str("hello").unwrap() == "world");

    assert!(stream2.recv().is_err());

    // send with label
    let _ = stream3.send(msg!{
        CHAN: "aaa",
        "hello": "world",
        LABEL: ["label1", "label5"],
        ACK: "123"
    });

    thread::sleep(Duration::from_millis(100));

    assert!(stream3.recv().unwrap().get_i32(OK).unwrap() == 0);

    // recv
    let recv = stream1.recv().unwrap();

    assert!(recv.get_str(CHAN).unwrap() == "aaa");
    assert!(recv.get_str("hello").unwrap() == "world");

    assert!(stream2.recv().is_err());

    // send with label
    let _ = stream3.send(msg!{
        CHAN: "aaa",
        "hello": "world",
        LABEL: ["label1", "label4"],
        ACK: "123"
    });

    thread::sleep(Duration::from_millis(100));

    assert!(stream3.recv().unwrap().get_i32(OK).unwrap() == 0);

    // recv
    let recv = stream1.recv().unwrap();

    assert!(recv.get_str(CHAN).unwrap() == "aaa");
    assert!(recv.get_str("hello").unwrap() == "world");

    let recv = stream2.recv().unwrap();

    assert!(recv.get_str(CHAN).unwrap() == "aaa");
    assert!(recv.get_str("hello").unwrap() == "world");

    // detach
    let _ = stream1.send(msg!{
        CHAN: DETACH,
        VALUE: "aaa",
        LABEL: "label1"
    });

    thread::sleep(Duration::from_millis(100));

    assert!(stream1.recv().unwrap().get_i32(OK).unwrap() == 0);

    // send with label
    let _ = stream3.send(msg!{
        CHAN: "aaa",
        "hello": "world",
        LABEL: ["label1", "label4"],
        ACK: "123"
    });

    thread::sleep(Duration::from_millis(100));

    assert!(stream3.recv().unwrap().get_i32(OK).unwrap() == 0);

    // recv
    assert!(stream1.recv().is_err());

    let recv = stream2.recv().unwrap();

    assert!(recv.get_str(CHAN).unwrap() == "aaa");
    assert!(recv.get_str("hello").unwrap() == "world");

    // detach
    let _ = stream2.send(msg!{
        CHAN: DETACH,
        VALUE: "aaa",
        LABEL: ["label2", "label3"]
    });

    thread::sleep(Duration::from_millis(100));

    assert!(stream2.recv().unwrap().get_i32(OK).unwrap() == 0);

    // send with label
    let _ = stream3.send(msg!{
        CHAN: "aaa",
        "hello": "world",
        LABEL: "label2",
        ACK: "123"
    });

    thread::sleep(Duration::from_millis(100));

    assert!(stream3.recv().unwrap().get_i32(OK).unwrap() == 0);

    // recv
    let recv = stream1.recv().unwrap();

    assert!(recv.get_str(CHAN).unwrap() == "aaa");
    assert!(recv.get_str("hello").unwrap() == "world");

    assert!(stream2.recv().is_err());

    // send with label
    let _ = stream3.send(msg!{
        CHAN: "aaa",
        "hello": "world",
        LABEL: ["label2", "label3"],
        ACK: "123"
    });

    thread::sleep(Duration::from_millis(100));

    assert!(stream3.recv().unwrap().get_i32(OK).unwrap() == 0);

    // recv
    let recv = stream1.recv().unwrap();

    assert!(recv.get_str(CHAN).unwrap() == "aaa");
    assert!(recv.get_str("hello").unwrap() == "world");

    assert!(stream2.recv().is_err());

    let stream4 = queen.connect(msg!{}, None, None).unwrap();

    // auth
    let _ = stream4.send(msg!{
        CHAN: AUTH
    });

    // attach
    let _ = stream4.send(msg!{
        CHAN: ATTACH,
        VALUE: "aaa",
        LABEL: ["label1", "label2"]
    });

    let _ = stream4.send(msg!{
        CHAN: ATTACH,
        VALUE: "aaa",
        LABEL: ["label3", "label4"]
    });

    thread::sleep(Duration::from_millis(100));

    assert!(stream4.recv().unwrap().get_i32(OK).unwrap() == 0);
    assert!(stream4.recv().unwrap().get_i32(OK).unwrap() == 0);
    assert!(stream4.recv().unwrap().get_i32(OK).unwrap() == 0);

    // send with label
    let _ = stream3.send(msg!{
        CHAN: "aaa",
        "hello": "world",
        LABEL: "label1",
        ACK: "123"
    });

    thread::sleep(Duration::from_millis(100));

    assert!(stream3.recv().unwrap().get_i32(OK).unwrap() == 0);

    // recv
    let recv = stream4.recv().unwrap();

    assert!(recv.get_str(CHAN).unwrap() == "aaa");
    assert!(recv.get_str("hello").unwrap() == "world");

    // detach
    let _ = stream4.send(msg!{
        CHAN: DETACH,
        VALUE: "aaa",
        LABEL: ["label2", "label3"]
    });

    thread::sleep(Duration::from_millis(100));

    assert!(stream4.recv().unwrap().get_i32(OK).unwrap() == 0);

    // send with label
    let _ = stream3.send(msg!{
        CHAN: "aaa",
        "hello": "world",
        LABEL: "label1",
        ACK: "123"
    });

    thread::sleep(Duration::from_millis(100));

    assert!(stream3.recv().unwrap().get_i32(OK).unwrap() == 0);

    // recv
    let recv = stream4.recv().unwrap();

    assert!(recv.get_str(CHAN).unwrap() == "aaa");
    assert!(recv.get_str("hello").unwrap() == "world");
}

#[test]
fn share() {
    let queen = Queen::new(MessageId::new(), ()).unwrap();

    let stream1 = queen.connect(msg!{}, None, None).unwrap();
    let stream2 = queen.connect(msg!{}, None, None).unwrap();
    let stream3 = queen.connect(msg!{}, None, None).unwrap();
    let stream4 = queen.connect(msg!{}, None, None).unwrap();

    // auth
    let _ = stream1.send(msg!{
        CHAN: AUTH
    });

    let _ = stream2.send(msg!{
        CHAN: AUTH
    });

    let _ = stream3.send(msg!{
        CHAN: AUTH
    });

    let _ = stream4.send(msg!{
        CHAN: AUTH
    });

    thread::sleep(Duration::from_millis(100));

    assert!(stream1.recv().unwrap().get_i32(OK).unwrap() == 0);
    assert!(stream2.recv().unwrap().get_i32(OK).unwrap() == 0);
    assert!(stream3.recv().unwrap().get_i32(OK).unwrap() == 0);
    assert!(stream4.recv().unwrap().get_i32(OK).unwrap() == 0);

    // attatch
    let _ = stream1.send(msg!{
        CHAN: ATTACH,
        VALUE: "aaa"
    });

    let _ = stream2.send(msg!{
        CHAN: ATTACH,
        VALUE: "aaa"
    });

    thread::sleep(Duration::from_millis(100));

    assert!(stream1.recv().unwrap().get_i32(OK).unwrap() == 0);
    assert!(stream2.recv().unwrap().get_i32(OK).unwrap() == 0);

    // send
    let _ = stream3.send(msg!{
        CHAN: "aaa",
        "hello": "world",
        ACK: "123"
    });

    thread::sleep(Duration::from_millis(100));

    assert!(stream3.recv().unwrap().get_i32(OK).unwrap() == 0);

    let recv = stream1.recv().unwrap();

    assert!(recv.get_str(CHAN).unwrap() == "aaa");
    assert!(recv.get_str("hello").unwrap() == "world");
    assert!(recv.get(FROM).is_some());

    let recv = stream2.recv().unwrap();

    assert!(recv.get_str(CHAN).unwrap() == "aaa");
    assert!(recv.get_str("hello").unwrap() == "world");
    assert!(recv.get(FROM).is_some());

    // send with share
    let _ = stream3.send(msg!{
        CHAN: "aaa",
        "hello": "world",
        SHARE: true,
        ACK: "123"
    });

    thread::sleep(Duration::from_millis(100));

    assert!(stream3.recv().unwrap().get_i32(OK).unwrap() == 0);

    let mut read_num = 0;

    if stream1.recv().is_ok() {
        read_num += 1;
    }

    if stream2.recv().is_ok() {
        read_num += 1;
    }

    assert!(read_num == 1);

    // with label

    // attatch
    let _ = stream1.send(msg!{
        CHAN: ATTACH,
        VALUE: "bbb",
        LABEL: "label1"
    });

    let _ = stream2.send(msg!{
        CHAN: ATTACH,
        VALUE: "bbb",
        LABEL: "label1"
    });

    let _ = stream3.send(msg!{
        CHAN: ATTACH,
        VALUE: "bbb"
    });

    thread::sleep(Duration::from_millis(100));

    assert!(stream1.recv().unwrap().get_i32(OK).unwrap() == 0);
    assert!(stream2.recv().unwrap().get_i32(OK).unwrap() == 0);
    assert!(stream3.recv().unwrap().get_i32(OK).unwrap() == 0);

    // send with share
    let _ = stream4.send(msg!{
        CHAN: "bbb",
        "hello": "world",
        LABEL: "label1",
        ACK: "123"
    });

    thread::sleep(Duration::from_millis(100));

    assert!(stream4.recv().unwrap().get_i32(OK).unwrap() == 0);

    let recv = stream1.recv().unwrap();

    assert!(recv.get_str(CHAN).unwrap() == "bbb");
    assert!(recv.get_str("hello").unwrap() == "world");
    assert!(recv.get(FROM).is_some());

    let recv = stream2.recv().unwrap();

    assert!(recv.get_str(CHAN).unwrap() == "bbb");
    assert!(recv.get_str("hello").unwrap() == "world");
    assert!(recv.get(FROM).is_some());

    assert!(stream3.recv().is_err());

    // send with share
    let _ = stream4.send(msg!{
        CHAN: "bbb",
        "hello": "world",
        LABEL: "label1",
        SHARE: true,
        ACK: "123"
    });

    thread::sleep(Duration::from_millis(100));

    assert!(stream4.recv().unwrap().get_i32(OK).unwrap() == 0);

    let mut read_num = 0;

    if stream1.recv().is_ok() {
        read_num += 1;
    }

    if stream2.recv().is_ok() {
        read_num += 1;
    }

    assert!(read_num == 1);

    assert!(stream3.recv().is_err());
}

#[test]
fn s2s() {
    let queen = Queen::new(MessageId::new(), ()).unwrap();

    let stream1 = queen.connect(msg!{}, None, None).unwrap();
    let stream2 = queen.connect(msg!{}, None, None).unwrap();
    let stream3 = queen.connect(msg!{}, None, None).unwrap();

    // auth
    let _ = stream1.send(msg!{
        CHAN: AUTH,
        CLIENT_ID: MessageId::with_string("016f9dd00d746c7f89ce342387e4c462").unwrap()
    });

    let _ = stream2.send(msg!{
        CHAN: AUTH,
        CLIENT_ID: MessageId::with_string("016f9dd0c97338e09f5c61e91e43f7c0").unwrap()
    });

    // duplicate
    let _ = stream3.send(msg!{
        CHAN: AUTH,
        CLIENT_ID: MessageId::with_string("016f9dd0c97338e09f5c61e91e43f7c0").unwrap()
    });

    thread::sleep(Duration::from_millis(100));

    assert!(stream1.recv().unwrap().get_i32(OK).unwrap() == 0);
    assert!(stream2.recv().unwrap().get_i32(OK).unwrap() == 0);
    // assert!(stream3.recv().unwrap().get_i32(OK).unwrap() == 0);
    let recv = stream3.recv().unwrap();

    assert!(ErrorCode::has_error(&recv) == Some(ErrorCode::DuplicateClientId));

    let _ = stream3.send(msg!{
        CHAN: AUTH,
        CLIENT_ID: MessageId::with_string("016f9dd11953dba9c0943f8c7ba0924b").unwrap()
    });

    thread::sleep(Duration::from_millis(100));

    assert!(stream3.recv().unwrap().get_i32(OK).unwrap() == 0);

    // type
    let stream4 = queen.connect(msg!{}, None, None).unwrap();

    let _ = stream4.send(msg!{
        CHAN: AUTH,
        CLIENT_ID: 123
    });

    thread::sleep(Duration::from_millis(100));

    let recv = stream4.recv().unwrap();

    assert!(ErrorCode::has_error(&recv) == Some(ErrorCode::InvalidClientIdFieldType));

    // client to client
    let _ = stream3.send(msg!{
        CHAN: "aaa",
        "hello": "world",
        ACK: "123",
        TO: MessageId::with_string("016f9dd25e24d713c22ec04881afd5d2").unwrap()
    });

    thread::sleep(Duration::from_millis(100));

    let recv = stream3.recv().unwrap();

    assert!(ErrorCode::has_error(&recv) == Some(ErrorCode::TargetClientIdNotExist));
    assert!(recv.get_message_id(CLIENT_ID).unwrap() == &MessageId::with_string("016f9dd25e24d713c22ec04881afd5d2").unwrap());

    let _ = stream3.send(msg!{
        CHAN: "aaa",
        "hello": "world",
        ACK: "123",
        TO: MessageId::with_string("016f9dd00d746c7f89ce342387e4c462").unwrap()
    });

    thread::sleep(Duration::from_millis(100));

    assert!(stream3.recv().unwrap().get_i32(OK).unwrap() == 0);

    // recv
    let recv = stream1.recv().unwrap();

    assert!(recv.get_str(CHAN).unwrap() == "aaa");
    assert!(recv.get_str("hello").unwrap() == "world");
    assert!(recv.get_message_id(FROM).unwrap() == &MessageId::with_string("016f9dd11953dba9c0943f8c7ba0924b").unwrap());

    // client to clients
    let _ = stream3.send(msg!{
        CHAN: "aaa",
        "hello": "world",
        ACK: "123",
        TO: [MessageId::with_string("016f9dd00d746c7f89ce342387e4c463").unwrap(), MessageId::with_string("016f9dd25e24d713c22ec04881afd5d2").unwrap()]
    });

    thread::sleep(Duration::from_millis(100));

    let recv = stream3.recv().unwrap();

    assert!(ErrorCode::has_error(&recv) == Some(ErrorCode::TargetClientIdNotExist));
    let array = recv.get_array(CLIENT_ID).unwrap();
    assert!(array.len() == 2);
    assert!(array.contains(&MessageId::with_string("016f9dd00d746c7f89ce342387e4c463").unwrap().into()));
    assert!(array.contains(&MessageId::with_string("016f9dd25e24d713c22ec04881afd5d2").unwrap().into()));

    let _ = stream3.send(msg!{
        CHAN: "aaa",
        "hello": "world",
        ACK: "123",
        TO: [MessageId::with_string("016f9dd00d746c7f89ce342387e4c462").unwrap(), MessageId::with_string("016f9dd0c97338e09f5c61e91e43f7c0").unwrap()]
    });

    thread::sleep(Duration::from_millis(100));

    assert!(stream3.recv().unwrap().get_i32(OK).unwrap() == 0);

    // recv
    let recv = stream1.recv().unwrap();

    assert!(recv.get_str(CHAN).unwrap() == "aaa");
    assert!(recv.get_str("hello").unwrap() == "world");
    assert!(recv.get_message_id(FROM).unwrap() == &MessageId::with_string("016f9dd11953dba9c0943f8c7ba0924b").unwrap());

    let recv = stream2.recv().unwrap();

    assert!(recv.get_str(CHAN).unwrap() == "aaa");
    assert!(recv.get_str("hello").unwrap() == "world");
    assert!(recv.get_message_id(FROM).unwrap() == &MessageId::with_string("016f9dd11953dba9c0943f8c7ba0924b").unwrap());

    // generate message id
    let stream4 = queen.connect(msg!{}, None, None).unwrap();

    let _ = stream4.send(msg!{
        CHAN: AUTH
    });

    thread::sleep(Duration::from_millis(100));

    let recv = stream4.recv().unwrap();

    assert!(recv.get_message_id(CLIENT_ID).is_ok());
}

#[test]
fn client_event() {
    let queen = Queen::new(MessageId::new(), ()).unwrap();

    let stream1 = queen.connect(msg!{}, None, None).unwrap();

    let _ = stream1.send(msg!{
        CHAN: AUTH,
        ROOT: 123
    });

    thread::sleep(Duration::from_millis(100));

    let recv = stream1.recv().unwrap();

    assert!(ErrorCode::has_error(&recv) == Some(ErrorCode::InvalidRootFieldType));

    let _ = stream1.send(msg!{
        CHAN: AUTH,
        ROOT: true
    });

    thread::sleep(Duration::from_millis(100));

    assert!(stream1.recv().unwrap().get_i32(OK).unwrap() == 0);

    // supe attach
    let _ = stream1.send(msg!{
        CHAN: ATTACH,
        VALUE: CLIENT_READY
    });

    let _ = stream1.send(msg!{
        CHAN: ATTACH,
        VALUE: CLIENT_BREAK
    });

    let _ = stream1.send(msg!{
        CHAN: ATTACH,
        VALUE: CLIENT_ATTACH
    });

    let _ = stream1.send(msg!{
        CHAN: ATTACH,
        VALUE: CLIENT_DETACH
    });

    thread::sleep(Duration::from_millis(100));

    assert!(stream1.recv().unwrap().get_i32(OK).unwrap() == 0);
    assert!(stream1.recv().unwrap().get_i32(OK).unwrap() == 0);
    assert!(stream1.recv().unwrap().get_i32(OK).unwrap() == 0);
    assert!(stream1.recv().unwrap().get_i32(OK).unwrap() == 0);

    // auth
    let stream2 = queen.connect(msg!{}, None, None).unwrap();

    let _ = stream2.send(msg!{
        CHAN: AUTH,
        ROOT: true
    });

    thread::sleep(Duration::from_millis(100));

    assert!(stream2.recv().unwrap().get_i32(OK).unwrap() == 0);

    let recv = stream1.recv().unwrap();

    assert!(recv.get_str(CHAN).unwrap() == CLIENT_READY);
    assert!(recv.get_bool(ROOT).unwrap() == true);
    assert!(recv.get_message_id(CLIENT_ID).is_ok());
    assert!(recv.get_message(LABEL).is_ok());
    assert!(recv.get_message(ATTR).is_ok());

    // attach
    let _ = stream2.send(msg!{
        CHAN: ATTACH,
        VALUE: "aaa"
    });

    thread::sleep(Duration::from_millis(100));

    assert!(stream2.recv().unwrap().get_i32(OK).unwrap() == 0);

    let recv = stream1.recv().unwrap();

    assert!(recv.get_str(CHAN).unwrap() == CLIENT_ATTACH);
    assert!(recv.get_str(VALUE).unwrap() == "aaa");
    assert!(recv.get_message_id(CLIENT_ID).is_ok());

    let _ = stream2.send(msg!{
        CHAN: ATTACH,
        VALUE: "aaa",
        LABEL: "label1"
    });

    thread::sleep(Duration::from_millis(100));

    assert!(stream2.recv().unwrap().get_i32(OK).unwrap() == 0);

    let recv = stream1.recv().unwrap();

    assert!(recv.get_str(CHAN).unwrap() == CLIENT_ATTACH);
    assert!(recv.get_str(VALUE).unwrap() == "aaa");
    assert!(recv.get_str(LABEL).unwrap() == "label1");
    assert!(recv.get_message_id(CLIENT_ID).is_ok());

    // detach
    let _ = stream2.send(msg!{
        CHAN: DETACH,
        VALUE: "aaa",
        LABEL: "label1"
    });

    thread::sleep(Duration::from_millis(100));

    assert!(stream2.recv().unwrap().get_i32(OK).unwrap() == 0);

    let recv = stream1.recv().unwrap();

    assert!(recv.get_str(CHAN).unwrap() == CLIENT_DETACH);
    assert!(recv.get_str(VALUE).unwrap() == "aaa");
    assert!(recv.get_str(LABEL).unwrap() == "label1");
    assert!(recv.get_message_id(CLIENT_ID).is_ok());

    let _ = stream2.send(msg!{
        CHAN: DETACH,
        VALUE: "aaa"
    });

    thread::sleep(Duration::from_millis(100));

    assert!(stream2.recv().unwrap().get_i32(OK).unwrap() == 0);

    let recv = stream1.recv().unwrap();

    assert!(recv.get_str(CHAN).unwrap() == CLIENT_DETACH);
    assert!(recv.get_str(VALUE).unwrap() == "aaa");
    assert!(recv.get_message_id(CLIENT_ID).is_ok());

    drop(stream2);

    thread::sleep(Duration::from_millis(100));

    let recv = stream1.recv().unwrap();

    assert!(recv.get_str(CHAN).unwrap() == CLIENT_BREAK);
    assert!(recv.get_message_id(CLIENT_ID).is_ok());
    assert!(recv.get_message(LABEL).is_ok());
    assert!(recv.get_message(ATTR).is_ok());

    // auth
    let stream2 = queen.connect(msg!{}, None, None).unwrap();

    let _ = stream2.send(msg!{
        CHAN: AUTH,
        CLIENT_ID: MessageId::with_string("016f9dd11953dba9c0943f8c7ba0924b").unwrap()
    });

    thread::sleep(Duration::from_millis(100));

    assert!(stream2.recv().unwrap().get_i32(OK).unwrap() == 0);

    let _ = stream1.send(msg!{
        CHAN: CLIENT_KILL,
        CLIENT_ID: MessageId::with_string("016f9dd11953dba9c0943f8c7ba0924b").unwrap()
    });

    thread::sleep(Duration::from_millis(100));

    let recv = stream1.recv().unwrap();

    assert!(recv.get_str(CHAN).unwrap() == CLIENT_READY);
    assert!(recv.get_bool(ROOT).unwrap() == false);
    assert!(recv.get_message_id(CLIENT_ID).unwrap() == &MessageId::with_string("016f9dd11953dba9c0943f8c7ba0924b").unwrap());

    let recv = stream1.recv().unwrap();

    assert!(recv.get_str(CHAN).unwrap() == CLIENT_KILL);
    assert!(recv.get_i32(OK).unwrap() == 0);
    assert!(recv.get_message_id(CLIENT_ID).unwrap() == &MessageId::with_string("016f9dd11953dba9c0943f8c7ba0924b").unwrap());

    let recv = stream1.recv().unwrap();

    assert!(recv.get_str(CHAN).unwrap() == CLIENT_BREAK);
    assert!(recv.get_message_id(CLIENT_ID).unwrap() == &MessageId::with_string("016f9dd11953dba9c0943f8c7ba0924b").unwrap());

    assert!(stream2.is_close());
    assert!(stream2.recv().is_err());

    let _ = stream1.send(msg!{
        CHAN: CLIENT_KILL,
        CLIENT_ID: MessageId::with_string("016f9dd11953dba9c0943f8c7ba0924c").unwrap()
    });

    thread::sleep(Duration::from_millis(100));

    let recv = stream1.recv().unwrap();

    assert!(ErrorCode::has_error(&recv) == Some(ErrorCode::TargetClientIdNotExist));
    assert!(recv.get_message_id(CLIENT_ID).unwrap() == &MessageId::with_string("016f9dd11953dba9c0943f8c7ba0924c").unwrap());
}

#[test]
fn mine() {
    let queen = Queen::new(MessageId::new(), ()).unwrap();

    let stream1 = queen.connect(msg!{}, None, None).unwrap();

    // mine
    let _ = stream1.send(msg!{
        CHAN: MINE,
    });

    thread::sleep(Duration::from_millis(100));

    let recv = stream1.recv().unwrap();

    assert!(recv.get_i32(OK).unwrap() == 0);

    let value = recv.get_message(VALUE).unwrap();

    assert!(value.get_bool(AUTH).unwrap() == false);
    assert!(value.get_bool(ROOT).unwrap() == false);
    assert!(value.get_message(CHANS).unwrap().is_empty());
    assert!(value.is_null(CLIENT_ID) == false);
    assert!(value.get_u64(SEND_MESSAGES).unwrap() == 0);
    assert!(value.get_u64(RECV_MESSAGES).unwrap() == 1);

    // auth
    let _ = stream1.send(msg!{
        CHAN: AUTH,
        ROOT: true
    });

    // attach
    let _ = stream1.send(msg!{
        CHAN: ATTACH,
        VALUE: "hello",
    });

    thread::sleep(Duration::from_millis(100));

    assert!(stream1.recv().unwrap().get_i32(OK).unwrap() == 0);
    assert!(stream1.recv().unwrap().get_i32(OK).unwrap() == 0);

    // mine
    let _ = stream1.send(msg!{
        CHAN: MINE,
    });

    thread::sleep(Duration::from_millis(100));

    let recv = stream1.recv().unwrap();

    assert!(recv.get_i32(OK).unwrap() == 0);

    let value = recv.get_message(VALUE).unwrap();

    assert!(value.get_bool(AUTH).unwrap() == true);
    assert!(value.get_bool(ROOT).unwrap() == true);
    assert!(value.get_message(CHANS).unwrap().get_array("hello").is_ok());
    assert!(value.get_message_id(CLIENT_ID).is_ok());
    assert!(value.get_u64(SEND_MESSAGES).unwrap() == 3);
    assert!(value.get_u64(RECV_MESSAGES).unwrap() == 4);
}

#[test]
fn client_event_send_recv() {
    let queen = Queen::new(MessageId::new(), ()).unwrap();

    let stream1 = queen.connect(msg!{}, None, None).unwrap();
    let stream2 = queen.connect(msg!{}, None, None).unwrap();
    let stream3 = queen.connect(msg!{}, None, None).unwrap();

    // auth
    let _ = stream1.send(msg!{
        CHAN: AUTH
    });

    let _ = stream2.send(msg!{
        CHAN: AUTH
    });

    let _ = stream3.send(msg!{
        CHAN: AUTH,
        ROOT: true
    });

    thread::sleep(Duration::from_millis(100));

    assert!(stream1.recv().unwrap().get_i32(OK).unwrap() == 0);
    assert!(stream2.recv().unwrap().get_i32(OK).unwrap() == 0);
    assert!(stream3.recv().unwrap().get_i32(OK).unwrap() == 0);

    // attach client event
    let _ = stream3.send(msg!{
        CHAN: ATTACH,
        VALUE: CLIENT_SEND,
    });

    let _ = stream3.send(msg!{
        CHAN: ATTACH,
        VALUE: CLIENT_RECV,
    });

    thread::sleep(Duration::from_millis(100));

    assert!(stream3.recv().unwrap().get_i32(OK).unwrap() == 0);
    assert!(stream3.recv().unwrap().get_i32(OK).unwrap() == 0);

    // try send
    let _ = stream1.send(msg!{
        CHAN: "aaa",
        "hello": "world",
        ACK: "123"
    });

    thread::sleep(Duration::from_millis(100));

    let recv = stream1.recv().unwrap();

    assert!(ErrorCode::has_error(&recv) == Some(ErrorCode::NoConsumers));
    assert!(stream3.recv().is_err());

    // attach
    let _ = stream2.send(msg!{
        CHAN: ATTACH,
        VALUE: "aaa"
    });

    // send
    let _ = stream1.send(msg!{
        CHAN: "aaa",
        "hello": "world",
        ACK: "123"
    });

    thread::sleep(Duration::from_millis(100));

    let recv = stream1.recv().unwrap();

    assert!(recv.get_i32(OK).unwrap() == 0);

    let recv = stream2.recv().unwrap();

    assert!(recv.get_i32(OK).unwrap() == 0);

    // recv
    let recv = stream2.recv().unwrap();

    assert!(recv.get_str(CHAN).unwrap() == "aaa");
    assert!(recv.get_str("hello").unwrap() == "world");

    let recv = stream3.recv().unwrap();
    assert!(recv.get_str(CHAN).unwrap() == CLIENT_RECV);

    let recv = stream3.recv().unwrap();
    assert!(recv.get_str(CHAN).unwrap() == CLIENT_SEND);
}

#[test]
fn self_send_recv() {
    let queen = Queen::new(MessageId::new(), ()).unwrap();

    let stream1 = queen.connect(msg!{}, None, None).unwrap();

    // auth
    let _ = stream1.send(msg!{
        CHAN: AUTH
    });

    thread::sleep(Duration::from_millis(100));

    assert!(stream1.recv().unwrap().get_i32(OK).unwrap() == 0);

    // attach
    let _ = stream1.send(msg!{
        CHAN: ATTACH,
        VALUE: "aaa"
    });

    // send
    let _ = stream1.send(msg!{
        CHAN: "aaa",
        "hello": "world",
        ACK: "123"
    });

    thread::sleep(Duration::from_millis(100));

    let recv = stream1.recv().unwrap();
    assert!(recv.get_i32(OK).unwrap() == 0);

    let recv = stream1.recv().unwrap();
    assert!(recv.get_str("hello").unwrap() == "world");
    assert!(recv.get_message_id(FROM).is_ok());
    assert!(recv.get_i32(OK).is_err());

    let recv = stream1.recv().unwrap();
    assert!(recv.get_i32(OK).unwrap() == 0);
    assert!(recv.get_str(ACK).unwrap() == "123");

    assert!(stream1.recv().is_err());

    // stream2
    let stream2 = queen.connect(msg!{}, None, None).unwrap();

    // auth
    let _ = stream2.send(msg!{
        CHAN: AUTH
    });

    thread::sleep(Duration::from_millis(100));

    assert!(stream2.recv().unwrap().get_i32(OK).unwrap() == 0);

    // attach
    let _ = stream2.send(msg!{
        CHAN: ATTACH,
        VALUE: "aaa"
    });

    thread::sleep(Duration::from_millis(100));

    assert!(stream2.recv().unwrap().get_i32(OK).unwrap() == 0);

    // send
    let _ = stream1.send(msg!{
        CHAN: "aaa",
        "hello": "world",
        ACK: "123"
    });

    thread::sleep(Duration::from_millis(100));

    let recv = stream1.recv().unwrap();
    assert!(recv.get_str("hello").unwrap() == "world");
    assert!(recv.get_message_id(FROM).is_ok());
    assert!(recv.get_i32(OK).is_err());

    let recv = stream2.recv().unwrap();
    assert!(recv.get_str("hello").unwrap() == "world");
    assert!(recv.get_message_id(FROM).is_ok());
    assert!(recv.get_i32(OK).is_err());

    let recv = stream1.recv().unwrap();
    assert!(recv.get_i32(OK).unwrap() == 0);
    assert!(recv.get_str(ACK).unwrap() == "123");

    assert!(stream1.recv().is_err());
    assert!(stream2.recv().is_err());
}
