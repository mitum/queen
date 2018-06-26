use std::sync::Arc;
use std::collections::HashMap;
use std::io;
use std::thread;

use nson;

use error::Result;
use protocol::ContentType;
use client::Client;

pub type Handle = Fn(u8, Vec<u8>) -> Result<(u8, Vec<u8>)> + Send + Sync + 'static;

#[derive(Clone)]
pub struct RPC {
    pub client: Client,
    pub handles: HashMap<String, Arc<Box<Handle>>>
}

impl RPC {
    pub fn new(addr: &str) -> io::Result<RPC> {
        Ok(RPC {
            client: Client::new(addr)?,
            handles: HashMap::new()
        })
    }

    pub fn register<H>(&mut self, method: &str, handle: H)
        where H: Fn(u8, Vec<u8>) -> Result<(u8, Vec<u8>)> + Send + Sync + 'static
    {
        self.handles.insert(method.to_owned(), Arc::new(Box::new(handle)));
    }

    pub fn run(&self, worker_num: usize, username: &str, password: &str) -> io::Result<()> {
        let methods: Vec<String> = self.handles.keys().map(|k| k.to_owned()).collect();

        self.client.connect(username, password, methods)?;

        let handles = self.handles.clone();

        self.client.response(move |method, content_type, data| {
            if let Some(handle) = handles.get(&method) {
                let result = handle(content_type, data);

                return result;
            }

            Ok((0, vec![]))
        })?;

        let mut threads = Vec::new();

        for _ in 0..worker_num {

            let client = self.client.clone();

            threads.push(thread::spawn(move || {
                client.run().unwrap();
            }));
        }

        for thread in threads {
            let _ = thread.join().unwrap();
        }

        Ok(())
    }
}

// use serde::Serialize;
// use serde::de::DeserializeOwned;
use std::io::Cursor;

macro_rules! service {
    ($name:ident, $request:ident, $response:ident) => {
        #[derive(Serialize, Deserialize, Debug)]
        pub enum $name {
            Request($request),
            Response($response)
        }

        impl $name {
            pub fn request(rpc: &RPC, request: $request) -> Result<$response> {
                let n = nson::encode::to_nson(&request).unwrap();

                let mut buf = Vec::new();

                nson::encode::encode_object(&mut buf, &object!{"q": n}).unwrap();

                let (_, data) = rpc.client.request(stringify!($name), ContentType::NSON.bits(), buf)?;
                let mut reader = Cursor::new(data);

                let mut p = nson::decode::decode_object(&mut reader).unwrap();

                if let Some(r) = p.remove("p") {
                    let response = nson::decode::from_nson(r).unwrap();
                    return Ok(response)
                }

                unimplemented!()
            }

            pub fn response<F>(rpc: &mut RPC, handle: F)
                where F: Fn($request) -> Result<$response> + Send + Sync + 'static
            {
                rpc.register(stringify!($name), move |content_type, data| {

                    if content_type == ContentType::NSON.bits() {
                        let mut reader = Cursor::new(data);
                        let mut q = nson::decode::decode_object(&mut reader).unwrap();

                        if let Some(r) = q.remove("q") {
                            let request = nson::decode::from_nson(r).unwrap();

                            let response = handle(request).unwrap();

                            let p = nson::encode::to_nson(&response).unwrap();

                            let mut buf = Vec::new();
                            nson::encode::encode_object(&mut buf, &object!{"p": p}).unwrap();

                            return Ok((content_type, buf))
                        }
                    }

                    unimplemented!()
                });
            }
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Aaa {
    aa: i32
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Bbb {
    bb: i32
}

service!(Haha, Aaa, Bbb);