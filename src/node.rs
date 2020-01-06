use std::io::{self, ErrorKind::{WouldBlock}};
use std::os::unix::io::{AsRawFd};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use queen_io::epoll::{Epoll, Events, Token, Ready, EpollOpt};
use queen_io::queue::spsc::Queue;

use rand::{self, thread_rng, rngs::ThreadRng};
use rand::seq::SliceRandom;

use crate::Queen;
use crate::net::{Listen, Addr, NetWork, Packet, AccessFn};

pub struct Node {
    queen: Queen,
    epoll: Epoll,
    events: Events,
    queues: Vec<Queue<Packet>>,
    listens: Vec<Listen>,
    rand: ThreadRng,
    access_fn: Option<AccessFn>,
    pub run: Arc<AtomicBool>
}

impl Node {
    pub fn new(
        queen: Queen,
        works: usize,
        addrs: Vec<Addr>
    ) -> io::Result<Node> {
        let run = Arc::new(AtomicBool::new(true));

        let mut listens = Vec::new();

        for addr in addrs {
            listens.push(addr.bind()?);
        }

        let mut queues = Vec::new();

        for _ in 0..works {
            let queue: Queue<Packet> = Queue::with_cache(64)?;
        
            let queue2 = queue.clone();

            queues.push(queue);

            let mut net_work = NetWork::new(queue2, run.clone())?;

            thread::Builder::new().name("net".to_string()).spawn(move || {
                net_work.run()
            }).unwrap();
        }

        Ok(Node {
            queen,
            epoll: Epoll::new()?,
            events: Events::with_capacity(16),
            queues,
            listens,
            rand: thread_rng(),
            access_fn: None,
            run
        })
    }

    pub fn set_access_fn<F>(&mut self, f: F)
        where F: Fn(String) -> Option<String> + Send + Sync + 'static
    {
        self.access_fn = Some(Arc::new(Box::new(f)))
    }

    pub fn run(&mut self) -> io::Result<()> {
        for (id, listen) in self.listens.iter().enumerate() {
            self.epoll.add(&listen.as_raw_fd(), Token(id), Ready::readable(), EpollOpt::edge())?;
        }

        while self.run.load(Ordering::Relaxed) && self.queen.is_run() {
            let size = self.epoll.wait(&mut self.events, Some(Duration::from_secs(10)))?;

            for i in 0..size {
                let event = self.events.get(i).unwrap();
                let token = event.token();

                if let Some(listen) = self.listens.get(token.0) {
                    loop {
                        let (socket, addr) = match listen.accept() {
                            Ok(socket) => socket,
                            Err(err) => {
                                if let WouldBlock = err.kind() {
                                    break;
                                } else {
                                    return Err(err)
                                }
                            }
                        };

                        let attr = addr.to_message();

                        match self.queen.connect(attr, None) {
                            Ok(stream) => {
                                if let Some(queue) = self.queues.choose(&mut self.rand) {
                                    queue.push(Packet::NewServ{
                                        stream,
                                        net_stream: socket,
                                        access_fn: self.access_fn.clone()
                                    })
                                }
                            },
                            Err(err) => {
                                println!("connect: {:?}", err);
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

impl Drop for Node {
    fn drop(&mut self) {
        self.run.store(false, Ordering::Relaxed);
    }
}
