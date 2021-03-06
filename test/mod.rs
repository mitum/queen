use std::net::TcpListener;

mod test_queen;
mod test_port;
mod test_hook;

pub fn get_free_addr() -> String {
    let socket = TcpListener::bind("127.0.0.1:0").unwrap();
    socket.local_addr().unwrap().to_string()
}
