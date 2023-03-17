use std::mem::size_of;

pub unsafe fn any_as_u8_slice<'a, T: Sized>(p: &'a T) -> &'a [u8] {
    ::std::slice::from_raw_parts((p as *const T) as *const u8, size_of::<T>())
}

pub unsafe fn any_as_u8_slice_mut<'a, T: Sized>(p: &'a mut T) -> &'a mut [u8] {
    ::std::slice::from_raw_parts_mut((p as *mut T) as *mut u8, size_of::<T>())
}
