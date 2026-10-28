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
    /// Preferred alignment, in bytes: the ABI one unless stated.
    pub pref: u64,
    pub index_bits: u32,
    /// `HasUnstableRepresentation`: set by `ni:` ([`DataLayout::parse`]).
    pub unstable: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DataLayout {
    pub big_endian: bool,
    pointers: BTreeMap<u32, PointerSpec>,
    /// Integer widths with their ABI alignment in bytes.
    ints: BTreeMap<u32, u64>,
    floats: BTreeMap<u32, u64>,
    /// The preferred alignments of the integer and float widths, in bytes.
    int_prefs: BTreeMap<u32, u64>,
    float_prefs: BTreeMap<u32, u64>,
    /// The integer widths the target computes natively: `n`.
    legal: Vec<u32>,
    aggregate_align: u64,
    /// The preferred alignment of an aggregate, in bytes: `a:0:64`.
    aggregate_pref: u64,
    pub alloca_space: u32,
    /// The natural stack alignment in bytes: `S32`.
    pub stack_align: Option<u64>,
    layouts: StructLayouts,
}

/// LLVM's `StructLayouts` map: a struct's layout, made once. It is held for
/// one `Types` (by address and size, so another context's ids do not read it)
/// and only for a struct whose body is defined.
#[derive(Default)]
struct StructLayouts(
    std::sync::Mutex<(usize, usize, usize, crate::hash::HashMap<TypeId, std::sync::Arc<(u64, Vec<u64>)>>)>,
);

impl Clone for StructLayouts {
    fn clone(&self) -> Self {
        Self::default()
    }
}

impl PartialEq for StructLayouts {
    fn eq(
        &self,
        _: &Self,
    ) -> bool {
        true
    }
}

impl Eq for StructLayouts {}

impl std::fmt::Debug for StructLayouts {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        f.write_str("StructLayouts")
    }
}

