use serde_bytes::ByteBuf;

pub fn create_buffer(len: usize) -> Box<[u8]> {
    if len == 0 {
        return <Box<[u8]>>::default();
    }
    let layout = std::alloc::Layout::array::<u8>(len).unwrap();
    let ptr = unsafe { std::alloc::alloc_zeroed(layout) };
    let slice_ptr = core::ptr::slice_from_raw_parts_mut(ptr, len);
    unsafe { Box::from_raw(slice_ptr) }
}

pub fn create_struct_buffer<T: Sized>(len: usize) -> Box<[T]> {
    if len == 0 {
        return <Box<[T]>>::default();
    }
    let layout = std::alloc::Layout::array::<T>(len).unwrap();
    let ptr = unsafe { std::alloc::alloc_zeroed(layout) as *mut T };
    let slice_ptr = core::ptr::slice_from_raw_parts_mut(ptr, len);
    unsafe { Box::from_raw(slice_ptr) }
}

pub fn encode_as_hex_string(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| hex_string::u8_to_hex_string(b))
        .fold(String::new(), |mut acc, s| {
            acc.push(s[0]);
            acc.push(s[1]);
            acc
        })
}

pub trait IntoHexString {
    fn into_hex_string(&self) -> String;
}

impl IntoHexString for ByteBuf {
    fn into_hex_string(&self) -> String {
        encode_as_hex_string(self.as_slice())
    }
}

impl IntoHexString for [u8] {
    fn into_hex_string(&self) -> String {
        encode_as_hex_string(self)
    }
}

pub trait ApplyTransform: Sized {
    #[inline]
    fn apply<T>(self, f: impl FnOnce(Self) -> T) -> T {
        (f)(self)
    }
}
impl<TAny> ApplyTransform for TAny {}

pub trait UnifyError<TErr: ToString> {
    type IntoResult;
    fn unify_error(self, id: usize) -> Self::IntoResult;
}

impl<T, TErr: ToString> UnifyError<TErr> for Result<T, TErr> {
    type IntoResult = Result<T, (usize, String)>;

    fn unify_error(self, id: usize) -> Self::IntoResult {
        self.map_err(|e| (id, e.to_string()))
    }
}

pub fn unimpl_create<T>() -> T {
    unimplemented!()
}

pub fn unimpl_get_from<TFrom, TGet>(_: TFrom) -> TGet {
    unimplemented!()
}

pub fn unimpl_get_ref<TFrom, TGet>(_: &TFrom) -> &TGet {
    unimplemented!()
}

pub fn unimpl_get_mut<TFrom, TGet>(_: &mut TFrom) -> &mut TGet {
    unimplemented!()
}