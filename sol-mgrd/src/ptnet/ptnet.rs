use std::mem::size_of;

use sol_lib::helpers::any_as_u8_slice_mut;

use super::{ptnet_c};

#[repr(u8)]
#[allow(non_camel_case_types)]
pub enum COT {
    CYCLIC = ptnet_c::COT_CYCLIC as u8,
    SPONT = ptnet_c::COT_SPONT as u8,
    INIT = ptnet_c::COT_INIT as u8,
    REQ = ptnet_c::COT_REQ as u8,
    ACT = ptnet_c::COT_ACT as u8,
    ACT_CON = ptnet_c::COT_ACT_CON as u8,
    TERM = ptnet_c::COT_TERM as u8,
    INT = ptnet_c::COT_INT as u8,
    U_TI = ptnet_c::COT_U_TI as u8,
    U_COT = ptnet_c::COT_U_COT as u8,
    U_IOA = ptnet_c::COT_U_IOA as u8,
}

pub trait ASDHConstruct {
    fn with(ca: u8, cot: COT, pn: bool) -> ptnet_c::ASDH;
}

impl ASDHConstruct for ptnet_c::ASDH {
    fn with(ca: u8, cot: COT, pn: bool) -> ptnet_c::ASDH {
        let mut asdh = ptnet_c::ASDH {
            ca: ca,
            ..Default::default()
        };
        asdh.set_cot(cot as u8);
        asdh.set_pn(pn as u8);
        asdh
    }
}

pub trait VSQConstruct {
    fn with(n: u8, sq: bool) -> ptnet_c::VSQ;
}

impl VSQConstruct for ptnet_c::VSQ {
    fn with(n: u8, sq: bool) -> ptnet_c::VSQ {
        let mut vsq: ptnet_c::VSQ = Default::default();
        unsafe {
            vsq.__bindgen_anon_1.set_n(n);
            vsq.__bindgen_anon_1.set_sq(sq as u8);
        }
        vsq
    }
}

pub trait TIConstruct {
    fn with(tc: u8) -> ptnet_c::TI;
}

impl TIConstruct for ptnet_c::TI {
    fn with(tc: u8) -> ptnet_c::TI {
        ptnet_c::TI { raw: tc }
    }
}

pub trait TIBits {
    fn value(&self) -> u8;
    fn size(&self) -> u8;
    fn tc(&self) -> u8;
}

impl TIBits for ptnet_c::TI {
    fn value(&self) -> u8 { unsafe { self.raw } }
    fn size(&self) -> u8 { unsafe { self.__bindgen_anon_1.size() } }
    fn tc(&self) -> u8 { unsafe { self.__bindgen_anon_1.typecode() } }
}

pub trait DUIConstruct {
    fn with(ti: &ptnet_c::TI, vsq: &ptnet_c::VSQ) -> ptnet_c::DUI;
    fn with_direct(tc: u8, n: u8, sq: bool) -> ptnet_c::DUI;
}

impl DUIConstruct for ptnet_c::DUI {
    fn with(ti: &ptnet_c::TI, vsq: &ptnet_c::VSQ) -> ptnet_c::DUI {
        ptnet_c::DUI {
            ti: *ti,
            vsq: *vsq
        }
    }

    fn with_direct(tc: u8, n: u8, sq: bool) -> ptnet_c::DUI {
        ptnet_c::DUI {
            ti: ptnet_c::TI::with(tc),
            vsq: ptnet_c::VSQ::with(n, sq)
        }
    }
}

#[repr(u8)]
pub enum FC {
    PrmLinkTest = ptnet_c::FC_PRM_LINK_TEST as u8,
    PrmSendConfirm = ptnet_c::FC_PRM_SEND_CONFIRM as u8,
    PrmSendNoreply = ptnet_c::FC_PRM_SEND_NOREPLY as u8,
    SecAck = ptnet_c::FC_SEC_ACK as u8,
    SecNak = ptnet_c::FC_SEC_NAK as u8,
    SecLinkOk = ptnet_c::FC_SEC_LINK_OK as u8
}

pub trait HeaderBits {
    fn dir(&self) -> bool;
    fn prm(&self) -> bool;
    fn fc(&self) -> Option<FC>;
}

