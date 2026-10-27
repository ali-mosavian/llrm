//! Bytes with the relocations laid in them, and DWARF's variable-length
//! integers.

use llrm_object::Reloc;

/// `value` as ULEB128.
pub fn uleb(mut value: u64) -> Vec<u8> {
    let mut out = Vec::new();
    loop {
        let byte = (value & 0x7F) as u8;
        value >>= 7;
        if value == 0 {
            out.push(byte);
            return out;
        }
        out.push(byte | 0x80);
    }
}

/// `value` as SLEB128.
pub fn sleb(mut value: i64) -> Vec<u8> {
    let mut out = Vec::new();
    loop {
        let byte = (value & 0x7F) as u8;
        value >>= 7;
        let done = (value == 0 && byte & 0x40 == 0) || (value == -1 && byte & 0x40 != 0);
        if done {
            out.push(byte);
            return out;
        }
        out.push(byte | 0x80);
    }
}

#[derive(Clone, Default)]
pub struct Done {
    pub bytes: Vec<u8>,
    pub relocs: Vec<Reloc>,
}

#[derive(Default)]
pub struct Buf {
    pub bytes: Vec<u8>,
    pub relocs: Vec<Reloc>,
}

impl Buf {
    pub fn at(&self) -> usize {
        self.bytes.len()
    }

    pub fn u8(
        &mut self,
        value: u8,
    ) {
        self.bytes.push(value);
    }

    pub fn u16(
        &mut self,
        value: u16,
    ) {
        self.bytes.extend(value.to_le_bytes());
    }

    pub fn u32(
        &mut self,
        value: u32,
    ) {
        self.bytes.extend(value.to_le_bytes());
    }

    pub fn uleb(
        &mut self,
        value: u64,
    ) {
        self.bytes.extend(uleb(value));
    }

    pub fn sleb(
        &mut self,
        value: i64,
    ) {
        self.bytes.extend(sleb(value));
    }

    pub fn string(
        &mut self,
        text: &str,
    ) {
        self.bytes.extend(text.as_bytes());
        self.bytes.push(0);
    }

    /// A 4-byte offset into section `section`, which the linker places.
    pub fn section_offset(
        &mut self,
        section: usize,
        value: u32,
    ) {
        self.relocs.push(crate::section_reloc(self.bytes.len(), section, i64::from(value)));
        self.u32(0);
    }

    /// A 2-byte segment selector of section `section`.
    pub fn segment(
        &mut self,
        section: usize,
    ) {
        self.relocs.push(crate::segment_reloc(self.bytes.len(), section));
        self.u16(0);
    }

    /// An address field of `width` bytes: `offset` into the section `symbol` is
    /// at `symbol_offset`.
    pub fn address(
        &mut self,
        width: usize,
        symbol: usize,
        delta: i64,
    ) {
        self.relocs.push(crate::symbol_reloc(self.bytes.len(), width, symbol, delta));
        self.bytes.extend(std::iter::repeat_n(0, width));
    }

    /// `at` patched with `value`, a 4-byte length known only now.
    pub fn patch32(
        &mut self,
        at: usize,
        value: u32,
    ) {
        self.bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }

    pub fn done(self) -> Done {
        Done { bytes: self.bytes, relocs: self.relocs }
    }
}

/// A string section: each text once, NUL terminated, found again by its offset.
#[derive(Default)]
pub struct Strings {
    bytes: Vec<u8>,
    seen: llrm_support::hash::HashMap<String, u32>,
}

impl Strings {
    pub fn add(
        &mut self,
        text: &str,
    ) -> u32 {
        if let Some(&at) = self.seen.get(text) {
            return at;
        }
        let at = self.bytes.len() as u32;
        self.bytes.extend(text.as_bytes());
        self.bytes.push(0);
        self.seen.insert(text.to_owned(), at);
        at
    }

    pub fn part(&self) -> Done {
        Done { bytes: self.bytes.clone(), relocs: Vec::new() }
    }
}
