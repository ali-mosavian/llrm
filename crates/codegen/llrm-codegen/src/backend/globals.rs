//! Turns a MIR module's globals into assembler symbols and data: names, and
//! each initializer's bytes with a relocation for every address in it.
//!
//! # How it works
//! `names` maps every global to its assembler symbol, keyed as an `Addr` names
//! it (space, index). The name is what the ABI's `linked` makes of it, as
//! LLVM's Mangler does. An internal global whose name no assembler accepts
//! becomes `G$n`; an external one must already be a symbol, else it is an
//! error. An intrinsic has no symbol. A global outside the near address space
//! also gets a segment name, `seg <symbol>`, which a far pointer to it uses.
//!
//! `datums` emits a defined variable as a label followed by its initializer,
//! walked as LLVM's AsmPrinter does: integers and floats as little-endian bytes
//! (x87 extended as its ten-byte format), zero and null as zero bytes, structs
//! with their padding, arrays member by member. Adjacent bytes merge into one
//! datum. An address constant becomes a relocation:
//!
//! ```text
//!  initializer constant              emitted
//!  --------------------------------  ----------------------------------
//!  ptr to global (+ GEP offset)      Pointer { name, offset }, near
//!  far pointer to a far global       Pointer { far: true }: offset+segment
//!  far pointer to near data          offset word, then DGROUP's selector
//!  addrspacecast to the segment      SegmentWord(symbol)
//!  ptrtoint of an address, i16/i32   Pointer { bytes: 2 or 4 }
//! ```
//!
//! A far pointer to near data is built from DGROUP's selector and the
//! group-relative offset, as a near pointer made far is: one POINTER fixup
//! would take the target segment's own selector. `target` resolves an address
//! constant to its global and byte offset through constant GEPs; `absolute`
//! resolves a constant that names no global (null, an integer made a pointer).
//!
//! # Example
//! A C module on `-m16` with an array, a near pointer into it and a long.
//!
//! MIR:
//!
//! ```mir
//! @_table = global [6 x i8] c"\01\00\02\00\03\00"
//! @_second = global ptr getelementptr (i8, ptr @_table, i16 2)
//! @_big = global [4 x i8] c"p\11\01\00"
//! ```
//!
//! Data the masm listing prints for it. The pointer is a `dw` relocation
//! against `_table`, plus the constant offset:
//!
//! ```asm
//! _table label byte
//! db 001h,000h,002h,000h,003h,000h
//! _second label byte
//!     dw _table+2
//! _big label byte
//! db 070h,011h,001h,000h
//! ```
//!
//! From `-S` and `LLRM_MIR_STAGES` of that module at -O2.
//!
//! # Runs
//! `global names` and the data loop in `assemble::assembled_by`, once per
//! module after `nearcode`. Isel uses `space`, `target` and `absolute` for
//! address constants.
//!
//! # Used by
//! `assemble.rs`, `isel.rs`.
//!
//! # See also
//! LLVM `AsmPrinter::emitGlobalVariable`.

use llrm_mir::datalayout::DataLayout;
use llrm_mir::{CastOp, ConstantExpr, ConstantId, ConstantKind, FloatKind, GlobalId, GlobalKind, Module, Type, TypeId};

use crate::backend::masm::{Datum, Label, Pointer};
use crate::model::ir::Space;
use crate::support::hash::IndexMap;

/// Where a global's symbol is: defined here, or outside the module.
pub fn space(
    module: &Module,
    global: GlobalId,
) -> Space {
    match &module.global(global).kind {
        GlobalKind::Variable(variable) if variable.initializer.is_some() => Space::Segment,
        GlobalKind::Function(function) if !function.is_declaration() => Space::Segment,
        _ => Space::External,
    }
}

/// Each global's assembler name, keyed as an `Addr` names it: its name as
/// `linked` mangles it, as LLVM's Mangler does. An internal global's name
/// is free, so one no assembler takes becomes `G$n`, as does an external one
/// defined here (a frontend names it for the object); an imported one must be
/// a symbol once mangled. An intrinsic has none.
/// The group index naming a far variable's segment, `seg name`.
pub fn segment_of(global: GlobalId) -> i64 {
    SEGMENTS + i64::from(global.0)
}

