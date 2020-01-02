// node
pub const CHAN:      &str = "_chan";
pub const CHANS:     &str = "_chas";
pub const AUTH:      &str = "_auth";
pub const ATTACH:    &str = "_atta";
pub const ATTACH_ID: &str = "_atid";
pub const DETACH:    &str = "_deta";
pub const PING:      &str = "_ping";
pub const QUERY:     &str = "_quer";
pub const NODE_ID:   &str = "_noid";
pub const PORT_ID:   &str = "_ptid";
pub const VALUE:     &str = "_valu";
pub const LABEL:     &str = "_labe";
pub const TO:        &str = "_to";
pub const FROM:      &str = "_from";
pub const SHARE:     &str = "_shar";
pub const ACK:       &str = "_ack";
pub const SUPER:     &str = "_supe";
pub const ATTR:      &str = "_attr";
pub const CUSTOM:    &str = "_cust";

// message
pub const ID:        &str = "_id";
pub const ADDR:      &str = "_addr";
pub const ADDR_TYPE: &str = "_addr_type";

// error
pub const OK:    &str = "ok";
pub const ERROR: &str = "error";

// port event
pub const PORT_READY:  &str = "_ptre";
pub const PORT_BREAK:  &str = "_ptbr";
pub const PORT_ATTACH: &str = "_ptat";
pub const PORT_DETACH: &str = "_ptde";
pub const PORT_KILL:   &str = "_ptki";
pub const PORT_SEND:   &str = "_ptse";
pub const PORT_RECV:   &str = "_ptrc";

// query
pub const QUERY_PORT_NUM: &str = "$port_num";
pub const QUERY_CHAN_NUM: &str = "$chan_num";
pub const QUERY_PORTS:    &str = "$ports";
pub const QUERY_PORT:     &str = "$port";

// crypto
pub const AES_128_GCM:       &str = "AES_128_GCM";
pub const AES_256_GCM:       &str = "AES_256_GCM";
pub const CHACHA20_POLY1305: &str = "CHACHA20_POLY1305";

// network
pub const HANDSHAKE: &str = "_hand";
pub const ACCESS:    &str = "_acce";

// port
// pub const REPLY:   &str = "_reply";
pub const UNKNOWN: &str = "_unknown";

// rpc
pub const REQUEST_ID: &str = "_reqid";
pub const RPC_RECV:   &str = "RPC/RECV";
