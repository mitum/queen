use std::time::Duration;
use std::collections::{HashMap, HashSet};
use std::io::{self, ErrorKind::{ConnectionAborted}};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

use queen_io::epoll::{Epoll, Events, Token, Ready, EpollOpt};
use queen_io::queue::mpsc;

use nson::{Message, msg};
use nson::message_id::MessageId;

use slab::Slab;

use rand::{self, thread_rng, rngs::ThreadRng};
use rand::seq::SliceRandom;

use crate::stream::Stream;
use crate::util::oneshot;
use crate::dict::*;
use crate::error::ErrorCode;

pub use callback::Callback;

mod callback;

#[derive(Clone)]
pub struct Queen {
    queue: mpsc::Queue<Packet>,
    run: Arc<AtomicBool>
}

impl Queen {
    pub fn new<T: Send + 'static>(id: MessageId, data: T, callback: Option<Callback<T>>) -> io::Result<Queen> {
        let queue = mpsc::Queue::new()?;
        let run = Arc::new(AtomicBool::new(true));

        let queen = Queen {
            queue: queue.clone(),
            run: run.clone()
        };

        thread::Builder::new().name("relay".to_string()).spawn(move || {
            let mut inner = QueenInner::new(id, queue, data, callback.unwrap_or_default(), run)?;

            inner.run()
        }).unwrap();

        Ok(queen)
    }

    pub fn stop(&mut self) {
        self.run.store(false, Ordering::Relaxed);
    }

    pub fn is_run(&self) -> bool {
        self.run.load(Ordering::Relaxed)
    }

    pub fn connect(&self, attr: Message, timeout: Option<Duration>) -> io::Result<Stream> {
        let (stream1, stream2) = Stream::pipe(64, attr)?;

        let (tx, rx) = oneshot::oneshot::<bool>();

        let packet = Packet::NewConn(stream1, tx);

        self.queue.push(packet);

        if let Some(timeout) = timeout {
            let ret = rx.wait_timeout(timeout);

            if !ret.unwrap_or_default() {
                return Err(io::Error::new(ConnectionAborted, "ConnectionAborted"))
            }
        } else {
            let ret = rx.wait();

            if !ret.unwrap_or_default() {
                return Err(io::Error::new(ConnectionAborted, "ConnectionAborted"))
            }
        }

        Ok(stream2)
    }
}

struct QueenInner<T> {
    id: MessageId,
    epoll: Epoll,
    events: Events,
    queue: mpsc::Queue<Packet>,
    conns: Slab<Port>,
    chans: HashMap<String, HashSet<usize>>,
    ports: HashMap<MessageId, usize>,
    rand: ThreadRng,
    data: T,
    callback: Callback<T>,
    run: Arc<AtomicBool>
}

enum Packet {
    NewConn(Stream, oneshot::Sender<bool>)
}

impl<T> QueenInner<T> {
    const QUEUE_TOKEN: Token = Token(usize::max_value());

    fn new(id: MessageId, queue: mpsc::Queue<Packet>, data: T, callback: Callback<T>,run: Arc<AtomicBool>) -> io::Result<QueenInner<T>> {
        let queen = QueenInner {
            id,
            epoll: Epoll::new()?,
            events: Events::with_capacity(1024),
            queue,
            conns: Slab::new(),
            chans: HashMap::new(),
            ports: HashMap::new(),
            rand: thread_rng(),
            callback,
            data,
            run
        };

        Ok(queen)
    }

    fn run(&mut self) -> io::Result<()> {
        self.epoll.add(&self.queue, Self::QUEUE_TOKEN, Ready::readable(), EpollOpt::level())?;

        while self.run.load(Ordering::Relaxed) {
            let size = self.epoll.wait(&mut self.events, None)?;

            for i in 0..size {
                let event = self.events.get(i).unwrap();

                if event.token() == Self::QUEUE_TOKEN {
                    self.dispatch_queue()?;
                } else {
                    self.dispatch_conn(event.token().0)?;
                }
            }
        }

        Ok(())
    }

    fn dispatch_queue(&mut self) -> io::Result<()> {
        if let Some(packet) = self.queue.pop() {
            match packet {
                Packet::NewConn(stream, sender) => {
                    let entry = self.conns.vacant_entry();

                    let port = Port::new(entry.key(), stream);

                    let success = if let Some(accept_fn) = &self.callback.accept_fn {
                       accept_fn(&port, &mut self.data)
                    } else {
                       true
                    };

                    if success {
                        self.epoll.add(&port.stream, Token(entry.key()), Ready::readable(), EpollOpt::level())?;
                        
                        entry.insert(port);

                        sender.send(true);
                    } else {
                        // stream.close();
                        sender.send(false);
                    }
                }
            }
        }

        Ok(())
    }