/// Where `segment_of` numbers start: above DGROUP's.
const SEGMENTS: i64 = 1 << 22;

pub fn names(
    module: &Module,
    spaces: &llrm_mir::spaces::Spaces,
    linked: &dyn Fn(&str) -> String,
) -> Result<IndexMap<(Space, i64), String>, String> {
    let mut out = IndexMap::default();
    for (at, global) in module.globals.iter().enumerate() {
        let id = GlobalId(at as u32);
        // An intrinsic is lowered where it is called, never linked.
        if global.name.as_deref().is_some_and(|name| {
            (llrm_mir::intrinsics::is_reserved(name) && !is_list(name)) || llrm_mir::atomic::decode(name).is_some()
        }) {
            continue;
        }
        let name = linked(global.name.as_deref().unwrap_or_default());
        let symbol = if is_symbol(&name) {
            name
        } else if global.linkage == llrm_mir::Linkage::External && space(module, id) == Space::External {
            return Err(format!("@{name} is no assembler symbol"));
        } else {
            format!("G${at}")
        };
        out.extend(segment_name(module, id, &symbol, spaces));
        out.insert((space(module, id), i64::from(id.0)), symbol);
    }
    Ok(out)
}

/// A far global's segment, named for `symbol`: a far pointer to code, as
/// ON ERROR registers its handler, takes it.
pub fn segment_name(
    module: &Module,
    id: GlobalId,
    symbol: &str,
    spaces: &llrm_mir::spaces::Spaces,
) -> Option<((Space, i64), String)> {
    (module.global(id).address_space != spaces.near).then(|| ((Space::Group, segment_of(id)), format!("seg {symbol}")))
}

/// The constructor list: laid out as the loader's init-function pointers.
pub const CONSTRUCTORS: &str = "llvm.global_ctors";

/// The segment the constructor list is laid out in.
pub const INIT_SEGMENT: &str = ".init_array";

/// `llvm.used` and `llvm.compiler.used` are for the compiler and the linker
/// only; LLVM lays neither out.
pub fn is_compiler_only(name: &str) -> bool {
    matches!(name, "llvm.used" | "llvm.compiler.used")
}

fn is_list(name: &str) -> bool {
    name == CONSTRUCTORS
}

fn is_symbol(name: &str) -> bool {
    let mut characters = name.chars();
    characters.next().is_some_and(|first| first.is_ascii_alphabetic() || "_$@?".contains(first))
        && characters.all(|one| one.is_ascii_alphanumeric() || "_$@?".contains(one))
}

/// The group near data is in.
pub const DGROUP: &str = "DGROUP";

/// A far pointer to `name`, `offset` into it. One to near data is DGROUP's
/// selector and the group-relative offset, as a near pointer made far is:
/// a single POINTER fixup would take the target segment's own selector,
/// which code pairing it with a near offset (a BASIC array descriptor's
/// AD_fhd and AD_oAdjusted) reads the wrong cells through.
pub fn far_pointer(
    name: String,
    offset: i64,
    near: bool,
    layout: &DataLayout,
    space: u32,
) -> Vec<Datum> {
    let bytes = layout.pointer(space).bits / 8;
    if near {
        vec![
            Datum::Pointer(Pointer { name, offset, far: false, bytes: layout.offset_bits(space) / 8 }),
            Datum::SegmentWord(DGROUP.to_owned()),
        ]
    } else {
        vec![Datum::Pointer(Pointer { name, offset, far: true, bytes })]
    }
}

/// The alignment a variable is emitted at: `AsmPrinter::getGVAlignment` on the
/// module's data layout, none where that is a byte. DEVIATE E1: MIR has no
/// section attribute; a global the frontend placed in a named segment counts
/// as one in a section (`in_section`). DEVIATE E2: where data lives in 64K
/// segments (`segmented`), an initialized global over 16 bytes is not raised to
/// 16 (`DataLayout::getPreferredAlign`, DataLayout.cpp#L1175): the object's
/// segment would become paragraph aligned, up to 15 dead bytes of the scarce
/// DGROUP per object (QB runtime `input.c`: +32 B, `dgroup_nearly_full`).
pub fn alignment(
    module: &Module,
    layout: Option<&llrm_mir::datalayout::DataLayout>,
    variable: &llrm_mir::module::GlobalVariable,
    in_section: bool,
    segmented: bool,
) -> Option<i64> {
    // A module with no data layout states its own alignments only.
    let Some(layout) = layout else { return variable.align.map(|to| to as i64) };
    let to = layout.global_align(
        &module.context.types,
        variable.ty,
        variable.align,
        in_section,
        variable.initializer.is_some(),
        !segmented,
    );
    (to > 1).then_some(to as i64)
}

