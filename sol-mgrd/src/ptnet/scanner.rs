use std::mem::size_of;
use sol_lib::helpers::any_as_u8_slice_mut;

use super::{ASDH, DUI, IOA, COT_U_TI, COT_U_COT, COT_U_IOA, VSQBits, TIBits, IE};

enum State {
    ScanASDH,
    ScanDUI,
    ScanIOA,
    ScanIE
}

#[derive(Clone,Debug,PartialEq)]
pub enum Token {
    ASDH(ASDH),
    DUI(DUI),
    IOA(IOA),
    IE(IE)
}

pub struct Scanner<'a> {
    state: State,
    packet: &'a [u8],
    ioa: u8,
    ies_remaining: u8,
    pos: usize,
    /// packet asdh
    asdh: ASDH,
    /// latest dui
    dui: DUI
}

#[derive(Copy,Clone,Debug,PartialEq)]
pub enum Error<'a> {
    /// EOF reached, packet parsed
    EOF,
    ShortRead,
    InvalidPacket(&'a str)
}

impl<'a> Scanner<'a> {
    pub fn new(packet: &'a [u8]) -> Self {
        Scanner {
            state: State::ScanASDH,
            packet: packet,
            ioa: 0,
            ies_remaining: 0,
            pos: 0,
            asdh: Default::default(),
            dui: Default::default()
        }
    }

    pub fn next_token(&mut self) -> Result<Token, Error<'a>> {
        let rem = self.packet.len() - self.pos;
        match self.state {
            State::ScanASDH => {
                if rem < size_of::<ASDH>() {
                    return Err(Error::ShortRead);
                }

                // asdh available, save
                unsafe {
                    any_as_u8_slice_mut(&mut self.asdh)
                    .copy_from_slice(&self.packet[self.pos..(self.pos + size_of::<ASDH>())]);
                }

                self.pos += size_of::<ASDH>();
                self.state = State::ScanDUI;

                return Ok(Token::ASDH(self.asdh));
            },
            State::ScanDUI => {
                if rem == 0 {
                    // successfully reached EOF
                    return Err(Error::EOF);
                } else if rem < size_of::<DUI>() {
                    return Err(Error::ShortRead);
                }

                // dui available, save
                unsafe {
                    any_as_u8_slice_mut(&mut self.dui)
                    .copy_from_slice(&self.packet[self.pos..(self.pos + size_of::<DUI>())]);
                }

                self.ies_remaining = self.dui.vsq.n();
                if self.ies_remaining == 0 {
                    return Err(Error::InvalidPacket("VSQ.N zero"));
                }

                self.pos += size_of::<DUI>();
                self.state = State::ScanIOA;

                return Ok(Token::DUI(self.dui));
            },
            State::ScanIOA => {
                if rem < size_of::<IOA>() {
                    return Err(Error::ShortRead);
                }

                // ioa available
                unsafe {
                    any_as_u8_slice_mut(&mut self.ioa)
                    .copy_from_slice(&self.packet[self.pos..(self.pos + size_of::<IOA>())]);
                }

                self.pos += size_of::<IOA>();

                match self.asdh.cot() as u32 {
                    COT_U_TI | COT_U_COT | COT_U_IOA => {
                        // ASDU with this COTs does not carry any information elements and shall carry only one IOB/IE
                        self.ies_remaining -= 1;
                        self.state = State::ScanDUI;
                    },
                    _ => {
                        self.state = State::ScanIE
                    }
                };

                return Ok(Token::IOA(self.ioa));
            },
            State::ScanIE => {
                if rem < self.dui.ti.size() as usize {
                    return Err(Error::ShortRead);
                }

                // IE available
                let ie_buf = &self.packet[self.pos..(self.pos + self.dui.ti.size() as usize)];

                self.pos += self.dui.ti.size() as usize;
                self.ies_remaining -= 1;

                if self.ies_remaining > 0 {
                    if self.dui.vsq.sq() {
                        self.state = State::ScanIE;
                        self.ioa += 1;
                    } else {
                        self.state = State::ScanIOA;
                    }
                } else {
                    self.state = State::ScanDUI;
                }

                return Ok(Token::IE(IE::try_from((self.dui.ti.value(), ie_buf)).map_err(|_e| Error::InvalidPacket("IE invalid"))?));
            }
        };
    }

    pub fn into_iob_iter(self) -> ScannerIntoIOBIterator<'a> {
        ScannerIntoIOBIterator { scanner: self, asdh: None, dui: None, ioa: None }
    }
}

impl<'a> IntoIterator for Scanner<'a> {
    type Item = Result<Token, Error<'a>>;
    type IntoIter = ScannerIntoIterator<'a>;

    fn into_iter(self) -> Self::IntoIter {
        ScannerIntoIterator {
            scanner: self
        }
    }
}

pub struct ScannerIntoIterator<'a> {
    scanner: Scanner<'a>
}