    fn dispatch_conn(&mut self, token: usize) -> io::Result<()> {
        if let Some(conn) = self.conns.get_mut(token) {
            if let Some(message) = conn.stream.recv() {
                if message.is_empty() && conn.stream.is_close() {
                    self.remove_conn(token)?;
                } else {
                    self.handle_message(token, message)?;
                }
            }
        }

        Ok(())
    }

    fn remove_conn(&mut self, token: usize) -> io::Result<()> {
        if self.conns.contains(token) {
            let conn = self.conns.remove(token);
            // conn.stream.close();
            self.epoll.delete(&conn.stream)?;

            for (chan, _) in &conn.chans {
                if let Some(ids) = self.chans.get_mut(chan) {
                    ids.remove(&token);

                    if ids.is_empty() {
                        self.chans.remove(chan);
                    }
                }
            }

            let mut event_msg = msg!{
                CHAN: PORT_BREAK
            };

            if let Some(port_id) = conn.id.clone() {
                self.ports.remove(&port_id);
                event_msg.insert(PORT_ID, port_id);
            }

            if let Some(remove_fn) = &self.callback.remove_fn {
                remove_fn(&conn, &mut self.data);
            }

            self.relay_super_message(token, PORT_BREAK, event_msg);
        }

        Ok(())
    }

    fn handle_message(&mut self, token: usize, mut message: Message) -> io::Result<()> {
        let success = if let Some(recv_fn) = &self.callback.recv_fn {
            recv_fn(&self.conns[token], &mut message, &mut self.data)
        } else {
            true
        };

        if !success {
            ErrorCode::RefuseReceiveMessage.insert_message(&mut message);
            
            self.conns[token].stream.send(message);
        
            return Ok(())
        }

        let chan = match message.get_str(CHAN) {
            Ok(chan) => chan,
            Err(_) => {
                ErrorCode::CannotGetChanField.insert_message(&mut message);

                self.conns[token].stream.send(message);

                return Ok(())
            }
        };

        if chan.starts_with('_') {
            match chan {
                AUTH => self.auth(token, message),
                ATTACH => self.attach(token, message),
                DETACH => self.detach(token, message),
                PING => self.ping(token, message),
                // QUERY => self.node_query(id, message)?,
                PORT_KILL => self.kill(token, message)?,
                _ => {
                    ErrorCode::UnsupportedChan.insert_message(&mut message);

                    self.conns[token].stream.send(message);
                }
            }
        } else {
            self.relay_message(token, chan.to_string(), message);
        }

        return Ok(())
    }

    fn auth(&mut self, token: usize, mut message: Message) {
        let success = if let Some(auth_fn) = &self.callback.auth_fn {
            auth_fn(&self.conns[token], &mut message, &mut self.data)
        } else {
            true
        };

        if !success {
            ErrorCode::AuthenticationFailed.insert_message(&mut message);

            self.conns[token].stream.send(message);

            return
        }

        let mut conn = &mut self.conns[token];

        if let Some(s) = message.get(SUPER) {
            if let Some(s) = s.as_bool() {
                conn.supe = s;
            } else {
                ErrorCode::InvalidSuperFieldType.insert_message(&mut message);

                self.conns[token].stream.send(message);

                return
            }
        }

        if let Some(port_id) = message.get(PORT_ID) {
            if let Some(port_id) = port_id.as_message_id() {
                if let Some(other_token) = self.ports.get(port_id) {
                        if *other_token != token {
                            ErrorCode::DuplicatePortId.insert_message(&mut message);

                            self.conns[token].stream.send(message);

                            return
                        }
                    }

                    self.ports.insert(port_id.clone(), token);

                    conn.id = Some(port_id.clone());
            } else {
                ErrorCode::InvalidPortIdFieldType.insert_message(&mut message);

                self.conns[token].stream.send(message);

                return
            }
        } else {
            let id = MessageId::new();

            conn.id = Some(id.clone());

            message.insert(PORT_ID, id);
        }

        conn.auth = true;

        message.insert(NODE_ID, self.id.clone());

        ErrorCode::OK.insert_message(&mut message);
        
        let mut event_msg = msg!{
            CHAN: PORT_READY,
            SUPER: conn.supe
        };

        if let Some(port_id) = &conn.id {
            event_msg.insert(PORT_ID, port_id.clone());
        }

        self.conns[token].stream.send(message);

        self.relay_super_message(token, PORT_READY, event_msg);
    }