/// A defined variable's data: its label, then its initializer's bytes and
/// relocations.
pub fn datums(
    module: &Module,
    global: GlobalId,
    names: &IndexMap<(Space, i64), String>,
) -> Result<Vec<Datum>, String> {
    let GlobalKind::Variable(variable) = &module.global(global).kind else {
        return Err("a function has no data".to_owned());
    };
    let initializer = variable.initializer.ok_or("a declaration has no data")?;
    let layout = DataLayout::parse(module.datalayout.as_deref().ok_or("a module with no datalayout")?)?;
    let name = names[&(Space::Segment, i64::from(global.0))].clone();
    let mut out = vec![Datum::Label(Label { name })];
    if module.global(global).name.as_deref() == Some(CONSTRUCTORS) {
        constructors(module, &layout, initializer, names, &mut out)?;
        return Ok(out);
    }
    Initializer { module, layout: &layout, names, out: &mut out }.constant(initializer)?;
    Ok(out)
}

/// The function pointers of `llvm.global_ctors`' `{ i32 priority, ptr function,
/// ptr data }` entries, lowest priority first and in order within one, as
/// `AsmPrinter::emitXXStructorList` sorts them; an entry without a function is
/// dropped.
fn constructors(
    module: &Module,
    layout: &DataLayout,
    list: ConstantId,
    names: &IndexMap<(Space, i64), String>,
    out: &mut Vec<Datum>,
) -> Result<(), String> {
    let context = &module.context;
    let bytes = layout.pointer(0).bits / 8;
    if bytes < 4 {
        return Err("llvm.global_ctors: the 16-bit DOS start-up runs no constructor table".to_owned());
    }
    out.insert(0, Datum::Align(crate::backend::masm::Align { to: i64::from(bytes) }));
    let entries = match &context.get(list).kind {
        ConstantKind::Aggregate(entries) => entries.clone(),
        _ => Vec::new(),
    };
    let mut found = Vec::new();
    for entry in entries {
        let ConstantKind::Aggregate(fields) = &context.get(entry).kind else { continue };
        let [priority, function, ..] = fields[..] else { return Err("a malformed llvm.global_ctors entry".to_owned()) };
        let ConstantKind::Int(priority) = context.get(priority).kind else {
            return Err("a malformed llvm.global_ctors priority".to_owned());
        };
        if absolute(module, layout, function) == Some(0) {
            continue;
        }
        let (global, offset) = target(module, layout, function)?;
        found.push((priority, global, offset));
    }
    found.sort_by_key(|(priority, ..)| *priority);
    for (_, global, offset) in found {
        let name = names[&(space(module, global), i64::from(global.0))].clone();
        out.push(Datum::Pointer(Pointer { name, offset, far: false, bytes }));
    }
    Ok(())
}

struct Initializer<'a> {
    module: &'a Module,
    layout: &'a DataLayout,
    names: &'a IndexMap<(Space, i64), String>,
    out: &'a mut Vec<Datum>,
}