impl HeaderBits for ptnet_c::Header {
    fn dir(&self) -> bool { (self.C & (ptnet_c::BIT_DIR as u8)) != 0 }
    fn prm(&self) -> bool { (self.C & (ptnet_c::BIT_PRM as u8)) != 0 }
    fn fc(&self) -> Option<FC> {
        let u_fc = (self.C as u32) & ptnet_c::BITS_FC;

        match u_fc {
            ptnet_c::FC_PRM_LINK_TEST => Some(FC::PrmLinkTest),
            ptnet_c::FC_PRM_SEND_CONFIRM => Some(FC::PrmSendConfirm),
            ptnet_c::FC_PRM_SEND_NOREPLY => Some(FC::PrmSendNoreply),
            ptnet_c::FC_SEC_ACK => Some(FC::SecAck),
            ptnet_c::FC_SEC_NAK => Some(FC::SecNak),
            ptnet_c::FC_SEC_LINK_OK => Some(FC::SecLinkOk),
            _ => None
        }
    }
}

pub trait VSQBits {
    fn n(&self) -> u8;
    fn sq(&self) -> bool;
}

impl VSQBits for ptnet_c::VSQ {
    fn n(&self) -> u8 { unsafe { self.__bindgen_anon_1.n() } }
    fn sq(&self) -> bool { unsafe { self.__bindgen_anon_1.sq() != 0 } }
}

#[derive(Debug,PartialEq)]
pub enum IE {
    Unknown(Vec<u8>),

    /// Measured valued
    TI32(ptnet_c::TI32),
    TI33(ptnet_c::TI33),
    TI34(ptnet_c::TI34),
    TI68(ptnet_c::TI68),
    TI129(ptnet_c::TI129),
    TI130(ptnet_c::TI130),
    TI131(ptnet_c::TI131),
    TI132(ptnet_c::TI132),
    TI161(ptnet_c::TI161),
    TI192(ptnet_c::TI192),

    /// Commands and setpoints
    TI48(ptnet_c::TI48),
    TI49(ptnet_c::TI49),
    TI50(ptnet_c::TI50),
    TI84(ptnet_c::TI84),
    TI147(ptnet_c::TI147),

    /// system information in monitor direction
    TI232(ptnet_c::TI232),

    /// system information in control direction
    TI16(ptnet_c::TI16),
    TI25(ptnet_c::TI25),
    TI56(ptnet_c::TI56),
    TI90(ptnet_c::TI90),
    TI219(ptnet_c::TI219),
    TI240(ptnet_c::TI240)
}

pub enum IEParseError {
    BufferTooShort
}

impl IE {
    fn parse_from<T: Sized + Default>(buffer: &[u8]) -> Result<T, IEParseError> {
        if buffer.len() < size_of::<T>() {
            return Err(IEParseError::BufferTooShort);
        }

        let mut ie: T = Default::default();
        unsafe {
            any_as_u8_slice_mut(&mut ie).copy_from_slice(buffer);
        }
        return Ok(ie);
    }
}

impl TryFrom<(/* tc: */ u8, /* buffer: */ &[u8])> for IE {
    type Error = IEParseError;

    fn try_from(value: (/* tc: */ u8, /* buffer: */ &[u8])) -> Result<Self, Self::Error> {
        match value.0 {
            32 => Ok(IE::TI32(IE::parse_from(value.1)?)),
            33 => Ok(IE::TI33(IE::parse_from(value.1)?)),
            34 => Ok(IE::TI34(IE::parse_from(value.1)?)),
            68 => Ok(IE::TI68(IE::parse_from(value.1)?)),
            129 => Ok(IE::TI129(IE::parse_from(value.1)?)),
            130 => Ok(IE::TI130(IE::parse_from(value.1)?)),
            131 => Ok(IE::TI131(IE::parse_from(value.1)?)),
            132 => Ok(IE::TI132(IE::parse_from(value.1)?)),
            161 => Ok(IE::TI161(IE::parse_from(value.1)?)),
            192 => Ok(IE::TI192(IE::parse_from(value.1)?)),

            48 => Ok(IE::TI48(IE::parse_from(value.1)?)),
            49 => Ok(IE::TI49(IE::parse_from(value.1)?)),
            50 => Ok(IE::TI50(IE::parse_from(value.1)?)),
            84 => Ok(IE::TI84(IE::parse_from(value.1)?)),
            147 => Ok(IE::TI147(IE::parse_from(value.1)?)),

            232 => Ok(IE::TI232(IE::parse_from(value.1)?)),

            16 => Ok(IE::TI16(IE::parse_from(value.1)?)),
            25 => Ok(IE::TI25(IE::parse_from(value.1)?)),
            56 => Ok(IE::TI56(IE::parse_from(value.1)?)),
            90 => Ok(IE::TI90(IE::parse_from(value.1)?)),
            219 => Ok(IE::TI219(IE::parse_from(value.1)?)),
            240 => Ok(IE::TI240(IE::parse_from(value.1)?)),

            _ => Ok(IE::Unknown(value.1.to_vec()))
        }
    }
}
