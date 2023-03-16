use packet::buffer::Buffer;
use super::ptnet_c;
use super::helpers::*;

pub struct PtNetPacket<'a,T: AsMut<[u8]>> {
    buffer: &'a mut dyn Buffer<Inner = T>
}

impl<'a,T: AsMut<[u8]>> PtNetPacket<'a,T> {
    pub fn with_asdh(asdh: &ptnet_c::ASDH, buffer: &'a mut dyn Buffer<Inner = T>) -> Result<PtNetPacket<'a, T>, packet::Error> {
        to_buffer(buffer, asdh)?;
        Ok(PtNetPacket::new(buffer))
    }

    fn new(buffer: &'a mut dyn Buffer<Inner = T>) -> Self {
        Self {
            buffer: buffer
        }
    }

    pub fn begin_asdu(&mut self, dui: &ptnet_c::DUI) -> Result<&mut Self, packet::Error> {
        self.to_buffer(dui)
    }

    pub fn add_ioa(&mut self, ioa: ptnet_c::IOA) -> Result<&mut Self, packet::Error> {
        self.to_buffer(&ioa)
    }

    pub fn add_ie<IE: Sized>(&mut self, ie: &IE) -> Result<&mut Self, packet::Error> {
        self.to_buffer(ie)
    }

    pub fn end_asdu(&mut self) -> Result<&mut Self, packet::Error> {
        Ok(self)
    }

    fn to_buffer<ITEM: Sized>(&mut self, item: &ITEM) -> Result<&mut Self, packet::Error> {
        to_buffer(self.buffer, item)?;
        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use crate::ptnet::{DUIConstruct, ASDHConstruct, COT};

    use super::*;

    #[test]
    fn it_build() {
        let mut buf = packet::buffer::Dynamic::new();
        let mut packet = PtNetPacket::with_asdh(
            &ptnet_c::ASDH::with(10, COT::SPONT, false),
            &mut buf
        ).unwrap();

        packet.begin_asdu(&ptnet_c::DUI::with_direct(ptnet_c::TC_C_RD, 1, false)).unwrap();
        packet.add_ioa(100).unwrap();
        packet.add_ie(&ptnet_c::TI25::default()).unwrap();
        packet.end_asdu().unwrap();

        packet.begin_asdu(&ptnet_c::DUI::with_direct(ptnet_c::TC_M_ME_FP, 5, true)).unwrap();
        packet.add_ioa(200).unwrap();
        packet.add_ie(&ptnet_c::TI131 { value: 3.4 }).unwrap();
        packet.add_ie(&ptnet_c::TI131 { value: 4.4 }).unwrap();
        packet.add_ie(&ptnet_c::TI131 { value: 5.4 }).unwrap();
        packet.add_ie(&ptnet_c::TI131 { value: 6.4 }).unwrap();
        packet.add_ie(&ptnet_c::TI131 { value: 7.4 }).unwrap();
        packet.end_asdu().unwrap();

        let exp_packet: &[u8] = &[
            10, 3,                  // ASDH (CA,COT)
            25, 1,                  // DUI (TI,VSQ)
            100,                    // IOA
            131, 0x15,              // DUI (TI,VSQ)
            200,                    // IOA
            0x9a, 0x99, 0x59, 0x40, // TI131 @ 200
            0xcd, 0xcc, 0x8c, 0x40, // TI131 @ 201
            0xcd, 0xcc, 0xac, 0x40, // TI131 @ 202
            0xcd, 0xcc, 0xcc, 0x40, // TI131 @ 203
            0xcd, 0xcc, 0xec, 0x40, // TI131 @ 204
        ];

        assert_eq!(buf.into_inner().as_slice(), exp_packet);
    }
}