impl Initializer<'_> {
    fn bytes(
        &mut self,
        bytes: &[u8],
    ) {
        if bytes.is_empty() {
            return;
        }
        if let Some(Datum::Bytes(last)) = self.out.last_mut() {
            last.extend_from_slice(bytes);
        } else {
            self.out.push(Datum::Bytes(bytes.to_vec()));
        }
    }

    fn symbol(
        &self,
        global: GlobalId,
    ) -> String {
        self.names[&(space(self.module, global), i64::from(global.0))].clone()
    }

    fn constant(
        &mut self,
        id: ConstantId,
    ) -> Result<(), String> {
        let context = &self.module.context;
        let types = &context.types;
        let constant = context.get(id);
        let size = self.layout.store_size(types, constant.ty) as usize;
        match &constant.kind {
            ConstantKind::Int(bits) => self.bytes(&bits.to_le_bytes()[..size]),
            ConstantKind::Float(bits) => match types.get(constant.ty) {
                Type::Float(FloatKind::Float) => self.bytes(&(*bits as u32).to_le_bytes()),
                Type::Float(FloatKind::X86Fp80) => self.bytes(&llrm_mir::types::x87_extended(*bits)),
                _ => self.bytes(&bits.to_le_bytes()[..size]),
            },
            ConstantKind::Float80(bits) => self.bytes(&llrm_mir::types::x87_image(*bits)),
            // An undef initializer is any bytes: zero. A poison one is a hole
            // of a switch lookup table (the pin's holes are `poison` too),
            // which the guard never indexes. `emitGlobalConstantImpl` writes
            // zeros for both:
            // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/AsmPrinter/AsmPrinter.cpp#L4560
            ConstantKind::Null | ConstantKind::Zero | ConstantKind::Undef | ConstantKind::Poison => {
                self.bytes(&vec![0; self.layout.alloc_size(types, constant.ty) as usize])
            }
            ConstantKind::Bytes(bytes) => self.bytes(bytes),
            ConstantKind::Aggregate(members) => match types.get(constant.ty).clone() {
                Type::Struct { .. } | Type::Named(_) => {
                    let (total, offsets) = self.layout.struct_layout(types, constant.ty);
                    let mut at = 0;
                    for (member, offset) in members.clone().into_iter().zip(offsets) {
                        self.bytes(&vec![0; (offset - at) as usize]);
                        self.constant(member)?;
                        at = offset
                            + self.layout.store_size(&self.module.context.types, self.module.context.get(member).ty);
                    }
                    self.bytes(&vec![0; (total - at) as usize]);
                }
                _ => {
                    // An element takes its alloc size: an `x86_fp80` is 10
                    // bytes stored and 12 apart at f80:32.
                    for member in members.clone() {
                        self.constant(member)?;
                        let types = &self.module.context.types;
                        let ty = self.module.context.get(member).ty;
                        let padding = self.layout.alloc_size(types, ty) - self.layout.store_size(types, ty);
                        self.bytes(&vec![0; padding as usize]);
                    }
                }
            },
            ConstantKind::Global(_) | ConstantKind::Expr(_) => self.address(id)?,
        }
        Ok(())
    }

    /// A relocated address: a near or far pointer, a segment, or a far
    /// pointer's offset word.
    fn address(
        &mut self,
        id: ConstantId,
    ) -> Result<(), String> {
        let context = &self.module.context;
        let constant = context.get(id);
        let datum = match (&constant.kind, context.types.get(constant.ty)) {
            // A pointer made of an integer (`(Video far *)0xA0000000`, a video
            // segment): its image, no relocation.
            (_, Type::Pointer(_)) if absolute(self.module, self.layout, id).is_some() => {
                let at = absolute(self.module, self.layout, id).unwrap_or(0);
                let size = self.layout.store_size(&context.types, constant.ty) as usize;
                self.bytes(&at.to_le_bytes()[..size]);
                return Ok(());
            }
            (_, Type::Pointer(0)) => {
                let (global, offset) = target(self.module, self.layout, id)?;
                Datum::Pointer(Pointer {
                    name: self.symbol(global),
                    offset,
                    far: false,
                    bytes: self.layout.pointer(0).bits / 8,
                })
            }
            (_, Type::Pointer(space)) if self.layout.is_pair(*space) => {
                let (global, offset) = target(self.module, self.layout, id)?;
                // A global outside the near space has a segment of its own.
                let near = !self.names.contains_key(&(Space::Group, segment_of(global)))
                    && matches!(self.module.global(global).kind, GlobalKind::Variable(_));
                self.out.extend(far_pointer(self.symbol(global), offset, near, self.layout, *space));
                return Ok(());
            }
            (ConstantKind::Expr(ConstantExpr::Cast { op: CastOp::AddrSpaceCast, value }), Type::Pointer(2)) => {
                let (global, _) = target(self.module, self.layout, *value)?;
                Datum::SegmentWord(self.symbol(global))
            }
            (ConstantKind::Expr(ConstantExpr::Cast { op: CastOp::PtrToInt, value }), Type::Int(bits @ (16 | 32))) => {
                let (global, offset) = target(self.module, self.layout, *value)?;
                Datum::Pointer(Pointer { name: self.symbol(global), offset, far: false, bytes: bits / 8 })
            }
            _ => return Err(format!("an initializer of {}", context.types.display(constant.ty))),
        };
        self.out.push(datum);
        Ok(())
    }
}