thread_local! {
    static COMPUTED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many struct layouts this thread has worked out, for a test that each is
/// made once.
pub fn layouts_computed() -> usize {
    COMPUTED.with(std::cell::Cell::get)
}

impl Default for DataLayout {
    /// LLVM's defaults, which a datalayout string overrides piecewise.
    fn default() -> Self {
        Self {
            big_endian: false,
            pointers: BTreeMap::from([(
                0,
                PointerSpec { bits: 64, align: 8, pref: 8, index_bits: 64, unstable: false },
            )]),
            ints: BTreeMap::from([(1, 1), (8, 1), (16, 2), (32, 4), (64, 4)]),
            floats: BTreeMap::from([(16, 2), (32, 4), (64, 8), (80, 2), (128, 16)]),
            int_prefs: BTreeMap::from([(1, 1), (8, 1), (16, 2), (32, 4), (64, 8)]),
            float_prefs: BTreeMap::from([(16, 2), (32, 4), (64, 8), (80, 2), (128, 16)]),
            legal: Vec::new(),
            aggregate_align: 1,
            aggregate_pref: 8,
            alloca_space: 0,
            stack_align: None,
            layouts: StructLayouts::default(),
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
                from.split(':')
                    .filter(|one| !one.is_empty())
                    .map(|one| one.parse().map_err(|_| format!("`{spec}` in the datalayout")))
                    .collect()
            };
            match head {
                "e" => layout.big_endian = false,
                "E" => layout.big_endian = true,
                "n" => layout.legal = numbers(rest)?.into_iter().map(|bits| bits as u32).collect(),
                "ni" => {
                    for space in numbers(rest)? {
                        if space == 0 {
                            return Err(format!("`{spec}`: address space 0 cannot be non-integral"));
                        }
                        let mut one = layout.pointer(space as u32);
                        one.unstable = true;
                        layout.pointers.insert(space as u32, one);
                    }
                }
                "S" => layout.stack_align = numbers(rest)?.first().map(|&bits| bytes(bits)).filter(|&one| one > 0),
                "m" | "F" | "G" | "P" | "Fi" | "Fn" => {}
                "A" => layout.alloca_space = numbers(rest)?.first().copied().unwrap_or(0) as u32,
                "p" => {
                    let (space, fields) = match rest.split_once(':') {
                        Some((space, fields)) => (space.parse::<u32>().unwrap_or(0), numbers(fields)?),
                        None => return Err(format!("`{spec}` in the datalayout")),
                    };
                    let [size, abi, rest @ ..] = fields.as_slice() else {
                        return Err(format!("`{spec}` in the datalayout"));
                    };
                    let index = rest.get(1).copied().unwrap_or(*size);
                    layout
                        .pointers
                        .insert(
                            space,
                            PointerSpec {
                                bits: *size as u32,
                                align: bytes(*abi),
                                pref: bytes(rest.first().copied().unwrap_or(*abi)),
                                index_bits: index as u32,
                                unstable: false,
                            },
                        );
                }
                "i" | "f" | "a" => {
                    let fields = numbers(rest)?;
                    match (head, fields.as_slice()) {
                        ("i", [size, abi, rest @ ..]) => {
                            layout.ints.insert(*size as u32, bytes(*abi));
                            layout.int_prefs.insert(*size as u32, bytes(rest.first().copied().unwrap_or(*abi)));
                        }
                        ("f", [size, abi, rest @ ..]) => {
                            layout.floats.insert(*size as u32, bytes(*abi));
                            layout.float_prefs.insert(*size as u32, bytes(rest.first().copied().unwrap_or(*abi)));
                        }
                        ("a", [_, abi, rest @ ..]) | ("a", [abi, rest @ ..]) if rest.len() <= 1 => {
                            layout.aggregate_align = bytes(*abi).max(1);
                            layout.aggregate_pref = bytes(rest.first().copied().unwrap_or(*abi)).max(1);
                        }
                        _ => return Err(format!("`{spec}` in the datalayout")),
                    }
                }
                "v" => {}
                _ => return Err(format!("`{spec}` in the datalayout")),
            }
        }
        Ok(layout)
    }

    /// The bytes between successive elements of an array of `kind`: its 80 bits
    /// (10 bytes) rounded up to the layout's alignment for the x87 type, so 12
    /// under `f80:32` and 10 under `f80:16`.
    pub fn float_alloc_size(
        &self,
        kind: FloatKind,
    ) -> u64 {
        bytes(u64::from(float_bits(kind))).next_multiple_of(self.float_align(kind))
    }

    /// A float's ABI alignment, in bytes.
    pub fn float_align(
        &self,
        kind: FloatKind,
    ) -> u64 {
        self.floats[&float_bits(kind)]
    }

    /// Whether the target computes `bits`-wide integers natively: LLVM's
    /// `isLegalInteger`, so none without an `n`.
    pub fn legal_integer(
        &self,
        bits: u32,
    ) -> bool {
        self.legal.contains(&bits)
    }

    /// `DataLayout::fitsInLegalInteger`
    /// ([DataLayout.h](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/IR/DataLayout.h)):
    /// whether some native integer is at least `bits` wide.
    pub fn fits_in_legal_integer(
        &self,
        bits: u32,
    ) -> bool {
        self.legal.iter().any(|&legal| bits <= legal)
    }

    /// The widest native integer, in bits: LLVM's
    /// `getLargestLegalIntTypeSizeInBits`, 0 where none is stated.
    pub fn largest_legal_integer(&self) -> u32 {
        self.legal.iter().copied().max().unwrap_or(0)
    }

    pub fn pointer(
        &self,
        space: u32,
    ) -> PointerSpec {
        self.pointers.get(&space).or_else(|| self.pointers.get(&0)).copied().expect("space 0 always has a pointer")
    }

    /// `DataLayout::isNonIntegralAddressSpace`
    /// ([DataLayout.h:412](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/IR/DataLayout.h#L412)).
    /// DEVIATE: no `HasExternalState` (the `pe` flag is not parsed).
    pub fn is_non_integral_address_space(
        &self,
        space: u32,
    ) -> bool {
        let spec = self.pointer(space);
        spec.bits != spec.index_bits || spec.unstable
    }

    /// Whether `ni:` names `space`: its pointers are not addresses (a selector
    /// alone), as the layout says it, where `is_non_integral_address_space`
    /// also holds a far pointer (its bits exceed its index).
    pub fn has_unstable_representation(
        &self,
        space: u32,
    ) -> bool {
        self.pointer(space).unstable
    }

    /// `DataLayout::mustNotIntroduceIntToPtr`
    /// ([DataLayout.h:461](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/IR/DataLayout.h#L461)).
    pub fn must_not_introduce_int_to_ptr(
        &self,
        space: u32,
    ) -> bool {
        self.pointer(space).unstable
    }

    /// Whether a pointer in `space` is a selector and an offset: wider than
    /// the space-0 pointer, which is the offset alone.
    pub fn is_pair(
        &self,
        space: u32,
    ) -> bool {
        self.pointer(space).bits > self.pointer(0).bits
    }

    /// The bits of `space`'s offset arithmetic: a pair's entry is a selector
    /// word then an offset word, a flat space's offset is its whole
    /// pointer. The one place the split is stated; an address wraps modulo
    /// 2^this.
    pub fn offset_bits(
        &self,
        space: u32,
    ) -> u32 {
        let bits = self.pointer(space).bits;
        if self.is_pair(space) { bits / 2 } else { bits }
    }

    /// Whether a displacement in `space` carries into the selector: a pair
    /// whose index is wider than its offset, as a huge pointer's.
    pub fn carries(
        &self,
        space: u32,
    ) -> bool {
        self.is_pair(space) && self.pointer(space).index_bits > self.offset_bits(space)
    }

    /// An integer's ABI alignment: its own entry, else the next wider one's,
    /// else the widest's, as LLVM chooses.
    fn int_align(
        &self,
        bits: u32,
    ) -> u64 {
        self.ints.range(bits..).next().or_else(|| self.ints.iter().next_back()).map_or(1, |(_, align)| *align)
    }

    pub fn align(
        &self,
        types: &Types,
        ty: TypeId,
    ) -> u64 {
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

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/DataLayout.cpp#L871
    /// `getPrefTypeAlign`: `getAlignment(Ty, abi_or_pref = false)`, in bytes.
    pub fn pref_align(
        &self,
        types: &Types,
        ty: TypeId,
    ) -> u64 {
        let wide = |specs: &BTreeMap<u32, u64>, bits: u32| {
            specs.range(bits..).next().or_else(|| specs.iter().next_back()).map_or(1, |(_, align)| *align)
        };
        match types.get(ty) {
            Type::Int(bits) => wide(&self.int_prefs, *bits),
            Type::Float(kind) => self.float_prefs[&float_bits(*kind)],
            Type::Pointer(space) => self.pointer(*space).pref,
            Type::Array { element, .. } => self.pref_align(types, *element),
            Type::Vector { .. } => self.store_size(types, ty).next_power_of_two(),
            Type::Struct { .. } | Type::Named(_) => {
                // The layout's alignment is the largest ABI one of the
                // fields, and one for a packed struct.
                let layout = self.align_of_struct_layout(types, ty);
                layout.max(self.aggregate_pref)
            }
            Type::Void | Type::Label | Type::Metadata | Type::Token | Type::Function { .. } => 1,
        }
    }

    /// The alignment a global variable is emitted at:
    /// `DataLayout::getPreferredAlign(GV)`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/IR/DataLayout.cpp#L1003-L1038>)
    /// and `AsmPrinter::getGVAlignment`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/AsmPrinter/AsmPrinter.cpp#L380-L405>):
    /// the type's preferred alignment, raised to the stated one, and 16 for an
    /// initialized global over 128 bits that states none (`large_to_16`; a
    /// target of 64K segments does not make it, `driver/data.rs` DEVIATE E2).
    /// A stated alignment of a global in a section is kept exactly.
    pub fn global_align(
        &self,
        types: &Types,
        ty: TypeId,
        stated: Option<u64>,
        in_section: bool,
        initialized: bool,
        large_to_16: bool,
    ) -> u64 {
        let mut align = self.pref_align(types, ty);
        match stated {
            Some(stated) if in_section => return stated,
            Some(stated) if stated >= align => align = stated,
            Some(stated) => align = stated.max(self.align(types, ty)),
            None if large_to_16 && initialized && align < 16 && self.size_bits(types, ty) > 128 => align = 16,
            None => {}
        }
        match stated {
            Some(stated) if stated > align => stated,
            _ => align,
        }
    }

    /// `StructLayout::getAlignment`: the largest ABI alignment of the fields
    /// (one for a packed struct), at least the aggregate ABI alignment.
    fn align_of_struct_layout(
        &self,
        types: &Types,
        ty: TypeId,
    ) -> u64 {
        let packed = match types.get(ty) {
            Type::Struct { packed, .. } => *packed,
            Type::Named(name) => types.body(name).is_some_and(|body| body.packed),
            _ => false,
        };
        let fields = types.fields(ty).unwrap_or_default();
        let largest = if packed { 1 } else { fields.iter().map(|&one| self.align(types, one)).max().unwrap_or(1) };
        largest.max(self.aggregate_align)
    }

    /// `Type::getPrimitiveSizeInBits`: 0 for a pointer, an aggregate, or a
    /// vector of them.
    fn primitive_bits(
        &self,
        types: &Types,
        ty: TypeId,
    ) -> u64 {
        match types.get(ty) {
            Type::Int(bits) => u64::from(*bits),
            Type::Float(kind) => u64::from(float_bits(*kind)),
            Type::Vector { element, count } => self.primitive_bits(types, *element) * u64::from(*count),
            _ => 0,
        }
    }

    /// `CastInst::isBitCastable`
    /// ([Instructions.cpp:3223](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Instructions.cpp#L3223)).
    pub fn is_bit_castable(
        &self,
        types: &Types,
        src: TypeId,
        dest: TypeId,
    ) -> bool {
        let first_class = |ty| {
            !matches!(
                types.get(ty),
                Type::Void | Type::Function { .. } | Type::Label | Type::Metadata | Type::Token
            )
        };
        if !first_class(src) || !first_class(dest) {
            return false;
        }
        if src == dest {
            return true;
        }
        // An element by element cast of two vectors of one count.
        let (src, dest) = match (types.get(src), types.get(dest)) {
            (Type::Vector { element: a, count: m }, Type::Vector { element: b, count: n }) if m == n => (*a, *b),
            _ => (src, dest),
        };
        if let (Type::Pointer(a), Type::Pointer(b)) = (types.get(src), types.get(dest)) {
            return a == b;
        }
        let (src_bits, dest_bits) = (self.primitive_bits(types, src), self.primitive_bits(types, dest));
        src_bits != 0 && src_bits == dest_bits
    }

    /// `CastInst::isBitOrNoopPointerCastable`
    /// ([Instructions.cpp:3260](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Instructions.cpp#L3260)):
    /// a bitcast, or a `ptrtoint` or `inttoptr` between a pointer and an
    /// integer as wide as it that is not of a non-integral space.
    pub fn is_bit_or_noop_pointer_castable(
        &self,
        types: &Types,
        src: TypeId,
        dest: TypeId,
    ) -> bool {
        let integral_pointer_as_wide_as =
            |pointer: u32, int: u32| int == self.pointer(pointer).bits && !self.is_non_integral_address_space(pointer);
        match (types.get(src), types.get(dest)) {
            (Type::Pointer(space), Type::Int(bits)) | (Type::Int(bits), Type::Pointer(space)) => {
                integral_pointer_as_wide_as(*space, *bits)
            }
            _ => self.is_bit_castable(types, src, dest),
        }
    }

    /// The bits a value of `ty` holds.
    pub fn size_bits(
        &self,
        types: &Types,
        ty: TypeId,
    ) -> u64 {
        match types.get(ty) {
            Type::Int(bits) => u64::from(*bits),
            Type::Float(kind) => u64::from(float_bits(*kind)),
            Type::Pointer(space) => u64::from(self.pointer(*space).bits),
            Type::Vector { element, count } => self.size_bits(types, *element) * u64::from(*count),
            _ => self.store_size(types, ty) * 8,
        }
    }

    /// The bytes a store of `ty` writes.
    pub fn store_size(
        &self,
        types: &Types,
        ty: TypeId,
    ) -> u64 {
        match types.get(ty) {
            Type::Array { element, count } => self.alloc_size(types, *element) * count,
            Type::Struct { .. } | Type::Named(_) => self.shared_struct_layout(types, ty).0,
            _ => bytes(self.size_bits(types, ty)),
        }
    }

    /// The bytes between successive elements of an array of `ty`.
    pub fn alloc_size(
        &self,
        types: &Types,
        ty: TypeId,
    ) -> u64 {
        self.store_size(types, ty).next_multiple_of(self.align(types, ty))
    }

    /// A struct's size and each field's offset.
    /// What a GEP's indices add to its pointer, as LLVM's `collectOffset`:
    /// a constant, and each variable index's scale by its position. A
    /// variable index is `None`; a struct's index never is.
    pub fn collect_offset(
        &self,
        types: &Types,
        source: TypeId,
        indices: &[Option<i128>],
    ) -> (i128, Vec<(usize, u64)>) {
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
                    let layout = self.shared_struct_layout(types, current);
                    constant += layout.1[field as usize] as i128;
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

    /// A struct's size and each field's offset.
    pub fn struct_layout(
        &self,
        types: &Types,
        ty: TypeId,
    ) -> (u64, Vec<u64>) {
        let layout = self.shared_struct_layout(types, ty);
        (layout.0, layout.1.clone())
    }

    /// `struct_layout`, held: `getStructLayout`.
    pub fn shared_struct_layout(
        &self,
        types: &Types,
        ty: TypeId,
    ) -> std::sync::Arc<(u64, Vec<u64>)> {
        let version = (types as *const Types as usize, types.version().0, types.version().1);
        {
            let mut held = self.layouts.0.lock().expect("the layout memo");
            if (held.0, held.1, held.2) != version {
                *held = (version.0, version.1, version.2, Default::default());
            } else if let Some(found) = held.3.get(&ty) {
                return std::sync::Arc::clone(found);
            }
        }
        COMPUTED.with(|count| count.set(count.get() + 1));
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
        let made = std::sync::Arc::new((offset.next_multiple_of(self.align(types, ty)), offsets));
        if types.fields(ty).is_some() {
            self.layouts.0.lock().expect("the layout memo").3.insert(ty, std::sync::Arc::clone(&made));
        }
        made
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

    /// `offset_bits` was a pair's half for every space: a flat 16-bit space
    /// said 8, so a caller that wrapped a flat offset by it (a frame index
    /// into a 32 KB object) wrapped at 256.
    #[test]
    fn a_flat_spaces_offset_bits_are_its_pointer_bits_and_a_pairs_are_half() {
        let layout = DataLayout::parse("e-p:16:16-p1:32:16:16:16-p3:32:16:16:32").unwrap();
        assert_eq!((layout.offset_bits(0), layout.offset_bits(1)), (16, 16));
        assert_eq!(layout.offset_bits(3), 16);
    }

    /// `isBitOrNoopPointerCastable`: a bitcast of one size, a pointer and an
    /// integer as wide as it only in an integral space (`p1` here is a far
    /// pointer of 32 bits with a 16-bit index: not integral), and two pointers
    /// of one space.
    #[test]
    fn a_pointer_is_castable_to_an_integer_as_wide_as_it_in_an_integral_space_only() {
        use crate::types::{FloatKind, Type};
        let module = crate::parse::module("").unwrap();
        let types = &module.context.types;
        let layout = DataLayout::parse("e-p:16:16-p1:32:16:16:16").unwrap();
        let (i16_, i32_, float) = (types.int(16), types.int(32), types.intern(Type::Float(FloatKind::Float)));
        let (near, far, near2) = (types.ptr(0), types.ptr(1), types.ptr(0));
        let castable = |a, b| layout.is_bit_or_noop_pointer_castable(types, a, b);
        assert!(castable(near, i16_) && castable(i16_, near), "a near pointer and an i16");
        assert!(!castable(far, i32_) && !castable(i32_, far), "a far pointer is not integral");
        assert!(castable(i32_, float) && !castable(i16_, float), "a bitcast is of one size");
        assert!(castable(near, near2) && !castable(near, far), "pointers of one space");
    }

    /// `getPrefTypeAlign`: LLVM's defaults prefer an aggregate 8-byte
    /// aligned (`a:0:64`) and an `i64` 8 (`i64:32:64`), over their ABI 1 and
    /// 4 here; a stated preferred alignment wins. `pointer_alignment` took the
    /// ABI one for a global and lost the low 3 bits of its address.
    #[test]
    fn the_preferred_alignment_of_an_aggregate_is_8_unless_the_layout_says_otherwise() {
        let mut module = crate::parse::module(
            "@g = global <{ [2 x i8], i16 }> zeroinitializer\n@h = global i64 0\n@p = global ptr null\n",
        )
        .unwrap();
        let types = &module.context.types;
        let of = |name: &str| {
            let variable = module.globals.iter().find(|one| one.name.as_deref() == Some(name)).unwrap();
            let crate::module::GlobalKind::Variable(variable) = &variable.kind else { unreachable!("a variable") };
            variable.ty
        };
        let layout = DataLayout::default();
        assert_eq!((layout.align(types, of("g")), layout.pref_align(types, of("g"))), (1, 8));
        assert_eq!((layout.align(types, of("h")), layout.pref_align(types, of("h"))), (4, 8));
        assert_eq!(layout.pref_align(types, of("p")), layout.pointer(0).align);
        let stated = DataLayout::parse("a:0:32-i64:32:128-p:64:64:128").unwrap();
        assert_eq!(
            (stated.pref_align(types, of("g")), stated.pref_align(types, of("h")), stated.pref_align(types, of("p"))),
            (4, 16, 16)
        );
        module.globals.clear();
    }

    /// Globals with no stated alignment were emitted byte aligned (a dword a
    /// loop reads misaligned across two cache lines on a 486). `getGVAlignment`
    /// gives the type's preferred alignment, 16 over 128 bits, and keeps a
    /// stated alignment of a global in a section.
    #[test]
    fn a_global_is_aligned_as_the_preferred_alignment_of_its_type() {
        let module = crate::parse::module(
            "@w = global i32 0\n@b = global i8 0\n@big = global [40 x i8] zeroinitializer\n@small = global [4 x i8] zeroinitializer\n",
        )
        .unwrap();
        let types = &module.context.types;
        let of = |name: &str| {
            let variable = module.globals.iter().find(|one| one.name.as_deref() == Some(name)).unwrap();
            let crate::module::GlobalKind::Variable(variable) = &variable.kind else { unreachable!("a variable") };
            variable.ty
        };
        let layout = DataLayout::parse("i32:32-i8:8").unwrap();
        let align = |name, stated, section, initialized| {
            layout.global_align(types, of(name), stated, section, initialized, true)
        };
        assert_eq!(align("w", None, false, true), 4);
        assert_eq!(align("b", None, false, true), 1);
        assert_eq!(align("big", None, false, true), 16, "over 128 bits and initialized");
        assert_eq!(align("big", None, false, false), 1, "a declaration is not raised");
        assert_eq!(align("small", Some(8), false, true), 8, "a stated alignment is kept");
        assert_eq!(align("w", Some(1), false, true), 4, "raised to the type's ABI alignment");
        assert_eq!(align("w", Some(1), true, true), 1, "kept exactly in a section");
        assert_eq!(
            layout.global_align(types, of("big"), None, false, true, false),
            1,
            "a target of 64K segments does not raise a large global"
        );
    }

    /// The m16 datalayout names `f80:16` since the fork's clang does; the
    /// string without it meant the same (a long double of 10 bytes, 2-aligned),
    /// so no object of the Watcom front end changes.
    #[test]
    fn f80_16_states_what_a_layout_without_f80_already_meant() {
        let new = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32";
        let old = new.replace("-f80:16", "");
        let old = old.as_str();
        let (old, new) = (DataLayout::parse(old).unwrap(), DataLayout::parse(new).unwrap());
        use crate::types::FloatKind;
        for kind in [FloatKind::Float, FloatKind::Double, FloatKind::X86Fp80] {
            assert_eq!(
                (old.float_align(kind), old.float_alloc_size(kind)),
                (new.float_align(kind), new.float_alloc_size(kind))
            );
        }
        assert_eq!(new.float_alloc_size(FloatKind::X86Fp80), 10);
    }

    /// A struct's layout was worked out at every size ask: `object_of` asked
    /// the same two nested structs 434 times in one LICM run, 3 MInstr of 23
    /// (QCport d_alias, m16 -Os). It is made once for each struct, as LLVM's
    /// `getStructLayout` map does.
    #[test]
    fn a_structs_layout_is_made_once_however_often_its_size_is_asked() {
        let module =
            crate::parse::module("%inner = type { i16, i32, [4 x i8] }\n%outer = type { %inner, i16, %inner }\n")
                .unwrap();
        let types = &module.context.types;
        let layout = DataLayout::parse("e-p:16:16-i32:32").unwrap();
        let outer = types.intern(crate::types::Type::Named("outer".into()));
        let before = super::layouts_computed();
        for _ in 0..100 {
            assert_eq!(layout.alloc_size(types, outer), 28);
        }
        assert_eq!(super::layouts_computed() - before, 2, "outer and inner, once each");
    }
}
