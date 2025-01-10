use std::mem::ManuallyDrop;

use bitvec::vec::BitVec;
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

pub struct FixedCache<T, const N: usize> {
    cursor: usize,
    occupied: [bool; N],
    ids: [usize; N],
    slots: [ManuallyDrop<T>; N],
}

impl<T, const N: usize> Drop for FixedCache<T, N> {
    fn drop(&mut self) {
        self.clear();
    }
}

impl<T, const N: usize> FixedCache<T, N> {
    pub fn new() -> Self {
        Self {
            cursor: 0,
            occupied: [false; N],
            ids: [0; N],
            slots: unsafe { std::mem::MaybeUninit::uninit().assume_init() },
        }
    }

    #[inline(always)]
    unsafe fn drop_slot_item(&mut self, slot_i: usize) {
        println!("Dropping item at slot {slot_i}");
        ManuallyDrop::drop(&mut self.slots[slot_i]);
    }

    pub fn save(&mut self, id: usize, item: T) -> &T {
        let slot_i = match self.ids.iter().position(|this_id| *this_id == id) {
            // id found, reuse the same slot
            Some(slot_i) => slot_i,
            // id not found, replace oldest cache item
            None => {
                let slot_i = self.cursor;
                self.cursor = (self.cursor + 1) % N;
                slot_i
            }
        };

        if self.occupied[slot_i] {
            unsafe {
                self.drop_slot_item(slot_i);
            }
        }

        self.occupied[slot_i] = true;
        self.ids[slot_i] = id;
        self.slots[slot_i] = ManuallyDrop::new(item);

        &self.slots[slot_i]
    }

    pub fn read(&self, id: usize) -> Option<&T> {
        let slot_i = self.ids.iter().position(|this_id| *this_id == id)?;
        if !self.occupied[slot_i] {
            return None;
        }
        Some(&self.slots[slot_i])
    }

    pub fn remove(&mut self, id: usize) -> bool {
        let Some(slot_i) = self.ids.iter().position(|this_id| *this_id == id) else {
            return false;
        };
        if !self.occupied[slot_i] {
            return false;
        }
        unsafe {
            self.drop_slot_item(slot_i);
        }
        true
    }

    pub fn clear(&mut self) {
        for slot_i in 0..N {
            if self.occupied[slot_i] {
                unsafe {
                    self.drop_slot_item(slot_i);
                }
            }
        }
    }
}

#[macro_export]
macro_rules! cache_read {
    ($cache: expr, $id: expr, $or_else_save: expr) => {
        match ($cache).read(($id)) {
            Some(item) => item,
            None => {
                let item = ($or_else_save);
                ($cache).save(($id), item)
            },
        }
    };
}

// let piece_buffer = match self.piece_cache.read(index) {
//     Some(buffer) => buffer,
//     None => {
//         self.file
//             .seek(SeekFrom::Start(self.get_cursor_pos(index, 0)))
//             .await?;

//         let mut piece_buffer = create_buffer(this_piece_length);
//         self.file.read_exact(&mut piece_buffer).await?;

//         self.piece_cache.save(index, piece_buffer)
//     },
// };

pub struct _FixedQueue<T, const N: usize> {
    head: usize,
    len: usize,
    items: [T; N],
}

impl<T, const N: usize> _FixedQueue<T, N> {
    pub fn new_empty() -> Self {
        Self {
            head: 0,
            len: 0,
            items: unsafe { std::mem::MaybeUninit::uninit().assume_init() }, // TODO
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn is_full(&self) -> bool {
        self.len == N
    }

    pub fn push(&mut self, item: T) -> bool {
        if self.len == N {
            return false;
        }
        let tail = (self.head + self.len) % N;
        self.items[tail] = item;
        return true;
    }

    pub fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            return None;
        }
        let item = unsafe { self.items.as_ptr().add(self.head).read() };
        self.len -= 1;
        self.head = (self.head + 1) % N;
        Some(item)
    }
}

pub fn bitcount_to_bytecount(bit_count: usize) -> usize {
    (bit_count / 8) + ((bit_count % 8) != 0) as usize
}

pub trait BitVecU8Ext {
    fn into_u8_vec(&self) -> Vec<u8>;
}

impl BitVecU8Ext for BitVec<u8> {
    fn into_u8_vec(&self) -> Vec<u8> {
        self.domain().collect()
    }
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