/// The global an address constant points into, and how far.
pub fn target(
    module: &Module,
    layout: &DataLayout,
    id: ConstantId,
) -> Result<(GlobalId, i64), String> {
    let context = &module.context;
    match &context.get(id).kind {
        ConstantKind::Global(global) => Ok((*global, 0)),
        ConstantKind::Expr(ConstantExpr::GetElementPtr { source, operands, .. }) => {
            let (global, base) = target(module, layout, operands[0])?;
            Ok((global, base + constant_offset(module, layout, *source, &operands[1..])?))
        }
        ConstantKind::Expr(ConstantExpr::Cast { op: CastOp::AddrSpaceCast, value }) => target(module, layout, *value),
        _ => Err("an address of no global".to_owned()),
    }
}

/// The address a constant is, where it names no global: null, an integer
/// made a pointer, and a constant offset from either.
pub fn absolute(
    module: &Module,
    layout: &DataLayout,
    id: ConstantId,
) -> Option<i64> {
    let context = &module.context;
    match &context.get(id).kind {
        // Poison is any address: null.
        ConstantKind::Null | ConstantKind::Zero | ConstantKind::Poison | ConstantKind::Undef => Some(0),
        ConstantKind::Expr(ConstantExpr::Cast { op: CastOp::IntToPtr, value }) => match context.get(*value).kind {
            ConstantKind::Int(bits) => {
                Some(llrm_mir::context::signed(bits, context.types.int_bits(context.get(*value).ty)?) as i64)
            }
            _ => None,
        },
        // A far address read through a near pointer is its offset: the low
        // bits, as the cast of a variable is.
        ConstantKind::Expr(ConstantExpr::Cast { op: CastOp::AddrSpaceCast, value }) => {
            let (Type::Pointer(from), Type::Pointer(to)) =
                (context.types.get(context.get(*value).ty), context.types.get(context.get(id).ty))
            else {
                return None;
            };
            let (from_bits, to_bits) = (layout.pointer(*from).bits, layout.pointer(*to).bits);
            if to_bits >= from_bits || layout.has_unstable_representation(*from) {
                return None;
            }
            let bits = absolute(module, layout, *value)? as u64 & ((1u64 << to_bits) - 1);
            Some(llrm_mir::context::signed(u128::from(bits), to_bits) as i64)
        }
        // An offset from a huge pointer's bits is not their sum: the carry past
        // the offset goes into the selector, which the caller that knows the
        // stride adds.
        ConstantKind::Expr(ConstantExpr::GetElementPtr { .. })
            if matches!(
                context.types.get(context.get(id).ty),
                Type::Pointer(space) if layout.has_unstable_representation(*space)
            ) =>
        {
            None
        }
        ConstantKind::Expr(ConstantExpr::GetElementPtr { source, operands, .. }) => Some(
            absolute(module, layout, operands[0])? + constant_offset(module, layout, *source, &operands[1..]).ok()?,
        ),
        _ => None,
    }
}

/// What a constant GEP's `indices` into `source` add.
pub fn constant_offset(
    module: &Module,
    layout: &DataLayout,
    source: TypeId,
    indices: &[ConstantId],
) -> Result<i64, String> {
    let context = &module.context;
    let indices: Vec<Option<i128>> = indices
        .iter()
        .map(|&one| match context.get(one).kind {
            ConstantKind::Int(bits) => {
                let width = context.types.int_bits(context.get(one).ty).unwrap_or(64);
                Some(llrm_mir::context::signed(bits, width))
            }
            _ => None,
        })
        .collect();
    let (offset, variable) = layout.collect_offset(&context.types, source, &indices);
    if !variable.is_empty() {
        return Err("a constant address with no constant offset".to_owned());
    }
    Ok(offset as i64)
}
