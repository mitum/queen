use std::collections::{VecDeque, HashSet};
use std::io::{self, ErrorKind::PermissionDenied};
use std::time::Duration;
use std::thread::sleep;

use nson::{Message, msg};

use crate::poll::{poll, Ready, Events};
use crate::net::Addr;

use super::conn::Connection;

pub struct Bridge {
    session_a: Session,
    session_b: Session,
    read_buffer: VecDeque<Message>,
    white_list: HashSet<String>,
    run: bool,
}

pub struct BridgeConfig {
    pub addr1: Addr,
    pub auth_msg1: Message,
    pub hmac_key1: Option<String>,
    pub addr2: Addr,
    pub auth_msg2: Message,
    pub hmac_key2: Option<String>,
    pub white_list: HashSet<String>
}

struct Session {
    conn: Option<(i32, Connection)>,
    state: State,
    addr: Addr,
    auth_msg: Message,
    hmac_key: Option<String>,
}

#[derive(Debug, Eq, PartialEq)]
enum State {
    UnAuth,
    Authing,
    Authed
}

impl Bridge {
    pub fn connect(config: BridgeConfig) -> Bridge {
        Bridge {
            session_a: Session {
                conn: None,
                state: State::UnAuth,
                addr: config.addr1,
                auth_msg: config.auth_msg1,
                hmac_key: config.hmac_key1
            },
            session_b: Session {
                conn: None,
                state: State::UnAuth,
                addr: config.addr2,
                auth_msg: config.auth_msg2,
                hmac_key: config.hmac_key2
            },
            read_buffer: VecDeque::new(),
            white_list: config.white_list,
            run: true
        }
    }

    pub fn run(&mut self) -> io::Result<()> {
        while self.run {
            self.run_once()?;
        }

        Ok(())
    }

    pub fn run_once(&mut self) -> io::Result<()> {
        {
            macro_rules! link {
                ($session:ident) => {
                    if self.$session.conn.is_none() {
                        let conn = match self.$session.addr.connect() {
                            Ok(conn) => conn,
                            Err(err) => {
                                println!("link: {:?} err: {}", self.$session.addr, err);

                                sleep(Duration::from_secs(1));

                                return Ok(())
                            }
                        };

                        let fd = conn.fd();

                        self.$session.conn = Some((fd, conn));
                    }
                };
            }

            link!(session_a);
            link!(session_b);
        }

        {
            macro_rules! auth {
                ($session:ident) => {
                    if self.$session.state == State::UnAuth {
                        let mut msg = msg!{
                            "_chan": "_auth"
                        };

                        msg.extend(self.$session.auth_msg.clone());

                        self.$session.conn
                            .as_mut().unwrap()
                            .1.push_data(msg.to_vec().unwrap(), &self.$session.hmac_key);

                        self.$session.state = State::Authing;
                    }
                };
            }

            auth!(session_a);
            auth!(session_b);
        }

        let mut events = Events::new();

        {
            macro_rules! event_put {
                ($session:ident) => {
                    let (fd, conn) = self.$session.conn.as_ref().unwrap();

                    let mut interest = Ready::readable() | Ready::hup();

                    if conn.want_write() {
                        interest.insert(Ready::writable());
                    }

                    events.put(*fd, interest);
                };
            }
            event_put!(session_a);
            event_put!(session_b);
        }

        if poll(&mut events, Some(Duration::from_secs(1)))? > 0 {
            for event in &events {
                macro_rules! event {
                    ($session:ident) => {
                        if self.$session.conn.as_ref().map(|(id, _)| { *id == event.fd() }).unwrap_or(false) {
                            let readiness = event.readiness();

                            if readiness.is_hup() || readiness.is_error() {
                                self.$session.conn = None;
                                self.$session.state = State::UnAuth;

                                continue;
                            }

                            if readiness.is_readable() {
                                if let Some((_, conn)) = &mut self.$session.conn {
                                    if conn.read(&mut self.read_buffer, &self.$session.hmac_key).is_err() {
                                        self.$session.conn = None;
                                        self.$session.state = State::UnAuth;
                                    }

                                    if !self.read_buffer.is_empty() {
                                        self.handle_message_from_conn(stringify!($session))?;
                                    }
                                }
                            }

                            if readiness.is_writable() {
                                if let Some((_, conn)) = &mut self.$session.conn {
                                    if conn.write().is_err() {
                                        self.$session.conn = None;
                                        self.$session.state = State::UnAuth;
                                    }
                                }
                            }
                        }
                    };
                }

                event!(session_a);
                event!(session_b);
            }
        }

        Ok(())
    }

    fn handle_message_from_conn(&mut self, s: &str) -> io::Result<()> {
        macro_rules! handle_message {
            ($session_a:ident, $session_b:ident) => {
                while let Some(message) = self.read_buffer.pop_front() {
                    if let Ok(chan) = message.get_str("_chan") {
                        if chan.starts_with("_") {
                            match chan {
                                "_auth" => {
                                    if let Ok(ok) = message.get_i32("ok") {
                                        if ok == 0 {
                                            self.$session_a.state = State::Authed;

                                            for chan in &self.white_list {
                                                let msg = msg!{
                                                    "_chan": "_atta",
                                                    "_valu": chan
                                                };

                                                self.$session_a.conn
                                                    .as_mut().unwrap()
                                                    .1.push_data(msg.to_vec().unwrap(), &self.$session_a.hmac_key);
                                            }

                                            continue;
                                        } else {
                                            return Err(io::Error::new(PermissionDenied, "PermissionDenied"))
                                        }
                                    }

                                    self.$session_a.state = State::UnAuth;
                                }
                                "_atta" => {
                                    if let Ok(ok) = message.get_i32("ok") {
                                        if ok != 0 {
                                            println!("_atta: {:?}", message);
                                        }
                                    }
                                }
                                _ => ()
                            }
                        } else {
                            if self.$session_a.state != State::Authed || self.$session_b.state != State::Authed {
                                continue;
                            }

                            if let Some((_, conn)) = &mut self.$session_b.conn {            
                                conn.push_data(message.to_vec().unwrap(), &self.$session_b.hmac_key)
                            }
                        }
                    }
                }

            };
        }

        if s == "session_a" {
            handle_message!(session_a, session_b);
        } else if s == "session_b" {
            handle_message!(session_b, session_a);
        }

        Ok(())
    }
}