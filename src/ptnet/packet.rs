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
    use super::*;

    #[test]
    fn it_build() {
        let mut buf = packet::buffer::Dynamic::new();
        let mut packet = PtNetPacket::new(&mut buf);
        let _ti = ptnet_c::TI { raw: ptnet_c::TC_C_RD };

        packet.begin_asdu(&ptnet_c::DUI {
            ti: ptnet_c::TI { raw: ptnet_c::TC_C_RD },
            vsq: ptnet_c::VSQ { raw: 1 }
        }).unwrap();
    }
}