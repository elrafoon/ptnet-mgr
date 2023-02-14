#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]

pub mod ptnet {
    include!(concat!(env!("OUT_DIR"), "/ptnet.rs"));
}

pub mod connection {
    include!(concat!(env!("OUT_DIR"), "/ptlink_connection.rs"));
}