    fn attach(&mut self, token: usize, mut message: Message) {
        // check auth
        if !self.conns[token].auth {
            ErrorCode::Unauthorized.insert_message(&mut message);

            self.conns[token].stream.send(message);

            return
        }

        if let Ok(chan) = message.get_str(VALUE).map(ToOwned::to_owned) {
            // check super
            match chan.as_str() {
                PORT_READY | PORT_BREAK | PORT_ATTACH | PORT_DETACH => {

                    if !self.conns[token].supe {
                        ErrorCode::Unauthorized.insert_message(&mut message);

                        self.conns[token].stream.send(message);

                        return
                    }

                }
                _ => ()
            }

            // can attach
            let success = if let Some(attach_fn) = &self.callback.attach_fn {
                attach_fn(&self.conns[token], &mut message, &mut self.data)
            } else {
                true
            };

            if !success {
                ErrorCode::Unauthorized.insert_message(&mut message);

                self.conns[token].stream.send(message);

                return
            }

            // label
            let mut labels = vec![];

            if let Some(label) = message.get(LABEL) {
                if let Some(label) = label.as_str() {
                    labels.push(label.to_string());
                } else if let Some(label_array) = label.as_array() {
                    for v in label_array {
                        if let Some(v) = v.as_str() {
                            labels.push(v.to_string());
                        } else {
                            ErrorCode::InvalidLabelFieldType.insert_message(&mut message);

                            self.conns[token].stream.send(message);

                            return
                        }
                    }
                } else {
                    ErrorCode::InvalidLabelFieldType.insert_message(&mut message);

                    self.conns[token].stream.send(message);

                    return
                }
            }

            // port event
            let mut event_msg = msg!{
                CHAN: PORT_ATTACH
            };

            event_msg.insert(VALUE, &chan);

            if let Some(label) = message.get(LABEL) {
                event_msg.insert(LABEL, label.clone());
            }

            // session_attach
            let ids = self.chans.entry(chan.to_owned()).or_insert_with(HashSet::new);

            ids.insert(token);

            {
                let conn = self.conns.get_mut(token).unwrap();

                conn.chans.insert(chan, labels);

                if let Some(port_id) = &conn.id {
                    event_msg.insert(PORT_ID, port_id.clone());
                }
            }

            self.relay_super_message(token, PORT_ATTACH, event_msg);

            ErrorCode::OK.insert_message(&mut message);
        } else {
            ErrorCode::CannotGetValueField.insert_message(&mut message);
        }

        self.conns[token].stream.send(message);
    }

    fn detach(&mut self, token: usize, mut message: Message) {
        // check auth
        if !self.conns[token].auth {
            ErrorCode::Unauthorized.insert_message(&mut message);

            self.conns[token].stream.send(message);

            return
        }

        if let Ok(chan) = message.get_str(VALUE).map(ToOwned::to_owned) {
            if let Some(detach_fn) = &self.callback.detach_fn {
                detach_fn(&self.conns[token], &mut message, &mut self.data);
            }

            // label
            let mut labels = vec![];

            if let Some(label) = message.get(LABEL) {
                if let Some(label) = label.as_str() {
                    labels.push(label.to_string());
                } else if let Some(label_array) = label.as_array() {
                    for v in label_array {
                        if let Some(v) = v.as_str() {
                            labels.push(v.to_string());
                        } else {
                            ErrorCode::InvalidLabelFieldType.insert_message(&mut message);

                            self.conns[token].stream.send(message);

                            return
                        }
                    }
                } else {
                    ErrorCode::InvalidLabelFieldType.insert_message(&mut message);

                    self.conns[token].stream.send(message);

                    return
                }
            }

            // port event
            let mut event_msg = msg!{
                CHAN: PORT_DETACH,
                VALUE: &chan
            };

            if let Some(label) = message.get(LABEL) {
                event_msg.insert(LABEL, label.clone());
            }

            // session_detach
            {
                let conn = self.conns.get_mut(token).unwrap();

                if labels.is_empty() {
                    conn.chans.remove(&chan);

                    if let Some(ids) = self.chans.get_mut(&chan) {
                        ids.remove(&token);

                        if ids.is_empty() {
                            self.chans.remove(&chan);
                        }
                    }
                } else {
                    if let Some(vec) = conn.chans.get_mut(&chan) {
                        *vec = vec.iter().filter(|label| !labels.contains(label)).map(|s| s.to_string()).collect();
                    }
                }

                if let Some(port_id) = &conn.id {
                    event_msg.insert(PORT_ID, port_id.clone());
                }
            }

            self.relay_super_message(token, PORT_DETACH, event_msg);
        
            ErrorCode::OK.insert_message(&mut message);
        } else {
            ErrorCode::CannotGetValueField.insert_message(&mut message);
        }

        self.conns[token].stream.send(message);
    }

