//! LLVM's `DataLayout`: the sizes, alignments and field offsets a module's
//! `target datalayout` states, with LLVM's defaults for what it leaves out.
//! In MIR it states the program's layout, not a target's.

use std::collections::BTreeMap;

use crate::types::{FloatKind, Type, TypeId, Types};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PointerSpec {
    pub bits: u32,
    /// ABI alignment, in bytes.
    pub align: u64,
    pub index_bits: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DataLayout {
    pub big_endian: bool,
    pointers: BTreeMap<u32, PointerSpec>,
    /// Integer widths with their ABI alignment in bytes.
    ints: BTreeMap<u32, u64>,
    floats: BTreeMap<u32, u64>,
    /// The integer widths the target computes natively: `n`.
    legal: Vec<u32>,
    aggregate_align: u64,
    pub alloca_space: u32,
}

impl Default for DataLayout {
    /// LLVM's defaults, which a datalayout string overrides piecewise.
    fn default() -> Self {
        Self {
            big_endian: false,
            pointers: BTreeMap::from([(0, PointerSpec { bits: 64, align: 8, index_bits: 64 })]),
            ints: BTreeMap::from([(1, 1), (8, 1), (16, 2), (32, 4), (64, 4)]),
            floats: BTreeMap::from([(16, 2), (32, 4), (64, 8), (80, 2), (128, 16)]),
            legal: Vec::new(),
            aggregate_align: 1,
            alloca_space: 0,
        }
    }
}

fn bytes(bits: u64) -> u64 {
    bits.div_ceil(8)
}

impl DataLayout {
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut layout = Self::default();
        for spec in text.split('-').filter(|one| !one.is_empty()) {
            let (head, rest) = spec.split_at(spec.find(|c: char| c.is_ascii_digit() || c == ':').unwrap_or(spec.len()));
            let numbers = |from: &str| -> Result<Vec<u64>, String> {
                from.split(':').filter(|one| !one.is_empty()).map(|one| one.parse().map_err(|_| format!("`{spec}` in the datalayout"))).collect()
            };
            match head {
                "e" => layout.big_endian = false,
                "E" => layout.big_endian = true,
                "n" => layout.legal = numbers(rest)?.into_iter().map(|bits| bits as u32).collect(),
                "m" | "S" | "F" | "G" | "P" | "ni" | "Fi" | "Fn" => {}
                "A" => layout.alloca_space = numbers(rest)?.first().copied().unwrap_or(0) as u32,
                "p" => {
                    let (space, fields) = match rest.split_once(':') {
                        Some((space, fields)) => (space.parse::<u32>().unwrap_or(0), numbers(fields)?),
                        None => return Err(format!("`{spec}` in the datalayout")),
                    };
                    let [size, abi, rest @ ..] = fields.as_slice() else { return Err(format!("`{spec}` in the datalayout")) };
                    let index = rest.get(1).copied().unwrap_or(*size);
                    layout.pointers.insert(space, PointerSpec { bits: *size as u32, align: bytes(*abi), index_bits: index as u32 });
                }
                "i" | "f" | "a" => {
                    let fields = numbers(rest)?;
                    match (head, fields.as_slice()) {
                        ("i", [size, abi, ..]) => drop(layout.ints.insert(*size as u32, bytes(*abi))),
                        ("f", [size, abi, ..]) => drop(layout.floats.insert(*size as u32, bytes(*abi))),
                        ("a", [_, abi, ..]) | ("a", [abi]) => layout.aggregate_align = bytes(*abi).max(1),
                        _ => return Err(format!("`{spec}` in the datalayout")),
                    }
                }
                "v" => {}
                _ => return Err(format!("`{spec}` in the datalayout")),
            }
        }
        Ok(layout)
    }

    /// Whether the target computes `bits`-wide integers natively: LLVM's
    /// `isLegalInteger`, so none without an `n`.
    pub fn legal_integer(&self, bits: u32) -> bool {
        self.legal.contains(&bits)
    }

    /// The widest native integer, in bits: LLVM's
    /// `getLargestLegalIntTypeSizeInBits`, 0 where none is stated.
    pub fn largest_legal_integer(&self) -> u32 {
        self.legal.iter().copied().max().unwrap_or(0)
    }

    pub fn pointer(&self, space: u32) -> PointerSpec {
        self.pointers.get(&space).or_else(|| self.pointers.get(&0)).copied().expect("space 0 always has a pointer")
    }

    /// Whether a pointer in `space` is a selector and an offset: wider than
    /// the space-0 pointer, which is the offset alone.
    pub fn is_pair(&self, space: u32) -> bool {
        self.pointer(space).bits > self.pointer(0).bits
    }

    /// The bits of `space`'s offset arithmetic: a pair's entry is a selector word then an offset word, a flat space's offset is
    /// its whole pointer. The one place the split is stated; an address wraps modulo 2^this.
    pub fn offset_bits(&self, space: u32) -> u32 {
        let bits = self.pointer(space).bits;
        if self.is_pair(space) { bits / 2 } else { bits }
    }

    /// Whether a displacement in `space` carries into the selector: a pair
    /// whose index is wider than its offset, as a huge pointer's.
    pub fn carries(&self, space: u32) -> bool {
        self.is_pair(space) && self.pointer(space).index_bits > self.offset_bits(space)
    }

    /// An integer's ABI alignment: its own entry, else the next wider one's,
    /// else the widest's, as LLVM chooses.
    fn int_align(&self, bits: u32) -> u64 {
        self.ints.range(bits..).next().or_else(|| self.ints.iter().next_back()).map_or(1, |(_, align)| *align)
    }

    pub fn align(&self, types: &Types, ty: TypeId) -> u64 {
        match types.get(ty) {
            Type::Int(bits) => self.int_align(*bits),
            Type::Float(kind) => self.floats[&float_bits(*kind)],
            Type::Pointer(space) => self.pointer(*space).align,
            Type::Array { element, .. } => self.align(types, *element),
            Type::Vector { .. } => self.store_size(types, ty).next_power_of_two(),
            Type::Struct { packed: true, .. } => 1,
            Type::Named(name) if types.body(name).is_some_and(|body| body.packed) => 1,
            Type::Struct { .. } | Type::Named(_) => {
                let fields = types.fields(ty).unwrap_or_default();
                fields.iter().map(|&one| self.align(types, one)).max().unwrap_or(1).max(self.aggregate_align)
            }
            Type::Void | Type::Label | Type::Metadata | Type::Token | Type::Function { .. } => 1,
        }
    }

    /// The bits a value of `ty` holds.
    pub fn size_bits(&self, types: &Types, ty: TypeId) -> u64 {
        match types.get(ty) {
            Type::Int(bits) => u64::from(*bits),
            Type::Float(kind) => u64::from(float_bits(*kind)),
            Type::Pointer(space) => u64::from(self.pointer(*space).bits),
            Type::Vector { element, count } => self.size_bits(types, *element) * u64::from(*count),
            _ => self.store_size(types, ty) * 8,
        }
    }

    /// The bytes a store of `ty` writes.
    pub fn store_size(&self, types: &Types, ty: TypeId) -> u64 {
        match types.get(ty) {
            Type::Array { element, count } => self.alloc_size(types, *element) * count,
            Type::Struct { .. } | Type::Named(_) => self.struct_layout(types, ty).0,
            _ => bytes(self.size_bits(types, ty)),
        }
    }

    /// The bytes between successive elements of an array of `ty`.
    pub fn alloc_size(&self, types: &Types, ty: TypeId) -> u64 {
        self.store_size(types, ty).next_multiple_of(self.align(types, ty))
    }

    /// A struct's size and each field's offset.
    /// What a GEP's indices add to its pointer, as LLVM's `collectOffset`:
    /// a constant, and each variable index's scale by its position. A
    /// variable index is `None`; a struct's index never is.
    pub fn collect_offset(&self, types: &Types, source: TypeId, indices: &[Option<i128>]) -> (i128, Vec<(usize, u64)>) {
        let mut constant = 0;
        let mut variable = Vec::new();
        let mut current = source;
        for (at, index) in indices.iter().enumerate() {
            let scale = match (at, types.get(current)) {
                (0, _) => self.alloc_size(types, current),
                (_, Type::Array { element, .. } | Type::Vector { element, .. }) => {
                    current = *element;
                    self.alloc_size(types, current)
                }
                _ => {
                    let field = index.expect("a struct's index is a constant");
                    let (_, offsets) = self.struct_layout(types, current);
                    constant += offsets[field as usize] as i128;
                    current = types.member(current, field as u64).expect("a verified field");
                    continue;
                }
            };
            match index {
                Some(index) => constant += index * scale as i128,
                None => variable.push((at, scale)),
            }
        }
        (constant, variable)
    }

    pub fn struct_layout(&self, types: &Types, ty: TypeId) -> (u64, Vec<u64>) {
        let packed = match types.get(ty) {
            Type::Struct { packed, .. } => *packed,
            Type::Named(name) => types.body(name).is_some_and(|body| body.packed),
            _ => false,
        };
        let mut offset: u64 = 0;
        let mut offsets = Vec::new();
        for &field in types.fields(ty).unwrap_or_default() {
            if !packed {
                offset = offset.next_multiple_of(self.align(types, field));
            }
            offsets.push(offset);
            offset += self.alloc_size(types, field);
        }
        (offset.next_multiple_of(self.align(types, ty)), offsets)
    }
}

pub fn float_bits(kind: FloatKind) -> u32 {
    match kind {
        FloatKind::Float => 32,
        FloatKind::Double => 64,
        FloatKind::X86Fp80 => 80,
    }
}

#[cfg(test)]
mod tests {
    use super::DataLayout;

    /// `offset_bits` was a pair's half for every space: a flat 16-bit space said 8, so a caller that wrapped a flat offset by it
    /// (a frame index into a 32 KB object) wrapped at 256.
    #[test]
    fn a_flat_spaces_offset_bits_are_its_pointer_bits_and_a_pairs_are_half() {
        let layout = DataLayout::parse("e-p:16:16-p1:32:16:16:16-p3:32:16:16:32").unwrap();
        assert_eq!((layout.offset_bits(0), layout.offset_bits(1)), (16, 16));
        assert_eq!(layout.offset_bits(3), 16);
    }
}
