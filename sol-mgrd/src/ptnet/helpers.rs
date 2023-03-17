use std::mem::size_of;
use packet::buffer::Buffer;
use sol_lib::helpers::any_as_u8_slice;

pub fn to_buffer<BUF: AsMut<[u8]>, ITEM: Sized>(buffer: &mut dyn Buffer<Inner = BUF>, item: &ITEM) -> Result<(), packet::Error> {
    buffer.next(size_of::<ITEM>())?;
    unsafe {
        buffer.data_mut().copy_from_slice(any_as_u8_slice(item));
    }
    Ok(())
}