impl<'a> Iterator for ScannerIntoIterator<'a> {
    type Item = Result<Token, Error<'a>>;

    fn next(&mut self) -> Option<Self::Item> {
        let res = self.scanner.next_token();
        match &res {
            Ok(_) => Some(res),
            Err(err) => match err {
                Error::EOF => None,
                _ => Some(res)
            },
        }
    }
}

#[derive(Clone,Debug,PartialEq)]
pub struct IOB {
    pub asdh: ASDH,
    pub dui: DUI,
    pub ioa: IOA,
    pub ie: IE
}

pub struct ScannerIntoIOBIterator<'a> {
    scanner: Scanner<'a>,
    asdh: Option<ASDH>,
    dui: Option<DUI>,
    ioa: Option<IOA>,
}

impl<'a> Iterator for ScannerIntoIOBIterator<'a> {
    type Item = Result<IOB, Error<'a>>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let res = self.scanner.next_token();
            match &res {
                Ok(tok) => match tok {
                    Token::ASDH(asdh) => { self.asdh = Some(*asdh) },
                    Token::DUI(dui) => { self.dui = Some(*dui) },
                    Token::IOA(ioa) => { self.ioa = Some(*ioa) },
                    Token::IE(ie) => {
                        if let Some(asdh) = self.asdh {
                            if let Some(dui) = self.dui {
                                if let Some(ioa) = self.ioa {
                                    if dui.vsq.sq() {
                                        self.ioa = Some(ioa + 1);
                                    } else {
                                        self.ioa = None;
                                    }

                                    return Some(Ok(IOB {
                                        asdh: asdh,
                                        dui: dui,
                                        ioa: ioa,
                                        ie: ie.clone()
                                    }));
                                } else {
                                    return Some(Err(Error::InvalidPacket("Missing IOA")));
                                }
                            } else {
                                return Some(Err(Error::InvalidPacket("Missing DUI")));
                            }
                        } else {
                            return Some(Err(Error::InvalidPacket("Missing ASDH")));
                        }
                    },
                },
                Err(err) => match err {
                    Error::EOF => return None,
                    _ => return Some(Err(*err))
                },
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::ptnet::{ASDHConstruct, COT, DUIConstruct, TI161, TI34};
    use super::*;

    const PKT1: &[u8] = &[
        10, 5,                          // ASDH
        161, 3,                         // DUI=TI161, 3x IOB
        100,                            // IOA=100
        0xEF,0xBE, 0xED, 0xFE, 0x80,    // 0xFEEDBEEF, QDS=IV
        110,                            // IOA=110
        0x67, 0x45, 0x23, 0x01, 0x00,   // 0x01234567, QDS=0
        120,                            // IOA=120
        0x40, 0x30, 0x20, 0x10, 0xC0,   // 0x10203040, QDS=IV|NT
    ];

    // expected parser results starting with IOA token
    const PKT1_EXP_FROM_IOA: &[Token] = &[
        Token::IOA(100),
        Token::IE(IE::TI161(TI161 { value: 0xFEEDBEEF, qds: 0x80 })),
        Token::IOA(110),
        Token::IE(IE::TI161(TI161 { value: 0x01234567, qds: 0x00 })),
        Token::IOA(120),
        Token::IE(IE::TI161(TI161 { value: 0x10203040, qds: 0xC0 })),
    ];

    const PKT2: &[u8] = &[
        0, 3,                           // ASDH
        34, 0x15,                       // DUI=TI34, SEQ(5)
        50,                             // IOA=50
        0x10, 0x20, 0x30, 0x40, 0x50    // TI34(10),TI34(20),TI34(30),TI34(40),TI34(50),
    ];

    // expected parser results starting with IOA token
    const PKT2_EXP_FROM_IOA: &[Token] = &[
        Token::IOA(50),
        Token::IE(IE::TI34(TI34 { value: 0x10 })),
        Token::IE(IE::TI34(TI34 { value: 0x20 })),
        Token::IE(IE::TI34(TI34 { value: 0x30 })),
        Token::IE(IE::TI34(TI34 { value: 0x40 })),
        Token::IE(IE::TI34(TI34 { value: 0x50 })),
    ];

    #[test]
    fn it_parse_3x_161_no_sq() {
        let asdh_dui: &[Token] = &[
            Token::ASDH(ASDH::with(10, COT::REQ, false)),
            Token::DUI(DUI::with_direct(161, 3, false))
        ];

        let mut scanner = Scanner::new(PKT1);

        for tok in asdh_dui.iter().chain(PKT1_EXP_FROM_IOA.iter()) {
            let next_token = scanner.next_token().unwrap();
            assert_eq!(next_token, *tok);
        }

        assert_eq!(scanner.next_token(), Result::Err(Error::EOF));
    }

    #[test]
    fn it_parse_5x_34_sq() {
        let asdh_dui: &[Token] = &[
            Token::ASDH(ASDH::with(0, COT::SPONT, false)),
            Token::DUI(DUI::with_direct(34, 5, true))
        ];

        let mut scanner = Scanner::new(PKT2);

        for tok in asdh_dui.iter().chain(PKT2_EXP_FROM_IOA.iter()) {
            let next_token = scanner.next_token().unwrap();
            assert_eq!(next_token, *tok);
        }

        assert_eq!(scanner.next_token(), Result::Err(Error::EOF));
    }

    #[test]
    fn it_parse_2_dui() {
        let exp_asdh_dui_1: &[Token] = &[
            Token::ASDH(ASDH::with(10, COT::REQ, false)),
            Token::DUI(DUI::with_direct(161, 3, false))
        ];

        let exp_dui_2: &[Token] = &[
            Token::DUI(DUI::with_direct(34, 5, true))
        ];

        let pkt = PKT1.iter()
            .chain(PKT2[2..].iter()).map(|e| *e).collect::<Vec<u8>>();

        let mut scanner = Scanner::new(&pkt[..]);

        let exp = exp_asdh_dui_1.iter()
            .chain(PKT1_EXP_FROM_IOA.iter())
            .chain(exp_dui_2.iter())
            .chain(PKT2_EXP_FROM_IOA.iter());

        for tok in exp {
            let next_token = scanner.next_token().unwrap();
            assert_eq!(next_token, *tok);
        }

        assert_eq!(scanner.next_token(), Result::Err(Error::EOF));
    }

    #[test]
    fn it_parse_iterator() {
        let asdh_dui: &[Token] = &[
            Token::ASDH(ASDH::with(10, COT::REQ, false)),
            Token::DUI(DUI::with_direct(161, 3, false))
        ];

        let scanner = Scanner::new(PKT1);

        let iter_exp: Vec<Token> = asdh_dui.iter()
            .chain(PKT1_EXP_FROM_IOA.iter())
            .map(|e| e.clone())
            .collect();
        let iter_scn: Vec<Token> = scanner.into_iter()
            .map(|res| res.unwrap())
            .collect();

        assert_eq!(iter_exp, iter_scn);
    }

    #[test]
    fn it_parse_iob_iterator() {
        let pkt_1_exp_iobs: &[IOB] = &[
            IOB {
                asdh: ASDH::with(10, COT::REQ, false),
                dui: DUI::with_direct(161, 3, false),
                ie: IE::TI161(TI161 { value: 0xFEEDBEEF, qds: 0x80 }),
                ioa: 100
            },
            IOB {
                asdh: ASDH::with(10, COT::REQ, false),
                dui: DUI::with_direct(161, 3, false),
                ie: IE::TI161(TI161 { value: 0x01234567, qds: 0x00 }),
                ioa: 110
            },
            IOB {
                asdh: ASDH::with(10, COT::REQ, false),
                dui: DUI::with_direct(161, 3, false),
                ie: IE::TI161(TI161 { value: 0x10203040, qds: 0xC0 }),
                ioa: 120
            },
        ];

        let pkt_2_exp_iobs: &[IOB] = &[
            IOB {
                asdh: ASDH::with(10, COT::REQ, false),
                dui: DUI::with_direct(34, 5, true),
                ie: IE::TI34(TI34 { value: 0x10 }),
                ioa: 50
            },
            IOB {
                asdh: ASDH::with(10, COT::REQ, false),
                dui: DUI::with_direct(34, 5, true),
                ie: IE::TI34(TI34 { value: 0x20 }),
                ioa: 51
            },
            IOB {
                asdh: ASDH::with(10, COT::REQ, false),
                dui: DUI::with_direct(34, 5, true),
                ie: IE::TI34(TI34 { value: 0x30 }),
                ioa: 52
            },
            IOB {
                asdh: ASDH::with(10, COT::REQ, false),
                dui: DUI::with_direct(34, 5, true),
                ie: IE::TI34(TI34 { value: 0x40 }),
                ioa: 53
            },
            IOB {
                asdh: ASDH::with(10, COT::REQ, false),
                dui: DUI::with_direct(34, 5, true),
                ie: IE::TI34(TI34 { value: 0x50 }),
                ioa: 54
            },
        ];

        let pkt = PKT1.iter()
            .chain(PKT2[2..].iter())
            .map(|e| *e)
            .collect::<Vec<u8>>();
        let scanner = Scanner::new(&pkt[..]);

        let iter_exp: Vec<IOB> = pkt_1_exp_iobs.iter()
            .chain(pkt_2_exp_iobs.iter())
            .map(|e| e.clone())
            .collect();
        let iter_scn: Vec<IOB> = scanner.into_iob_iter()
            .map(|res| res.unwrap())
            .collect();

        assert_eq!(iter_exp, iter_scn);
    }
}