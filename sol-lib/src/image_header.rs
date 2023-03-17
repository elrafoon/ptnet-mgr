use std::num::ParseIntError;

use crc::{Crc, CRC_32_CKSUM};

use crate::helpers::any_as_u8_slice;

pub const CRC32: Crc<u32> = Crc::<u32>::new(&CRC_32_CKSUM);

#[repr(packed(1))]
#[derive(Clone,Copy,Default,Debug)]
pub struct HWVersion {
    pub vid: u8,
    pub pid: u8,
    pub rev: u8
}

#[derive(Debug)]
pub enum ParseError {
    NotEnoughTokens(&'static str),
    TooMuchTokens(&'static str),
    ParseIntError(ParseIntError)
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::NotEnoughTokens(place) => write!(f, "Not enough tokens in {}", place),
            ParseError::TooMuchTokens(place) => write!(f, "Too much tokens in {}", place),
            ParseError::ParseIntError(error) => write!(f, "Error parsing number: {}", error)
        }
    }
}

impl From<ParseIntError> for ParseError {
    fn from(value: ParseIntError) -> Self { ParseError::ParseIntError(value) }
}

impl std::str::FromStr for HWVersion {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut pos: usize = 0;
        let mut parts = [0u8; 3];
        for tok in s.split(":") {
            if pos >= parts.len() {
                return Err(ParseError::TooMuchTokens("HWVersion"));
            }

            parts[pos] = u8::from_str_radix(tok, 16)?;
            pos += 1;
        }
        if pos == parts.len() {
            Ok(HWVersion { vid: parts[0], pid: parts[1], rev: parts[2] })
        } else {
            Err(ParseError::NotEnoughTokens("HWVersion"))
        }
    }
}

#[repr(packed(1))]
#[derive(Clone,Copy,Default,Debug)]
pub struct FWVersion {
    pub major: u8,
    pub minor: u8,
    pub patch: u8
}

impl std::str::FromStr for FWVersion {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut pos: usize = 0;
        let mut parts = [0u8; 3];
        for tok in s.split(".") {
            if pos >= parts.len() {
                return Err(ParseError::TooMuchTokens("FWVersion"));
            }

            parts[pos] = u8::from_str_radix(tok, 10)?;
            pos += 1;
        }

        if pos == parts.len() {
            Ok(FWVersion { major: parts[0], minor: parts[1], patch: parts[2] })
        } else {
            Err(ParseError::NotEnoughTokens("FWVersion"))
        }
    }
}

#[repr(packed(1))]
#[derive(Clone,Copy,Default,Debug)]
pub struct HeaderFields0 {
    pub hw_version: HWVersion,
    pub fw_version: FWVersion,
    pub payload_size: u32,
    pub payload_crc: u32
}

#[repr(packed(1))]
#[derive(Clone,Copy,Default,Debug)]
pub struct HeaderFields {
    /// header version
    pub version: u8,
    pub v0: HeaderFields0
}

#[repr(packed(1))]
#[derive(Clone,Copy)]
pub union Header {
    pub raw: [u8; 116],
    pub fields: HeaderFields
}

impl std::fmt::Debug for Header {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        unsafe { self.fields.fmt(f) }
    }
}

impl Default for Header {
    fn default() -> Self {
        Self { raw: [0; 116] }
    }
}

pub const MAGIC1: u32 = 0xFEEDBEEF;
pub const MAGIC2: u32 = 0xDEADBEEF;

#[repr(packed(1))]
#[derive(Clone,Copy,Debug)]
pub struct Container {
    pub magic1: u32,
    pub header: Header,
    pub header_crc: u32,
    pub magic2: u32
}

impl Default for Container {
    fn default() -> Self {
        Self { magic1: MAGIC1, header: Default::default(), header_crc: Default::default(), magic2: MAGIC2 }
    }
}

pub fn crc(buf: &[u8]) -> u32 {
    CRC32.checksum(buf)
}

#[derive(Debug)]
pub enum VerifyError {
    HeaderMagicNotPresent,
    HeaderCRCInvalid,
    PayloadSizeInvalid,
    PayloadCRCInvalid
}

impl std::fmt::Display for VerifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VerifyError::HeaderMagicNotPresent => write!(f, "Magic not present"),
            VerifyError::HeaderCRCInvalid => write!(f, "Header CRC invalid"),
            VerifyError::PayloadSizeInvalid => write!(f, "Payload size invalid"),
            VerifyError::PayloadCRCInvalid => write!(f, "Payload CRC invalid")
        }
    }
}

impl Container {
    pub fn verify(&self, payload: Option<&[u8]>) -> Result<(), VerifyError> {
        if self.magic1 != MAGIC1 || self.magic2 != MAGIC2 {
            return Err(VerifyError::HeaderMagicNotPresent);
        }

        if crc(unsafe { any_as_u8_slice(&self.header) }) != self.header_crc {
            return Err(VerifyError::HeaderCRCInvalid);
        }

        if let Some(pay) = payload {
            let v0 = unsafe { &self.header.fields.v0 };

            if v0.payload_size != pay.len() as u32 {
                return Err(VerifyError::PayloadSizeInvalid);
            }

            if crc(pay) != v0.payload_crc {
                return Err(VerifyError::PayloadCRCInvalid);
            }
        }

        Ok(())
    }
}