    fn ping(&mut self, token: usize, mut message: Message) {
        ErrorCode::OK.insert_message(&mut message);

        self.conns[token].stream.send(message);
    }

    fn kill(&mut self, token: usize, mut message: Message) -> io::Result<()> {
        {
            let conn = &mut self.conns[token];

            if !conn.auth || !conn.supe {
                ErrorCode::Unauthorized.insert_message(&mut message);

                conn.stream.send(message);

                return Ok(())
            }
        }

        let success = if let Some(kill_fn) = &self.callback.kill_fn {
            kill_fn(&self.conns[token], &mut message, &mut self.data)
        } else {
            true
        };

        if !success {
            ErrorCode::Unauthorized.insert_message(&mut message);

            self.conns[token].stream.send(message);

            return Ok(())
        }

        let mut remove_id = None;

        if let Some(port_id) = message.get(PORT_ID) {
            if let Some(port_id) = port_id.as_message_id() {
                if let Some(other_id) = self.ports.get(port_id).cloned() {
                    remove_id = Some(other_id);
                }
            } else {
                ErrorCode::InvalidPortIdFieldType.insert_message(&mut message);

                self.conns[token].stream.send(message);

                return Ok(())
            }
        }

        ErrorCode::OK.insert_message(&mut message);

        self.conns[token].stream.send(message);

        if let Some(remove_id) = remove_id {
            self.remove_conn(remove_id)?;
        }

        return Ok(())
    }

