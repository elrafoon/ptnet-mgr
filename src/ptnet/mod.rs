#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]

mod ptnet;
mod packet;
mod scanner;
mod helpers;

pub use ptnet::*;
pub use ptnet_c::*;
pub use self::packet::*;
pub use scanner::*;

pub enum MessageResultCode {
    Ok = 0,
    NotDelivered = 1,
    TimedOut = 2,
    LinkDown = 3,
    PortInvalid = 4
}

pub mod ptnet_c {
    use super::{VSQBits, TIBits};

    include!(concat!(env!("OUT_DIR"), "/ptnet.rs"));

    impl From<u16> for super::MessageResultCode {
        fn from(value: u16) -> Self {
            match value {
                0 => super::MessageResultCode::Ok,
                1 => super::MessageResultCode::NotDelivered,
                2 => super::MessageResultCode::TimedOut,
                3 => super::MessageResultCode::LinkDown,
                4 => super::MessageResultCode::PortInvalid,
                _ => panic!("MessageResultCode for {} not defined", value)
            }
        }
    }

    impl MessageResult {
        pub fn result_code(&self) -> super::MessageResultCode {
            self.result.into()
        }
    }

    impl std::fmt::Debug for super::TI {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "TI{}", self.value())
        }
    }

    impl PartialEq for super::TI {
        fn eq(&self, other: &Self) -> bool {
            self.value() == other.value()
        }
    }

    impl std::fmt::Debug for super::VSQ {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "VSQ(SQ={},N={})", self.sq(), self.n())
        }
    }

    impl PartialEq for super::VSQ {
        fn eq(&self, other: &Self) -> bool {
            unsafe { self.raw == other.raw }
        }
    }

    impl std::fmt::Debug for super::DUI {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("DUI").field("ti", &self.ti).field("vsq", &self.vsq).finish()
        }
    }

    impl PartialEq for super::DUI {
        fn eq(&self, other: &Self) -> bool {
            self.ti == other.ti && self.vsq == other.vsq
        }
    }

    impl PartialEq for super::TI25 {
        fn eq(&self, other: &Self) -> bool {
            true
        }
    }
}
