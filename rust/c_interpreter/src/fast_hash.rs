use std::collections;
use std::hash::{BuildHasherDefault, Hasher};

#[derive(Default)]
pub(crate) struct FastHasher {
    state: usize,
}

impl FastHasher {
    #[inline]
    fn mix(&mut self, value: usize) {
        #[cfg(target_pointer_width = "64")]
        const K: usize = 0x517c_c1b7_2722_0a95;
        #[cfg(target_pointer_width = "32")]
        const K: usize = 0x9e37_79b9;

        self.state = (self.state.rotate_left(5) ^ value).wrapping_mul(K);
    }
}

impl Hasher for FastHasher {
    #[inline]
    fn finish(&self) -> u64 {
        self.state as u64
    }

    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        self.mix(bytes.len());
        let word_size = std::mem::size_of::<usize>();
        let mut chunks = bytes.chunks_exact(word_size);
        for chunk in &mut chunks {
            let mut word = [0u8; std::mem::size_of::<usize>()];
            word.copy_from_slice(chunk);
            self.mix(usize::from_le_bytes(word));
        }
        let remainder = chunks.remainder();
        if !remainder.is_empty() {
            let mut word = [0u8; std::mem::size_of::<usize>()];
            word[..remainder.len()].copy_from_slice(remainder);
            self.mix(usize::from_le_bytes(word));
        }
    }

    #[inline]
    fn write_u8(&mut self, i: u8) {
        self.mix(i as usize);
    }

    #[inline]
    fn write_u16(&mut self, i: u16) {
        self.mix(i as usize);
    }

    #[inline]
    fn write_u32(&mut self, i: u32) {
        self.mix(i as usize);
    }

    #[inline]
    fn write_u64(&mut self, i: u64) {
        #[cfg(target_pointer_width = "64")]
        {
            self.mix(i as usize);
        }
        #[cfg(target_pointer_width = "32")]
        {
            self.mix(i as usize);
            self.mix((i >> 32) as usize);
        }
    }

    #[inline]
    fn write_u128(&mut self, i: u128) {
        self.write_u64(i as u64);
        self.write_u64((i >> 64) as u64);
    }

    #[inline]
    fn write_usize(&mut self, i: usize) {
        self.mix(i);
    }

    #[inline]
    fn write_i8(&mut self, i: i8) {
        self.mix(i as usize);
    }

    #[inline]
    fn write_i16(&mut self, i: i16) {
        self.mix(i as usize);
    }

    #[inline]
    fn write_i32(&mut self, i: i32) {
        self.mix(i as usize);
    }

    #[inline]
    fn write_i64(&mut self, i: i64) {
        self.write_u64(i as u64);
    }

    #[inline]
    fn write_i128(&mut self, i: i128) {
        self.write_u128(i as u128);
    }

    #[inline]
    fn write_isize(&mut self, i: isize) {
        self.mix(i as usize);
    }
}

pub(crate) type FastHashMap<K, V> = collections::HashMap<K, V, BuildHasherDefault<FastHasher>>;
pub(crate) type FastHashSet<T> = collections::HashSet<T, BuildHasherDefault<FastHasher>>;