    fn relay_message(&mut self, token: usize, chan: String, mut message: Message) {
        // check auth
        if !self.conns[token].auth {
            ErrorCode::Unauthorized.insert_message(&mut message);

            self.conns[token].stream.send(message);

            return
        }

        let success = if let Some(emit_fn) = &self.callback.emit_fn {
            emit_fn(&self.conns[token], &mut message, &mut self.data)
        } else {
            true
        };

        if !success {
            ErrorCode::Unauthorized.insert_message(&mut message);

            self.conns[token].stream.send(message);

            return
        }

        let reply_msg = if let Some(ack) = message.get(ACK) {
            let mut reply_msg = msg!{
                CHAN: ACK,
                ACK: ack.clone()
            };

            ErrorCode::OK.insert_message(&mut reply_msg);

            message.remove(ACK);

            Some(reply_msg)
        } else {
            None
        };

        // to
        let mut to_ids = vec![];

        if let Some(to) = message.get(TO) {
            if let Some(to_id) = to.as_message_id() {
                if !self.ports.contains_key(to_id) {
                    ErrorCode::TargetPortIdNotExist.insert_message(&mut message);

                    self.conns[token].stream.send(message);

                    return
                }

                to_ids.push(to_id.clone());
            } else if let Some(to_array) = to.as_array() {
                for to in to_array {
                    if let Some(to_id) = to.as_message_id() {
                        if !self.ports.contains_key(to_id) {
                            ErrorCode::TargetPortIdNotExist.insert_message(&mut message);

                            self.conns[token].stream.send(message);

                            return
                        }

                        to_ids.push(to_id.clone());
                    } else {
                        ErrorCode::InvalidToFieldType.insert_message(&mut message);

                        self.conns[token].stream.send(message);

                        return
                    }
                }
            } else {
                ErrorCode::InvalidToFieldType.insert_message(&mut message);

                self.conns[token].stream.send(message);

                return
            }
        }

        if let Some(port_id) = &self.conns[token].id {
            message.insert(FROM, port_id.clone());
        }

        // labels
        let mut labels = vec![];

        if let Some(label) = message.get(LABEL) {
            if let Some(label) = label.as_str() {
                labels.push(label.to_string());
            } else if let Some(label_array) = label.as_array() {
                for v in label_array {
                    if let Some(v) = v.as_str() {
                        labels.push(v.to_string());
                    } else {
                        ErrorCode::InvalidLabelFieldType.insert_message(&mut message);

                        self.conns[token].stream.send(message);

                        return
                    }
                }
            } else {
                ErrorCode::InvalidLabelFieldType.insert_message(&mut message);

                self.conns[token].stream.send(message);

                return
            }
        }

        let mut no_consumers = true;

        if !to_ids.is_empty() {
            no_consumers = false;

            for to in &to_ids {
                if let Some(conn_id) = self.ports.get(to) {
                    if let Some(conn) = self.conns.get_mut(*conn_id) {
                        let success = if let Some(send_fn) = &self.callback.send_fn {
                            send_fn(&conn, &mut message, &mut self.data)
                        } else {
                            true
                        };

                        if success {
                            conn.stream.send(message.clone());
                        }
                    }
                }
            }
        } else if message.get_bool(SHARE).ok().unwrap_or(false) {
            let mut array: Vec<usize> = Vec::new();

            if let Some(ids) = self.chans.get(&chan) {
                for conn_id in ids {
                    if let Some(conn) = self.conns.get_mut(*conn_id) {
                        // filter labels
                        if !labels.is_empty() {
                            let conn_labels = conn.chans.get(&chan).expect("It shouldn't be executed here!");

                            if !conn_labels.iter().any(|l| labels.contains(l)) {
                                continue
                            }
                        }

                        array.push(*conn_id);
                    }
                }
            }

            if !array.is_empty() {
                no_consumers = false;

                if array.len() == 1 {
                    if let Some(conn) = self.conns.get_mut(array[0]) {
                        let success = if let Some(send_fn) = &self.callback.send_fn {
                            send_fn(&conn, &mut message, &mut self.data)
                        } else {
                            true
                        };

                        if success {
                            conn.stream.send(message.clone());
                        }
                    }
                } else if let Some(id) = array.choose(&mut self.rand) {
                    if let Some(conn) = self.conns.get_mut(*id) {
                        let success = if let Some(send_fn) = &self.callback.send_fn {
                            send_fn(&conn, &mut message, &mut self.data)
                        } else {
                            true
                        };

                        if success {
                            conn.stream.send(message.clone());
                        }
                    }
                }
            }

        } else if let Some(ids) = self.chans.get(&chan) {
            for conn_id in ids {
                if let Some(conn) = self.conns.get_mut(*conn_id) {
                    // filter labels
                    if !labels.is_empty() {
                        let conn_labels = conn.chans.get(&chan).expect("It shouldn't be executed here!");

                        if !conn_labels.iter().any(|l| labels.contains(l)) {
                            continue
                        }
                    }

                    no_consumers = false;

                    let success = if let Some(send_fn) = &self.callback.send_fn {
                        send_fn(&conn, &mut message, &mut self.data)
                    } else {
                        true
                    };

                    if success {
                        conn.stream.send(message.clone());
                    }
                }
            }
        }

        if no_consumers {
            ErrorCode::NoConsumers.insert_message(&mut message);

            self.conns[token].stream.send(message);

            return
        }

        if let Some(reply_msg) = reply_msg {
            self.conns[token].stream.send(reply_msg);
        }
    }

    fn relay_super_message(&mut self, token: usize, chan: &str, message: Message) {
        if let Some(tokens) = self.chans.get(chan) {
            for other_token in tokens {
                if token == *other_token {
                    continue;
                }

                if let Some(conn) = self.conns.get_mut(*other_token) {
                    let mut message = message.clone();

                    let success = if let Some(send_fn) = &self.callback.send_fn {
                        send_fn(&conn, &mut message, &mut self.data)
                    } else {
                        true
                    };

                    if success {
                        conn.stream.send(message);
                    }
                }
            }
        }
    }
}

pub struct Port {
    pub token: usize,
    pub auth: bool,
    pub supe: bool,
    pub chans: HashMap<String, Vec<String>>,
    pub stream: Stream,
    pub id: Option<MessageId>,
}

impl Port {
    pub fn new(token: usize, stream: Stream) -> Port {
        Port {
            token,
            auth: false,
            supe: false,
            chans: HashMap::new(),
            stream,
            id: None
        }
    }
}
