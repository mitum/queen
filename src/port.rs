use std::time::Duration;
use std::io::{self};

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;
use std::sync::mpsc::channel;

use queen_io::queue::spsc::Queue;
use queen_io::queue::mpsc;

use nson::{Message};
use nson::message_id::MessageId;

use crate::dict::*;
use crate::Connector;

pub use recv::{Recv, AsyncRecv};
use port_backend::{PortBackend, Packet};

mod recv;
mod port_backend;

#[derive(Clone)]
pub struct Port {
    inner: Arc<PortInner>,
    run: Arc<AtomicBool>
}

struct PortInner {
    id: MessageId,
    recv_id: AtomicUsize,
    queue: mpsc::Queue<Packet>
}

impl Port {
    pub fn connect(id: MessageId, connector: Connector, auth_msg: Message) -> io::Result<Port> {
        let run = Arc::new(AtomicBool::new(true));

        let queue = mpsc::Queue::new()?;

        let queue2 = queue.clone();

        let mut inner = PortBackend::new(
                            id.clone(),
                            connector,
                            auth_msg,
                            queue2,
                            run.clone()
                        )?;

        thread::Builder::new().name("port_backend".to_string()).spawn(move || {
            inner.run().unwrap();
        }).unwrap();

        Ok(Port {
            inner: Arc::new(PortInner {
                id,
                recv_id: AtomicUsize::new(0),
                queue,
            }),
            run
        })
    }

    pub fn recv(
        &self,
        chan: &str,
        lables: Option<Vec<String>>
    ) -> Recv {
        let (tx, rx) = channel();

        let id = self.inner.recv_id.fetch_add(1, Ordering::SeqCst);

        self.inner.queue.push(Packet::AttachBlock(id, chan.to_string(), lables, tx));

        Recv {
            port: self.clone(),
            id,
            chan: chan.to_string(),
            recv: rx
        }
    }

    pub fn async_recv(
        &self,
        chan: &str,
        lables: Option<Vec<String>>
    ) -> io::Result<AsyncRecv> {
        let queue = Queue::with_cache(64)?;

        let id = self.inner.recv_id.fetch_add(1, Ordering::SeqCst);

        self.inner.queue.push(Packet::AttachAsync(id, chan.to_string(), lables, queue.clone()));

        Ok(AsyncRecv {
            port: self.clone(),
            id,
            chan: chan.to_string(),
            recv: queue
        })
    }

    pub fn send(
        &self,
        chan: &str,
        mut msg: Message,
        lable: Option<Vec<String>>
    ) {
        msg.insert(CHAN, chan);

        if let Some(lable) = lable {
            msg.insert(LABEL, lable);
        }

        if msg.get_message_id(ID).is_err() {
            msg.insert(ID, MessageId::new());
        }

        loop {
            if self.inner.queue.pending() < 64 {
                self.inner.queue.push(Packet::Send(msg));
                return
            }

            thread::sleep(Duration::from_millis(10));
        }
    }

    pub fn id(&self) -> &MessageId {
        &self.inner.id
    }

    pub fn is_run(&self) -> bool {
        self.run.load(Ordering::Relaxed)
    }

    pub fn stop(&self) {
        self.run.store(false, Ordering::Relaxed);
    }
}

impl Drop for Port {
    fn drop(&mut self) {
        if Arc::strong_count(&self.inner) == 1 {
            self.run.store(false, Ordering::Relaxed);
        }
    }
}
