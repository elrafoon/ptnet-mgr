use std::mem::size_of;
use packet::buffer::Buffer;

pub unsafe fn any_as_u8_slice<'a, T: Sized>(p: &'a T) -> &'a [u8] {
    ::std::slice::from_raw_parts((p as *const T) as *const u8, size_of::<T>())
}

pub unsafe fn any_as_u8_slice_mut<'a, T: Sized>(p: &'a mut T) -> &'a mut [u8] {
    ::std::slice::from_raw_parts_mut((p as *mut T) as *mut u8, size_of::<T>())
}

pub fn to_buffer<BUF: AsMut<[u8]>, ITEM: Sized>(buffer: &mut dyn Buffer<Inner = BUF>, item: &ITEM) -> Result<(), packet::Error> {
    buffer.next(size_of::<ITEM>())?;
    unsafe {
        buffer.data_mut().copy_from_slice(any_as_u8_slice(item));
    }
    Ok(())
}