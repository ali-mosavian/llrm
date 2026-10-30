//! The assembly-level module: allocated LIR with its frame sequences written
//! out, as the items `objbuild` encodes and the jwasm source `-S` prints.
//!
//! # How it works
//! `assemble` collects each machined function into a [`Procedure`] (its LIR
//! body, the bytes it reserves below the frame register, its callees, flags)
//! and the module's data into [`Datum`]s, giving a [`Module`]. `listing` turns
//! one procedure into [`Item`]s, and `text` prints the module from them. The
//! printer and the encoder read the same items, so the two cannot drift. Every
//! operand is already placed; an unplaced one is an error ([`Unprintable`]).
//!
//! A procedure that makes calls and needs the stack aligned (`calleefacts`, the
//! target's `stack_alignment`) reserves what makes the return address, its
//! pushes, the frame register and the reserve a multiple of it, so a call finds
//! the stack aligned; a call's arguments are padded to it by `isel`.
//!
//! ```text
//!   Procedure.body (LIR)     frame facts          Item list
//!   ----------------------   ------------------   -----------------------
//!   blocks, returns       -> parts(): entry   ->  enter (entry only)
//!   cleaned_returns          and leave            block labels, items
//!   (ret imm / retf n)       sequences            leave before each ret
//!                            shrinkwrap::wrapped  pieces set up in the
//!                                                 the Save block
//! ```
//!
//! The entry and return sequences are not in the LIR. `parts` builds them:
//! `push bp; mov bp,sp; sub sp,reserve` (or `enter` where size is tuned and
//! the target has it), the stack-limit compare and its overflow branches, a
//! `push` per callee-saved register the body names or a call disturbs, and
//! the mirror on return. An interrupt handler wraps its frame in a save of
//! every register and the segment setup, and a runtime-built frame is not
//! rebuilt.
//!
//! Where the frame register is not needed (nothing holds a value in it), the
//! frame is dropped and each cell is addressed through the stack pointer
//! (`stack_addressed`): every instruction's stack effect moves the offset,
//! and every way into a block (a branch, or the block before it falling through
//! where the next block is among its successors) must agree on it. Anything
//! that puts that out of reach (the stack pointer written another way, inline
//! code other than x87 register operations, an indirect call that may pop, the
//! frame register read as a value) keeps the frame. A BASIC procedure's
//! compiler-built frame is like any other; one the runtime builds (`B$ENRA`)
//! is walked by the runtime and kept. Tuned for size, the stack-addressed form
//! is kept only if it is no longer in bytes.
//!
//! Under `-g` the items also carry marks (line starts, body start and end,
//! where a variable's value is defined) which encode to no bytes. A move of an
//! immediate into a segment register, which x86 lacks, is written `push imm;
//! pop seg`.
//!
//! The frame and the saved registers are set up at the Save block rather than
//! the entry when `shrinkwrap::wrapped` says so, and are taken back before the
//! return of the Restore block.
//!
//! `cleaned_returns` rewrites each `ret` of a callee that removes its
//! arguments to `ret n` or `retf n`; past what `ret imm16` can remove, a
//! near return of a 32-bit target pops the return address into the last slot
//! of the area instead (`moved_return`). `return_overhead_bytes` is the size of
//! the leave sequence, which `jumps::duplicated_returns` needs.
//!
//! # Example
//! The allocated LIR has no frame sequences. `listing` adds the `push` and
//! `pop` of the callee-saved `si` the body uses, a `ret` after each leave, and
//! the labels.
//!
//! Before:
//! ```lir
//! === ControlFlow
//!   total: 25 instructions
//!     block 0x0000
//!       0x0000     ; inserted; delivers v19@AX
//!       0x0000  SI := mov AX  ; inserted
//!       ; ...
//!       0x0003  cmp DX, 0x4  ; inserted
//!       0x0003  jne   ; inserted
//!       0x0003     ; inserted
//!       0x0003     ; inserted
//!       0x0003     ; inserted; requires v20@EAX
//!     block 0x0009
//!       0x000a  cmp DX, CX  ; inserted
//!       0x000a  jae   ; inserted
//!     ; ...
//! ```
//!
//! After:
//! ```asm
//! _total proc near
//!     push si
//! L0_0:
//!     mov si, ax
//!     ; ...
//!     cmp dx, 4
//!     jne L0_9
//!     pop si
//!     ret
//! L0_9:
//!     cmp dx, cx
//!     jae L0_18
//!     ; ...
//! ```
//!
//! From `ISEL_DUMP=1` and `-S` of a `total` over a `&[u16]`
//! (`-fno-inline-functions`) at `-m16 -O2`.
//!
//! # Runs
//! Once per procedure after the machine phases. `cleaned_returns` runs in
//! `assemble::assembled_by` (`masm cleaned returns`), before the return
//! duplication there; `listing` runs when the object is
//! built or `-S` is printed.
//!
//! # See also
//! [`crate::backend::objbuild`] for the encoder of the same items,
//! [`crate::backend::shrinkwrap`], [`crate::backend::stackusage`].

use std::collections::BTreeSet;
use std::fmt;
use std::sync::{Arc, LazyLock};

use llrm_lir::registers::Regs;
use llrm_lir::registers::{RegId, RegSet};
use llrm_object::assembler::Cfi;

use crate::backend::{select, target};
use crate::model::ir::{self, Addr, Loc, Operation, Semantics, Space};
use crate::model::lir::{self, SrcLoc};
use crate::support::hash::IndexMap;
use crate::support::pyrepr::Repr;

/// What LIR calls the frame register and the stack pointer, whatever the
/// target: `spelled` gives each its own.
const FRAME: RegId = ir::FRAME;
const STACK: RegId = ir::STACK;

/// `SIZES`.
pub static SIZES: LazyLock<IndexMap<u32, &'static str>> = LazyLock::new(|| {
    IndexMap::from_iter([(1, "byte"), (2, "word"), (4, "dword"), (8, "qword"), (10, "tbyte"), (16, "xmmword")])
});
/// `SEGMENTS`.
pub static SEGMENTS: LazyLock<IndexMap<&'static str, &'static str>> =
    LazyLock::new(|| IndexMap::from_iter([("_DATA", ".data"), ("_BSS", ".data?"), ("CONST", ".const")]));

/// An instruction or operand this printer has no spelling for.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Unprintable(pub String);

impl fmt::Display for Unprintable {
    fn fmt(
        &self,
        formatter: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for Unprintable {}

impl From<Unprintable> for crate::model::passes::Exception {
    fn from(one: Unprintable) -> Self {
        Self::defined_in("qbopt.backend.masm", "Unprintable", one.0)
    }
}

/// `InlinePart`: bytes, or `(kind, symbol, offset)` for a fixup.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InlinePart {
    Bytes(Vec<u8>),
    Fixup(String, String, i64),
    /// A near call (0xE8) or jump (0xE9) to a symbol: the opcode and the name.
    Near(u8, String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Callee {
    pub name: String,
    pub far: bool,
    /// The argument bytes it pops as it returns: the stack is that much higher
    /// after the call.
    pub pops: i64,
    /// Inline assembly laid down in place of a call.
    pub code: Vec<InlinePart>,
    /// A sibling call: a `jmp` after the epilogue, not a `call`.
    pub tail: bool,
}

impl Callee {
    /// `Callee(name, far)`, with no inline code.
    pub fn new(
        name: impl Into<String>,
        far: bool,
    ) -> Self {
        Self { name: name.into(), far, pops: 0, code: Vec::new(), tail: false }
    }
}

pub use crate::hir::model::StackCheck;

/// The data object `StackCheck::limit` names, which no global takes.
pub const STACK_LIMIT_ID: i64 = i64::MAX - 1;

#[derive(Clone, Debug, PartialEq)]
pub struct Procedure {
    pub name: String,
    pub public: bool,
    pub far: bool,
    pub body: lir::LirBody,
    /// bytes below bp: locals and spill slots
    pub reserve: i64,
    pub callees: IndexMap<i64, Callee>,
    /// An interrupt handler's data group, whose selector it loads into DS
    /// and ES; `None` for a procedure entered by a call.
    pub interrupt: Option<Addr>,
    /// Tuned for size (-Os): the jumps `objbuild` lays out.
    pub size: bool,
    /// Bytes the runtime's entry call (B$ENSA) takes below BP, which no
    /// instruction of the procedure shows: its header and the locals `cx`
    /// names.
    pub entry: i64,
    /// Compare SP with the runtime's limit once the frame is allocated.
    pub stack_check: Option<StackCheck>,
    /// The routine that touches the pages of a frame of 4 KiB or more before
    /// its `sub` (`__chkstk` on Windows: `mov eax, N; call __chkstk; sub rsp,
    /// rax`), where the target's object format calls for one.
    pub probe_stack: Option<String>,
    /// The target's frame register, stack pointer and callee-saved registers.
    pub registers: llrm_target::FrameRegisters,
}

/// A near procedure of `code` and a return, called by name: a routine of the
/// target's that the module holds once.
pub fn routine_procedure(
    name: &str,
    code: Vec<u8>,
    registers: llrm_target::FrameRegisters,
    regs: Regs,
) -> Procedure {
    let what = Semantics { name: Some(name.into()), ..Semantics::new(Operation::Call) };
    let instruction = lir::Insn::new(1, None, Some(what), vec![], vec![]);
    // The return is an instruction, so the frame the printer opens is closed
    // before it: a `ret` among the bytes would skip the epilogue.
    let back = Semantics { name: Some("ret".into()), ..Semantics::new(Operation::Return) };
    let back = lir::Insn::new(2, None, Some(back), vec![], vec![]);
    let mut body = lir::LirBody::new(
        name,
        1,
        vec![lir::LirBlock::new(1, vec![std::sync::Arc::new(instruction), std::sync::Arc::new(back)]).into()],
        Default::default(),
        Default::default(),
    );
    body.registers = Some(regs);
    Procedure {
        name: name.into(),
        public: false,
        far: false,
        body,
        reserve: 0,
        callees: IndexMap::from_iter([(
            1,
            Callee { name: name.into(), far: false, pops: 0, code: vec![InlinePart::Bytes(code)], tail: false },
        )]),
        interrupt: None,
        size: false,
        entry: 0,
        stack_check: None,
        probe_stack: None,
        registers,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Label {
    pub name: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Fill {
    pub size: i64,
    /// None: uninitialised
    pub byte: Option<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Pointer {
    pub name: String,
    pub offset: i64,
    /// A selector and an offset, where the target has selectors.
    pub far: bool,
    /// The bytes of the cell it fills: the target's pointer width (or the
    /// offset's, for a far one made of an offset word and a selector word).
    pub bytes: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Align {
    pub to: i64,
}

/// `MCAlignFragment`: pad the code to a multiple of `1 << log2` with
/// no-operations, unless that takes more than `max_skip` bytes (0: no limit).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CodeAlign {
    pub log2: u8,
    pub max_skip: u8,
}

/// What a target pads its code with and to: `getPrefFunctionAlignment` as a
/// power of two (0: none) and `getMaximumNopSize` (0 reads as 1).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Padding {
    pub function: u8,
    pub nop: u8,
}

/// `Datum`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Datum {
    Label(Label),
    /// A label heading data a writer drops when nothing names it: up to the
    /// next Object.
    Object(Label),
    Fill(Fill),
    Pointer(Pointer),
    Align(Align),
    Bytes(Vec<u8>),
    /// A frontend's own item, which Python's untyped `Module.data` carries
    /// through: QB's `_SegmentWord`, a `dw seg name` only its writer encodes.
    SegmentWord(String),
}

/// The bytes `items` occupy from segment offset `from` (an `Align` pads to a
/// multiple of its `to`).
pub fn data_size(
    items: &[Datum],
    from: i64,
) -> i64 {
    items.iter().fold(from, |at, item| match item {
        Datum::Label(_) | Datum::Object(_) => at,
        Datum::Fill(fill) => at + fill.size,
        Datum::Pointer(pointer) => at + i64::from(pointer.bytes) + if pointer.far { 2 } else { 0 },
        Datum::Align(align) if align.to > 1 => at + (align.to - at % align.to) % align.to,
        Datum::Align(_) => at,
        Datum::Bytes(bytes) => at + bytes.len() as i64,
        Datum::SegmentWord(_) => at + 2,
    }) - from
}

/// `items` cut into segments of at most `limit` bytes, each cut a byte count in
/// a `Fill` or `Bytes`: a huge object past 64K is laid down as consecutive 64K
/// segments, whose selectors the linker places `limit >> 4` apart, as a huge
/// pointer steps them. A pointer, a label or an alignment is never cut.
pub fn split_data(
    items: Vec<Datum>,
    limit: i64,
) -> Result<Vec<Vec<Datum>>, String> {
    let mut pieces: Vec<Vec<Datum>> = vec![Vec::new()];
    let mut used = 0_i64;
    for item in items {
        let room = limit - used;
        match item {
            Datum::Fill(Fill { size, byte }) => {
                let (mut left, mut room) = (size, room);
                while left > 0 {
                    if room == 0 {
                        pieces.push(Vec::new());
                        used = 0;
                        room = limit;
                    }
                    let here = left.min(room);
                    pieces.last_mut().expect("a piece").push(Datum::Fill(Fill { size: here, byte }));
                    used += here;
                    room -= here;
                    left -= here;
                }
            }
            Datum::Bytes(bytes) => {
                let mut rest = &bytes[..];
                while !rest.is_empty() {
                    if used == limit {
                        pieces.push(Vec::new());
                        used = 0;
                    }
                    let here = rest.len().min((limit - used) as usize);
                    pieces.last_mut().expect("a piece").push(Datum::Bytes(rest[..here].to_vec()));
                    used += here as i64;
                    rest = &rest[here..];
                }
            }
            other => {
                let size = data_size(std::slice::from_ref(&other), used);
                if used + size > limit {
                    if size > 0 && size < limit && used == limit {
                        pieces.push(Vec::new());
                        used = 0;
                    } else if size > 0 {
                        return Err("an item of a huge object crosses a 64K boundary".to_owned());
                    }
                }
                used += size;
                pieces.last_mut().expect("a piece").push(other);
            }
        }
    }
    Ok(pieces)
}

#[derive(Clone, Debug, PartialEq)]
pub struct Module {
    /// The register file of the target the module is for.
    pub registers: Regs,
    /// the code segment's name, MODULE_TEXT
    pub code: String,
    pub names: IndexMap<(Space, i64), String>,
    /// (name, "far" | "near" | "byte")
    pub externs: Vec<(String, String)>,
    pub publics: Vec<String>,
    /// (segment, items)
    pub data: Vec<(String, Vec<Datum>)>,
    pub procedures: Vec<Procedure>,
    /// data segments outside DGROUP; only ever asked for membership
    pub private: BTreeSet<String>,
    /// Of those, the segments of uninitialised data: class FAR_BSS, which an
    /// image does not store.
    pub far_bss: BTreeSet<String>,
    /// The code segment of a procedure that is not in `code` (`#pragma
    /// code_seg`), by its name.
    pub code_segments: std::collections::BTreeMap<String, String>,
    /// Externs nothing references, declared so LINK pulls in their module.
    pub requests: BTreeSet<String>,
    /// `-g`'s debug information.
    pub debug: Option<super::debuginfo::Debug>,
    /// Bytes of the linker's stack this module adds to the others' (OMF stack
    /// segments concatenate): where its call graph is the whole program's.
    pub stack: i64,
    /// The target's object format: the listing's header, the writer's mode.
    pub object: llrm_target::object::ObjectFormat,
    /// How the code is padded.
    pub padding: Padding,
}

impl Module {
    /// Whether `segment` is outside DGROUP, so reached by its own selector
    /// alone and paragraph aligned: its first byte is offset 0 of its
    /// frame, and a 64K object in it fits. Every writer asks this.
    pub fn selector_addressed(
        &self,
        segment: &str,
    ) -> bool {
        self.private.contains(segment)
    }
}

pub fn text(module: &Module) -> Result<String, Unprintable> {
    text_by(module, |procedure, number| listing(procedure, number))
}

/// `module`'s text, each procedure's items as `listed` gives them.
pub fn text_by(
    module: &Module,
    listed: impl Fn(&Procedure, usize) -> Result<Vec<Item>, Unprintable>,
) -> Result<String, Unprintable> {
    // The listing writer is a step of its own: with `-S` it was 21 M of a
    // d_faces -O2 compile that no timed step held.
    crate::support::debug::timed("masm text", || text_listed(module, listed))
}

fn text_listed(
    module: &Module,
    listed: impl Fn(&Procedure, usize) -> Result<Vec<Item>, Unprintable>,
) -> Result<String, Unprintable> {
    let mut out: Vec<String> = module.object.header.iter().cloned().chain([String::new()]).collect();
    out.extend(module.publics.iter().map(|name| format!("public {name}")));
    if module.stack > 0 {
        out.push(format!(".stack {}", module.stack));
    }
    for (segment, items) in &module.data {
        let private = module.selector_addressed(segment);
        out.push(SEGMENTS.get(segment.as_str()).map_or_else(
            || {
                format!(
                    "{segment} segment {} public '{}'",
                    if private { "para" } else { "word" },
                    if module.far_bss.contains(segment) {
                        "FAR_BSS"
                    } else if segment == "BSSB" {
                        "BSS"
                    } else if private {
                        "FAR_DATA"
                    } else {
                        "DATA"
                    }
                )
            },
            |one| (*one).to_owned(),
        ));
        out.extend(
            module.externs.iter().filter(|(_, kind)| kind == "byte").map(|(name, _)| format!("extern {name}:byte")),
        );
        out.extend(items.iter().flat_map(datum));
        if !SEGMENTS.contains_key(segment.as_str()) {
            out.push(format!("{segment} ends"));
            if !private {
                out.push(format!("DGROUP group {segment}"));
            }
        }
    }
    out.extend(
        module
            .externs
            .iter()
            .filter(|(_, kind)| kind != "byte")
            .map(|(name, kind)| format!("extern {name}:{}", if kind == "far-byte" { "byte" } else { kind })),
    );
    out.push(format!(".code {}", module.code));
    for (number, procedure) in module.procedures.iter().enumerate() {
        out.extend(_procedure_of(procedure, listed(procedure, number)?, &module.names, number)?);
    }
    out.push("end".into());
    Ok(out.join("\n") + "\n")
}

/// The jump tables of the procedures that were kept, named for the jumps that
/// read them: for each index of a table jump the code address of its block, a
/// pointer as wide as the module's.
pub fn jump_tables(
    procedures: &[Procedure],
    names: &mut IndexMap<(Space, i64), String>,
    module: &llrm_mir::Module,
) -> Vec<Datum> {
    let bytes = module
        .datalayout
        .as_deref()
        .and_then(|text| llrm_mir::datalayout::DataLayout::parse(text).ok())
        .map_or(2, |layout| layout.pointer(0).bits / 8);
    let mut out = Vec::new();
    for (number, procedure) in procedures.iter().enumerate() {
        let jumps =
            procedure.body.blocks.iter().flat_map(|block| block.insns.iter()).filter_map(|one| one.what.as_ref());
        for what in jumps.filter(|what| what.op == Operation::Jump && what.indirect && !what.table.is_empty()) {
            let Some(Loc::Mem(ir::Mem { addr: Some(addr), .. })) = what.sources.first() else { continue };
            let name = format!("$J{}", addr.index);
            names.insert((Space::Segment, addr.index), name.clone());
            out.push(Datum::Object(Label { name }));
            out.extend(
                what.table
                    .iter()
                    .map(
                        |&target| Datum::Pointer(Pointer { name: label(number, target), offset: 0, far: false, bytes }),
                    ),
            );
        }
    }
    out
}

pub fn datum(item: &Datum) -> Vec<String> {
    match item {
        Datum::Label(Label { name }) | Datum::Object(Label { name }) => vec![format!("{name} label byte")],
        Datum::Fill(Fill { size, byte }) => {
            vec![format!("    db {size} dup ({})", byte.map_or_else(|| "?".to_owned(), |one| one.to_string()))]
        }
        Datum::Pointer(Pointer { name, offset, bytes, .. }) => {
            vec![format!(
                "    {} {name}{}",
                match bytes {
                    4 => "dd",
                    2 => "dw",
                    8 => "dq",
                    other => panic!("a pointer of {other} bytes"),
                },
                _signed(*offset)
            )]
        }
        Datum::Align(Align { to }) => vec![format!("    align {to}")],
        Datum::Bytes(item) => _code(&[InlinePart::Bytes(item.clone())]),
        // `_code((item,))` yields nothing for an item it cannot match.
        Datum::SegmentWord(_) => Vec::new(),
    }
}

/// `Item`.
#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    /// The block that follows starts at a multiple of this (`setAlignment`).
    Align(CodeAlign),
    Label(Label),
    Callee(Callee),
    Semantics(Semantics),
    Mark(Mark),
    /// A `.cfi_*` directive (`MCCFIInstruction`), where LLVM's lands: no
    /// code of its own.
    Cfi(llrm_object::assembler::Cfi),
    /// A `.seh_*` directive (Win64): no code of its own, stamped with the
    /// offset after the instruction before it.
    Seh(llrm_object::Seh),
}

/// `-g`: a place in the code a debugger is told of.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mark {
    /// The code after it is this source line's: the procedure's `index`th
    /// line, where [`line_label`] names it.
    Line { line: SrcLoc, index: u32 },
    /// The procedure's own code starts, its prologue done: the first code
    /// of a source line.
    BodyStart,
    /// The procedure's own code ends, its epilogue next: after the last
    /// code of a source line but a return.
    BodyEnd,
    /// The call before it popped this many bytes of its arguments as it
    /// returned: the stack is that much higher from here.
    Pops(i64),
    /// The instruction before it made value `tag` (by the number its register
    /// had in SSA), in `place`.
    Def { tag: u32, place: Place },
    /// Note `note` of the body (`LirBody::notes`) stands here.
    Note(u32),
    /// The call before it clobbers this register (one mark for each it
    /// clobbers).
    Clobbered(RegId),
    /// The procedure declares its call frame information (`Item::Cfi`): it
    /// has a frame descriptor even where it has no directive.
    Declared,
    /// A shrink-wrapped frame has been set up by the instructions before it.
    Opened,
    /// A shrink-wrapped frame has been taken back by the instructions before
    /// it.
    Closed,
}

/// Where an instruction put a value `-g` names.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Place {
    Register(RegId),
    /// A frame cell, as the frame register would address it at this
    /// displacement.
    Cell {
        disp: i64,
        bytes: u32,
    },
}

/// The arguments the caller pushed, as values in their cells from the first
/// byte.
fn blocks_arguments(body: &lir::LirBody) -> Vec<Item> {
    body.arguments_in_cells
        .iter()
        .map(|&(tag, disp, bytes)| Item::Mark(Mark::Def { tag, place: Place::Cell { disp, bytes } }))
        .collect()
}

/// Where an instruction wrote `dest`, if a debugger can be told: a register, or
/// a frame cell addressed by displacement alone.
fn placed(dest: &Loc) -> Option<Place> {
    match dest {
        Loc::Reg(ir::Reg { register, .. }) => Some(Place::Register(*register)),
        Loc::Mem(cell) => match &cell.addr {
            Some(addr) if addr.space == Space::Frame && cell.index.is_none() && cell.base.is_none() => {
                Some(Place::Cell { disp: addr.disp, bytes: cell.width })
            }
            _ => None,
        },
        _ => None,
    }
}

/// The call frame information a prologue and epilogue declare
/// (`X86FrameLowering::emitPrologue`/`emitEpilogue`): the frame address as
/// each instruction of the entry and exit sequences moves it. `depth` is the
/// frame address minus the stack pointer; once the frame register is set the
/// address is `frame` past it and pushes and pops no longer move it.
#[derive(Clone, Debug)]
struct CfiState {
    slot: i64,
    sp: u16,
    bp: u16,
    depth: i64,
    framed: bool,
    frame: i64,
    /// The stack pointer as the body names it, at the width of an address: the
    /// sequences name the pseudo `STACK` at another.
    stack: RegId,
}

impl CfiState {
    fn entry(
        slot: i64,
        sp: u16,
        bp: u16,
        stack: RegId,
    ) -> Self {
        CfiState { slot, sp, bp, depth: slot, framed: false, frame: 0, stack }
    }

    fn is_stack(
        &self,
        place: Option<&Loc>,
    ) -> bool {
        matches!(
            place,
            Some(Loc::Reg(one)) if one.register == STACK || one.register == self.stack
        )
    }

    /// The directive that restates the frame address after code that left it
    /// (a return): the prologue's.
    fn restate(&self) -> Item {
        Item::Cfi(if self.framed {
            Cfi::DefCfa { register: self.bp, offset: self.frame }
        } else {
            Cfi::DefCfaOffset { offset: self.depth }
        })
    }

    /// The directive for a call that popped `bytes` of its arguments as it
    /// returned (`.cfi_adjust_cfa_offset -bytes`).
    fn after_pops(
        &mut self,
        bytes: i64,
    ) -> Vec<Item> {
        self.depth -= bytes;
        if self.framed || bytes == 0 {
            return Vec::new();
        }
        vec![Item::Cfi(Cfi::DefCfaOffset { offset: self.depth })]
    }

    /// The directives that follow `what`, where `X86FrameLowering` states them.
    /// `bp` is the frame register as the sequence names it.
    fn after(
        &mut self,
        what: &Semantics,
        bp: &Loc,
    ) -> Vec<Item> {
        let imm = |loc: &Loc| match loc {
            Loc::Imm(imm) => Some(imm.value),
            _ => None,
        };
        let mut out = Vec::new();
        match (what.op, what.name.as_deref()) {
            (Operation::Push, _) => {
                self.depth += self.slot;
                if &what.sources[0] == bp {
                    // The frame register's own save is declared at once, the
                    // others' at the end of the prologue.
                    out.push(Cfi::DefCfaOffset { offset: self.depth });
                    out.push(Cfi::Offset { register: self.bp, offset: -self.depth });
                } else if !self.framed {
                    out.push(Cfi::DefCfaOffset { offset: self.depth });
                }
            }
            (Operation::Pop, _) => {
                self.depth -= self.slot;
                if self.framed && what.dests.first() == Some(bp) {
                    self.framed = false;
                    out.push(Cfi::DefCfa { register: self.sp, offset: self.depth });
                } else if !self.framed {
                    out.push(Cfi::DefCfaOffset { offset: self.depth });
                }
            }
            (Operation::Move, _) if what.dests.first() == Some(bp) && self.is_stack(what.sources.first()) => {
                self.framed = true;
                self.frame = self.depth;
                out.push(Cfi::DefCfaRegister { register: self.bp });
            }
            (Operation::Binary, Some(name @ ("sub" | "add"))) if self.is_stack(what.dests.first()) => {
                let size = what.sources.get(1).and_then(imm).unwrap_or(0);
                self.depth += if name == "sub" { size } else { -size };
                if !self.framed && size != 0 {
                    out.push(Cfi::DefCfaOffset { offset: self.depth });
                }
            }
            (Operation::Address, _) => {
                if let Some(Loc::Mem(from)) = what.sources.first() {
                    self.depth = self.frame - from.offset;
                }
            }
            (Operation::Nothing, Some("leave")) => {
                self.depth = self.frame - self.slot;
                self.framed = false;
                out.push(Cfi::DefCfa { register: self.sp, offset: self.depth });
            }
            _ => {}
        }
        out.into_iter().map(Item::Cfi).collect()
    }
}

/// The `.seh_*` directive a prologue instruction needs
/// (`X86FrameLowering::BuildCFI`'s Win64 arm): `.seh_pushreg` for a push of a
/// register the function must keep, `.seh_stackalloc` for `sub rsp` and for a
/// push of any other register, `.seh_setframe` where the frame register is set
/// from `rsp`. `hardware` is a register's number in the instruction set.
fn seh_after(
    what: &Semantics,
    bp: &Loc,
    hardware: &dyn Fn(&Loc) -> Option<u8>,
) -> Option<Item> {
    use llrm_object::Seh;
    let imm = |loc: &Loc| match loc {
        Loc::Imm(imm) => Some(imm.value),
        _ => None,
    };
    let seh = match (what.op, what.name.as_deref()) {
        (Operation::Push, _) => {
            let number = hardware(what.sources.first()?)?;
            if llrm_x86::mc::win64unwind::is_nonvolatile(number) { Seh::PushReg(number) } else { Seh::StackAlloc(8) }
        }
        (Operation::Move, _) if what.dests.first() == Some(bp) && what.sources.first() == Some(&sp_reg()) => {
            Seh::SetFrame(hardware(bp)?, 0)
        }
        (Operation::Address, _) if what.dests.first() == Some(bp) => match what.sources.first() {
            Some(Loc::Mem(from)) => Seh::SetFrame(hardware(bp)?, u64::try_from(from.offset).ok()?),
            _ => return None,
        },
        (Operation::Binary, Some("sub")) if what.dests.first() == Some(&sp_reg()) => {
            Seh::StackAlloc(u64::try_from(what.sources.get(1).and_then(imm)?).ok()?)
        }
        _ => return None,
    };
    Some(Item::Seh(seh))
}

/// `X86FrameLowering::emitStackProbe`: a frame past a page is allocated through
/// the probe routine, which leaves `rsp` for the `sub` to move (`mov eax, N;
/// call __chkstk; sub rsp, rax`).
fn probed_entry(
    procedure: &Procedure,
    enter: Vec<Semantics>,
) -> Vec<Semantics> {
    let Some(((symbol, (wide, _)), regs)) =
        procedure.probe_stack.as_deref().zip(procedure.registers.slot_scratch).zip(procedure.body.registers.as_ref())
    else {
        return enter;
    };
    let size = reg(regs.0.named(wide, 4));
    let mut out = Vec::new();
    for one in enter {
        let bytes = match (one.op, one.name.as_deref(), one.dests.first(), one.sources.get(1)) {
            (Operation::Binary, Some("sub"), Some(dest), Some(Loc::Imm(imm)))
                if *dest == sp_reg() && imm.value >= STACK_PROBE_PAGE as i64 =>
            {
                imm.value
            }
            _ => {
                out.push(one);
                continue;
            }
        };
        out.extend([
            semantics(
                Operation::Move,
                "mov",
                vec![size.clone()],
                vec![Loc::Imm(ir::Imm { value: bytes, width: 2, address: None })],
            ),
            Semantics { name: Some(symbol.into()), ..Semantics::new(Operation::Call) },
            semantics(Operation::Binary, "sub", vec![sp_reg()], vec![sp_reg(), reg(wide)]),
        ]);
    }
    out
}

/// An entry instruction as an item: the stack probe's call is a callee.
fn entry_item(
    one: &Semantics,
    probe: Option<&str>,
) -> Item {
    match (one.op, &one.name) {
        (Operation::Call, Some(name)) if !one.indirect && probe == Some(&**name) => {
            Item::Callee(Callee::new(&**name, false))
        }
        _ => Item::Semantics(one.clone()),
    }
}

/// The immediate a `mov reg, imm` of the entry moves.
fn size_moved(one: &Semantics) -> Option<i64> {
    match (one.op, one.sources.first()) {
        (Operation::Move, Some(Loc::Imm(imm))) if matches!(one.dests.first(), Some(Loc::Reg(_))) => Some(imm.value),
        _ => None,
    }
}

/// `sub rsp, reg` after `mov reg, size` as the `sub rsp, size` it makes.
fn probed(
    one: &Semantics,
    size: Option<i64>,
) -> Option<Semantics> {
    let reg_source = matches!(one.sources.get(1), Some(Loc::Reg(_)));
    if one.op != Operation::Binary || one.name.as_deref() != Some("sub") || one.dests.first() != Some(&sp_reg()) {
        return None;
    }
    let (true, Some(value)) = (reg_source, size) else { return None };
    let mut stated = one.clone();
    stated.sources[1] = Loc::Imm(ir::Imm { value, width: 2, address: None });
    Some(stated)
}

fn reg(register: RegId) -> Loc {
    Loc::Reg(ir::Reg { register, width: 2 })
}

fn semantics(
    op: Operation,
    name: &str,
    dests: Vec<Loc>,
    sources: Vec<Loc>,
) -> Semantics {
    Semantics { name: Some(name.into()), dests, sources, ..Semantics::new(op) }
}

/// Every return of `body` as `retf bytes`, for a callee that removes its
/// arguments. `ret imm16` cannot remove 64 KB or more: a near return of a
/// 32-bit target keeps the count, and the epilogue writes `pop [esp + n]; add
/// esp, n; ret` (`moved_return`); `address` is the bytes of a return address.
pub fn cleaned_returns(
    body: &lir::LirBody,
    bytes: i64,
    address: i64,
) -> Result<lir::LirBody, String> {
    if !(0..=llrm_x86::calling::RET_POPS_MOST).contains(&bytes)
        && !(bytes > llrm_x86::calling::RET_POPS_MOST && address == 4)
    {
        return Err("far-return cleanup exceeds 16 bits".into());
    }
    if bytes == 0 {
        return Ok(body.clone());
    }
    let blocks = body
        .blocks
        .iter()
        .map(|block| {
            let insns = block
                .insns
                .iter()
                .map(|one| match one.what.as_ref() {
                    Some(what) if what.op == Operation::Return && what.name.as_deref() != Some("jmp") => {
                        let mut replaced = (**one).clone();
                        replaced.what = Some(Semantics {
                            sources: vec![Loc::Imm(ir::Imm {
                                value: bytes,
                                width: if bytes > llrm_x86::calling::RET_POPS_MOST { 4 } else { 2 },
                                address: None,
                            })],
                            ..what.clone()
                        });
                        Arc::new(replaced)
                    }
                    _ => Arc::clone(one),
                })
                .collect();
            block.with_insns(insns)
        })
        .collect();
    Ok(body.with_blocks(blocks))
}

/// Names the limit each procedure's stack check compares with, and the externs
/// those checks need: the limit a data object, the handler a far routine,
/// unless a procedure here is it.
pub fn stack_externs(
    procedures: &[Procedure],
    names: &mut IndexMap<(Space, i64), String>,
) -> Vec<(String, String)> {
    let mut externs = Vec::new();
    for check in procedures.iter().filter_map(|one| one.stack_check.as_ref()) {
        names.insert((Space::External, STACK_LIMIT_ID), check.limit.clone());
        externs.push((check.limit.clone(), "byte".to_owned()));
        if !procedures.iter().any(|one| one.name == check.handler) {
            externs.push((check.handler.clone(), if check.far { "far" } else { "near" }.to_owned()));
        }
    }
    externs
}

/// Whether a procedure, by its symbol, is entered only by the direct calls
/// `module`'s own functions make: its address is taken nowhere
/// (`callgraph::addressed`).
pub fn entered_directly<'a>(
    module: &'a llrm_mir::Module,
    names: &'a IndexMap<(Space, i64), String>,
) -> impl Fn(&str) -> bool + 'a {
    let addressed: BTreeSet<&String> = llrm_mir::callgraph::addressed(module)
        .iter()
        .filter_map(|id| names.get(&(Space::Segment, i64::from(id.0))))
        .collect();
    move |name| !addressed.contains(&name.to_owned())
}

/// The block number the overflow call takes: past every instruction's.
pub fn cold_at(body: &lir::LirBody) -> i64 {
    body.blocks.iter().map(|block| block.at).chain(body.insns().into_iter().map(|one| one.at)).max().unwrap_or(0) + 1
}

fn sp_reg() -> Loc {
    reg(STACK)
}

/// Whether a return of `body` is a sibling call (`jmp`).
fn sibling_calls(body: &lir::LirBody) -> bool {
    body.blocks
        .iter()
        .flat_map(|block| block.insns.iter())
        .any(|one| one.what.as_ref().is_some_and(|what| what.is_sibling_call()))
}

/// Whether a call in `body` leaves SP somewhere else (`alloca`). The registers
/// it saves are then all pushed at the entry, and the return sets SP from the
/// frame register.
fn moves_stack(body: &crate::model::lir::LirBody) -> bool {
    body.facts.0.get::<BodyRoots>(body).moves_stack
}

/// The registers `procedure` keeps for its caller: each one it names, or a call
/// it makes disturbs by its convention, that this convention leaves to the
/// callee, by its low half. A call to a routine whose convention disturbs more
/// than this one's (cdecl's ECX and EDX under Watcom's) takes the caller's
/// value.
fn saved_of(procedure: &Procedure) -> Vec<RegId> {
    if let Some(saved) = &procedure.body.frame_saved {
        return saved.clone();
    }
    let found = procedure.body.facts.0.get::<BodyRoots>(&procedure.body);
    let mut roots = if procedure.registers.free {
        found.of_values(procedure.body.regs(), procedure.registers.pointer)
    } else {
        found.plain.clone()
    };
    roots.extend(found.disturbed.iter());
    // An interrupt handler has saved everything before its frame, and a
    // runtime-built one has saved SI and DI (B$ENRA/B$ENRD, restored by
    // B$EXSA).
    let owned = procedure.interrupt.is_some() || procedure.entry != 0;
    procedure.registers.saved.iter().filter(|(whole, _)| !owned && roots.contains(whole)).map(|(_, low)| *low).collect()
}

/// Where `procedure` saves them, and sets up its frame where `frame`, when that
/// is not its entry (`shrinkwrap`).
pub(crate) fn wrap_of(
    procedure: &Procedure,
    body: &crate::model::lir::LirBody,
    frame: bool,
    stack_addressed: bool,
) -> Option<crate::backend::shrinkwrap::Wrap> {
    if moves_stack(body) || realign_of(body) != 0 {
        return None;
    }
    let lows = saved_of(procedure);
    let kept: BTreeSet<RegId> =
        procedure.registers.saved.iter().filter(|(_, low)| lows.contains(low)).map(|(whole, _)| *whole).collect();
    // The runtime's entry builds an `entry` frame, an interrupt handler and the
    // stack check run at the entry, and inline code may address the frame.
    let movable = procedure.entry == 0
        && procedure.interrupt.is_none()
        && procedure.stack_check.is_none()
        && procedure.callees.values().all(|one| !may_address_frame(&one.code));
    let registers = &procedure.registers;
    crate::backend::shrinkwrap::wrapped(
        body,
        &kept,
        (frame && movable).then_some((registers.pointer, registers.stack)),
        stack_addressed,
        procedure.size && !registers.wrap_split_size,
    )
}

/// The alignment the stack pointer is realigned to for `body`'s over-aligned
/// locals; 0 where none.
fn realign_of(body: &lir::LirBody) -> i64 {
    body.over_aligned.iter().map(|&(_, align)| align).max().unwrap_or(0)
}

/// Why `procedure` cannot have its stack pointer realigned, if it cannot: it
/// is refused rather than compiled with a local off its alignment.
///
/// DEVIATE RA1 (owner w4, 2026-10-28): a dynamic `alloca` beside an
/// over-aligned local is refused; the pin addresses the locals through a base
/// pointer (`X86RegisterInfo::hasBasePointer`). DEVIATE RA2 (owner w4,
/// 2026-10-28): `-g` variables are refused; their locations need the canonical
/// frame address, which a realigned frame moves. Exit for both: the pin's
/// base pointer and CFA locations in `lowered`.
pub fn realign_refusal(procedure: &Procedure) -> Option<String> {
    if realign_of(&procedure.body) == 0 {
        return None;
    }
    let body = &procedure.body;
    // No pin alignment exists at 16 bits (real mode has no `and sp` frame the
    // pin builds).
    let why = if !matches!(body.bits, 32 | 64) {
        "in 16-bit code"
    } else if procedure.entry != 0
        || procedure.interrupt.is_some()
        || procedure.far
        || procedure.stack_check.is_some()
        || procedure.callees.values().any(|one| !one.code.is_empty())
    {
        "in a frame the entry or inline code builds"
    } else if moves_stack(body) {
        "with a dynamic alloca"
    } else if !body.variables.is_empty() {
        "with debug variables"
    } else if body.insns().iter().any(|one| {
        one.what
            .as_ref()
            .is_some_and(
                |what| (what.dests.iter().any(writes_stack_pointer) && stack_adjustment(what).is_none())
                    || matches!(
                        what.op,
                        Operation::Pop if matches!(what.dests[..], [Loc::Mem(_)])
                    ),
            )
    }) {
        "with the stack pointer written in the body"
    } else {
        return None;
    };
    Some(format!("{}: an over-aligned local {why} is not realigned", procedure.name))
}

/// `procedure` set up as a realigned frame where it has over-aligned locals,
/// and as it is otherwise (unoptimized code keeps its registers after the
/// locals' space), except where it calls: its outgoing arguments are stored at
/// `[sp]` and up, where registers pushed after the space would be.
pub fn realigned(procedure: Procedure) -> Procedure {
    let calls = matches!(procedure.body.bits, 32 | 64)
        && procedure
            .body
            .blocks
            .iter()
            .flat_map(|block| block.insns.iter())
            .any(|one| one.what.as_ref().is_some_and(|what| what.op == Operation::Call));
    if realign_of(&procedure.body) == 0 && !calls { procedure } else { saved_first(procedure) }
}

/// `procedure` with its callee-saved registers pushed before the space for its
/// locals, as the pin's frame is (`push ebp; mov ebp, esp; push esi; sub esp,
/// N`), and every local that much lower. Unchanged where the entry or a piece
/// of the frame is set up elsewhere: runtime-built, interrupt, stack-checked,
/// far, inline-code, 16-bit and shrink-wrapped frames.
pub fn saved_first(procedure: Procedure) -> Procedure {
    let eligible = procedure.entry == 0
        && procedure.interrupt.is_none()
        && !procedure.far
        && matches!(procedure.body.bits, 32 | 64)
        && (procedure.reserve != 0 || moves_stack(&procedure.body))
        && procedure.callees.values().all(|one| one.code.is_empty())
        && [false, true].iter().all(|omit| {
            // Nothing wrapped, or the frame and every register at the one Save
            // (the pin saves them together); a register saved later than the
            // frame cannot lie above it.
            wrap_of(&procedure, &procedure.body, true, *omit).is_none_or(|wrap| {
                wrap.wrapped.is_empty()
                    || (wrap.wrapped.contains(&crate::backend::shrinkwrap::Piece::Frame)
                        && saved_of(&procedure).len()
                            == wrap
                                .wrapped
                                .iter()
                                .filter(|piece| matches!(piece, crate::backend::shrinkwrap::Piece::Saved(_)))
                                .count())
            })
        });
    let pushed = saved_of(&procedure).len() as i64 * procedure.registers.slot;
    // A frame with no register to push is still the pin's where it keeps the
    // frame register and moves the stack pointer (`alloca`): `mov rsp, rbp;
    // pop rbp`, not `lea rsp, [rbp - N]; leave`.
    let rebuilt = matches!(procedure.body.bits, 64) && moves_stack(&procedure.body);
    // The locals of a realigned stack lie below its aligned top, not below the
    // registers: nothing moves them.
    if realign_of(&procedure.body) != 0 {
        return if eligible {
            Procedure { body: lir::LirBody { saved_first: true, ..procedure.body.clone() }, ..procedure }
        } else {
            procedure
        };
    }
    if !eligible || (pushed == 0 && !rebuilt) {
        return procedure;
    }
    // The locals keep their alignment from BP (an 8-byte cell at 4 mod 8
    // costs a 486 3 clocks): they lie a multiple of it below the registers,
    // and the gap is reserved with them.
    let slot = procedure.registers.slot;
    let align = procedure.registers.align.max(slot);
    // Where the frame register is omitted and that is settled without a size
    // comparison (`frame_omitted`), its cell is not reserved: the return
    // address and the registers are what the locals are aligned below. The
    // 32-bit frames keep it (Watcom's layout, not the pin's).
    let dropped = matches!(procedure.body.bits, 64) && {
        let held = frame_held(&procedure);
        (!procedure.size || held || procedure.registers.free) && stack_addressed(&procedure, 0).is_some()
    };
    let above = if dropped { slot } else { 0 };
    let gap = (align - (above + pushed) % align) % align;
    match crate::backend::relayout::pushed_down(&procedure.body, pushed + gap - above) {
        Some(body) => Procedure {
            body: lir::LirBody { saved_first: true, slot_dropped: dropped, ..body },
            reserve: procedure.reserve + gap,
            ..procedure
        },
        None => procedure,
    }
}

/// `procedure` with what each return takes back as instructions before the
/// return, as the pin has them when `BranchFolder` runs
/// (`PrologEpilogInserter` runs first): the saved registers' pops and the
/// frame's teardown, which tail merging then counts. Unchanged where the
/// entry or a piece of the frame is set up elsewhere (runtime-built,
/// interrupt) or placed by `shrinkwrap`: those stay in the printer.
pub fn exits_inserted(procedure: Procedure) -> Procedure {
    if procedure.entry != 0 || procedure.interrupt.is_some() || procedure.body.frame_exit.is_some() {
        return procedure;
    }
    let held = frame_held(&procedure);
    let omit = frame_omitted(&procedure, 0, held).is_some();
    if !omit && held {
        return procedure;
    }
    let sequences = Sequences::of(&procedure, omit);
    if sequences.wrap.as_ref().is_some_and(|wrap| !wrap.wrapped.is_empty()) || sequences.exit.is_empty() {
        return procedure;
    }
    let saved = saved_of(&procedure);
    let blocks: Vec<crate::model::lir::Block> = procedure
        .body
        .blocks
        .iter()
        .map(|block| {
            if !block.insns.iter().any(|one| one.what.as_ref().is_some_and(|what| what.op == Operation::Return)) {
                return block.clone();
            }
            let mut made = lir::LirBlock::clone(block);
            let mut insns: Vec<Arc<lir::Insn>> = Vec::with_capacity(block.insns.len() + sequences.exit.len());
            for one in block.insns.iter() {
                if one.what.as_ref().is_some_and(|what| what.op == Operation::Return) {
                    let at = one.covers.map_or(one.at, |covers| covers.0);
                    insns.extend(sequences.exit.iter().map(|what| {
                        Arc::new(lir::Insn::new(one.at, Some((at, at)), Some(what.clone()), Vec::new(), Vec::new()))
                    }));
                }
                insns.push(Arc::clone(one));
            }
            made.insns = insns.into();
            made.into()
        })
        .collect();
    let body = procedure.body.with_blocks(blocks);
    Procedure { body: lir::LirBody { frame_exit: Some(omit), frame_saved: Some(saved), ..body }, ..procedure }
}

/// The stack-limit comparison once the frame is allocated, if the procedure
/// has one.
fn stack_check(
    procedure: &Procedure,
    reserve: i64,
) -> Vec<Semantics> {
    let mut out = Vec::new();
    if procedure.stack_check.is_some() {
        let cold = Some(cold_at(&procedure.body));
        let overflow = |name: &str| Semantics { target: cold, ..semantics(Operation::Branch, name, vec![], vec![]) };
        let limit = Addr { index: STACK_LIMIT_ID, ..Addr::new(Space::External, 0) };
        // `sub` carries where SP wrapped past zero.
        if reserve != 0 {
            out.push(overflow("jb"));
        }
        out.extend([
            // The limit is a stack slot's word.
            semantics(
                Operation::Compare,
                "cmp",
                vec![],
                vec![sp_reg(), Loc::Mem(ir::Mem::new(Some(limit), procedure.registers.slot as u32))],
            ),
            overflow("jb"),
        ]);
    }
    out
}

/// A `saved_first` frame's entry and return sequences, the pin's
/// (`framelowering`): `push bp; mov bp, sp`, the registers, `sub sp, N`; and
/// `add sp, N` (`lea sp, [bp - registers]` after an `alloca`), the pops, `pop
/// bp`. DEVIATE PE1 (owner w-5, 2026-11-27): a one-slot adjustment stays a
/// `sub`/`add`, not the pin's `push eax`/`pop`: the register the push may
/// clobber is not asked of the entry's live-ins here.
fn lowered(
    procedure: &Procedure,
    saved: &[RegId],
    reserve: i64,
    framed: bool,
    omit: bool,
) -> (Vec<Semantics>, Vec<Semantics>) {
    use crate::backend::framelowering::{FrameInfo, Op, emit_epilogue, emit_prologue};
    // The last of the target's callee-saved list is pushed first
    // (`spillCalleeSavedRegisters` walks the list backwards).
    let saved: Vec<RegId> = saved.iter().rev().copied().collect();
    let saved = &saved[..];
    const FP: u32 = u32::MAX - 1;
    const EAX: u32 = u32::MAX;
    let slot = procedure.registers.slot;
    let info = FrameInfo {
        has_fp: framed,
        stack_size: (slot * saved.len() as i64 + reserve + if framed { slot } else { 0 }) as u64,
        slot_size: slot as u64,
        saved: (0..saved.len() as u32).collect(),
        frame_pointer: FP,
        realign_to: Some(realign_of(&procedure.body)).filter(|&align| align != 0).map(|align| align as u64),
        has_var_sized_objects: !omit && framed && moves_stack(&procedure.body),
    };
    let register = |reg_number: u32| if reg_number == FP { reg(FRAME) } else { reg(saved[reg_number as usize]) };
    let adjust = |name: &str, bytes: u64| {
        semantics(
            Operation::Binary,
            name,
            vec![sp_reg()],
            vec![sp_reg(), Loc::Imm(ir::Imm { value: bytes as i64, width: 2, address: None })],
        )
    };
    let lower = |op: Op| match op {
        Op::Push(EAX) => adjust("sub", slot as u64),
        Op::Push(number) => semantics(Operation::Push, "push", vec![], vec![register(number)]),
        Op::Pop(number) => semantics(Operation::Pop, "pop", vec![register(number)], vec![]),
        Op::MovFpSp => semantics(Operation::Move, "mov", vec![reg(FRAME)], vec![reg(STACK)]),
        Op::MovSpFp => semantics(Operation::Move, "mov", vec![reg(STACK)], vec![reg(FRAME)]),
        Op::LeaSpFp(offset) => {
            let from = ir::Mem { through: FRAME, offset, disp_width: 2, ..ir::Mem::new(None, 2) };
            semantics(Operation::Address, "lea", vec![sp_reg()], vec![Loc::Mem(from)])
        }
        Op::AndSp(align) => semantics(
            Operation::Binary,
            "and",
            vec![sp_reg()],
            vec![sp_reg(), Loc::Imm(ir::Imm { value: -(align as i64), width: 2, address: None })],
        ),
        Op::SubSp(bytes) => adjust("sub", bytes),
        Op::AddSp(bytes) => adjust("add", bytes),
    };
    let mut enter: Vec<Semantics> = Vec::new();
    for op in emit_prologue(&info, EAX) {
        let allocates = matches!(op, Op::SubSp(_) | Op::Push(EAX));
        enter.push(lower(op));
        if allocates {
            enter.extend(stack_check(procedure, reserve));
        }
    }
    let leave = emit_epilogue(&info, None).into_iter().map(lower).collect();
    (enter, leave)
}

/// Whether inline code, bytes this printer cannot read, may address the frame.
/// Not the string moves the selector writes itself (`push ds; pop es; std; rep
/// movsd; cld`, `add si, 3; add di, 3; mov cx, dx; rep movsb`): of the few
/// opcodes below, none has a memory operand, so a frame is not needed for them
/// (scroll.c: `push bp; mov bp, sp` on a function with no locals).
fn may_address_frame(parts: &[InlinePart]) -> bool {
    let mut bytes = Vec::new();
    for part in parts {
        match part {
            InlinePart::Bytes(more) => bytes.extend(more),
            _ => return true,
        }
    }
    let code = &bytes[..];
    let mut at = 0;
    while at < code.len() {
        match code[at] {
            0x1E | 0x07 | 0xFD | 0xFC | 0xF3 | 0x66 | 0xA4 | 0xA5 => at += 1,
            // `add r, imm8`, `mov r, r`: register operands only (mod = 11)
            0x83 if code.get(at + 1).is_some_and(|modrm| modrm >> 6 == 3) => at += 3,
            0x89 | 0x8B if code.get(at + 1).is_some_and(|modrm| modrm >> 6 == 3) => at += 2,
            _ => return true,
        }
    }
    false
}

/// The implicit entry and return sequences shared by text and OMF emission.
pub fn _frame_parts(procedure: &Procedure) -> (Vec<Semantics>, Vec<Semantics>) {
    parts(procedure, frame_omitted(procedure, 0, frame_held(procedure)).is_some())
}

/// `_frame_parts`, with no frame register where `omit`: the entry sets none and
/// the return takes back what the entry reserved.
fn parts(
    procedure: &Procedure,
    omit: bool,
) -> (Vec<Semantics>, Vec<Semantics>) {
    let roots = _roots(&procedure.body);
    let saved = saved_of(procedure);
    let slot = procedure.registers.slot;
    let reserve = (procedure.reserve + slot - 1) / slot * slot;
    // Where the frame register's own cell was stays reserved: a cell addressed
    // through an index has no sign to say whether it lies above that cell
    // or below it, so every one keeps its distance from it.
    // Inline code is bytes this printer cannot read, so it may address the
    // frame. A frame the runtime's entry built (B$ENRA, B$ENRD) is not
    // built again by a shell of ours.
    let framed_by = |reserve: i64| {
        !omit
            && procedure.entry == 0
            && (reserve != 0
                || roots.contains(&procedure.body.regs().root(procedure.registers.pointer))
                || procedure.callees.values().any(|one| may_address_frame(&one.code)))
    };
    let locals = reserve != 0;
    let held = |reserve: i64| if omit && locals && !procedure.body.slot_dropped { reserve + slot } else { reserve };
    // A call finds the stack a multiple of the target's alignment, as its
    // callee assumes (a double on its stack at an 8-aligned address): the frame
    // takes what keeps it so, the return address, the pushed registers, the
    // frame register and the reserve a multiple of it.
    let align = procedure.registers.align;
    let calls = align > slot
        && procedure.body.bits >= 32
        && !procedure.far
        && procedure.interrupt.is_none()
        && procedure.entry == 0
        && procedure
            .body
            .blocks
            .iter()
            .flat_map(|block| block.insns.iter())
            .any(|one| one.what.as_ref().is_some_and(|what| what.op == Operation::Call));
    let mut reserve = reserve;
    if calls {
        let below =
            |reserve: i64| slot + slot * saved.len() as i64 + if framed_by(reserve) { slot } else { 0 } + held(reserve);
        while below(reserve) % align != 0 {
            reserve += slot;
        }
    }
    let framed = framed_by(reserve);
    let reserve = held(reserve);
    let (bp, sp) = (reg(FRAME), reg(STACK));
    if procedure.body.saved_first {
        return lowered(procedure, &saved, reserve, framed, omit);
    }
    // Inline code that moves SP (`alloca`) leaves the saved registers below an
    // unknown distance: SP is set from the frame register first.
    let moved = !omit && framed && moves_stack(&procedure.body);
    // A one-slot adjustment of a 64-bit frame is `push rax` / `pop rcx` where
    // no return is a sibling call (rcx may carry its argument):
    // X86FrameLowering's emitSPUpdate, with the register
    // findDeadCallerSavedReg gives a `ret`.
    let scratch =
        procedure.registers.slot_scratch.filter(|_| omit && reserve == slot && !sibling_calls(&procedure.body));
    let one_slot = scratch.is_some();
    let (accumulator, dead_at_return) = scratch.map_or((STACK, None), |(push, pop)| (push, Some(pop)));
    let mut leave: Vec<Semantics> = Vec::new();
    if moved {
        let below = reserve + slot * saved.len() as i64;
        let from = ir::Mem { through: FRAME, offset: -below, disp_width: 2, ..ir::Mem::new(None, 2) };
        leave.push(semantics(Operation::Address, "lea", vec![sp_reg()], vec![Loc::Mem(from)]));
    }
    leave.extend(saved.iter().rev().map(|one| semantics(Operation::Pop, "pop", vec![reg(*one)], vec![])));
    if one_slot && let Some(dead) = dead_at_return {
        leave.push(semantics(Operation::Pop, "pop", vec![reg(dead)], vec![]));
    } else if omit && reserve != 0 {
        leave.push(semantics(
            Operation::Binary,
            "add",
            vec![sp_reg()],
            vec![sp_reg(), Loc::Imm(ir::Imm { value: reserve, width: 2, address: None })],
        ));
    } else if reserve != 0 && framed {
        leave.push(semantics(Operation::Nothing, "leave", vec![], vec![]));
    } else if framed {
        leave.push(semantics(Operation::Pop, "pop", vec![bp.clone()], vec![]));
    }
    // `enter` only where the target's description takes it for size
    // (`frame_enter`).
    let opened =
        framed && reserve != 0 && procedure.size && procedure.registers.enter && procedure.stack_check.is_none();
    let mut enter: Vec<Semantics> = Vec::new();
    if opened {
        let count = |value, width| Loc::Imm(ir::Imm { value, width, address: None });
        enter.push(semantics(Operation::Nothing, "enter", vec![], vec![count(reserve, 2), count(0, 1)]));
    } else if framed {
        enter.extend([
            semantics(Operation::Push, "push", vec![], vec![bp.clone()]),
            semantics(Operation::Move, "mov", vec![bp], vec![sp.clone()]),
        ]);
    }
    if one_slot {
        enter.push(semantics(Operation::Push, "push", vec![], vec![reg(accumulator)]));
    } else if reserve != 0 && !opened {
        enter.push(semantics(
            Operation::Binary,
            "sub",
            vec![sp.clone()],
            vec![sp, Loc::Imm(ir::Imm { value: reserve, width: 2, address: None })],
        ));
    }
    enter.extend(stack_check(procedure, reserve));
    enter.extend(saved.iter().map(|one| semantics(Operation::Push, "push", vec![], vec![reg(*one)])));
    if let Some(group) = procedure.interrupt {
        let (before, after) = _interrupt_parts(group);
        enter.splice(0..0, before);
        leave.extend(after);
    }
    (enter, leave)
}

/// Where the frame an interrupt handler saved starts above its own BP: the
/// last register it saved, past what its entry pushes after those and the BP
/// its frame pushes. the target's `interrupt_frame` (calling.toml) is the
/// frame.
pub fn interrupt_parameters() -> i64 {
    let (enter, _) = _interrupt_parts(Addr::new(Space::Group, 0));
    let pushed: i64 = enter[INTERRUPT_SAVED..]
        .iter()
        .map(|one| match (one.op, one.name.as_deref()) {
            (_, Some("pushad")) => 32,
            (Operation::Push, _) => 2,
            (Operation::Pop, _) => -2,
            _ => 0,
        })
        .sum();
    pushed + 2
}

/// The entry's first instructions, which save the frame: PUSHAD, then the
/// segments, so the last one pushed is the lowest slot.
const INTERRUPT_SAVED: usize = 5;

/// `StackProbeSize`: a frame of this many bytes or more is probed.
const STACK_PROBE_PAGE: u64 = 4096;

/// What an interrupt handler wraps its frame in. It may interrupt anything,
/// so it saves every register it or a callee may change once, as the frame
/// the target's `interrupt_frame` (calling.toml) lays out, and gives compiled
/// code what it assumes: DGROUP in DS and ES, the direction flag clear. A
/// handler's register parameters are slots of that frame, so what it writes to
/// them is what POPAD or `iret` goes back with. The x87 state is not saved.
fn _interrupt_parts(group: Addr) -> (Vec<Semantics>, Vec<Semantics>) {
    let push = |one: Loc| semantics(Operation::Push, "push", vec![], vec![one]);
    let pop = |one: RegId| semantics(Operation::Pop, "pop", vec![reg(one)], vec![]);
    let enter = vec![
        semantics(Operation::Nothing, "pushad", vec![], vec![]),
        push(reg(RegId::DS)),
        push(reg(RegId::ES)),
        push(reg(RegId::FS)),
        push(reg(RegId::GS)),
        push(Loc::Imm(ir::Imm { value: 0, width: 2, address: Some(group) })),
        pop(RegId::DS),
        push(reg(RegId::DS)),
        pop(RegId::ES),
        semantics(Operation::Nothing, "cld", vec![], vec![]),
    ];
    let leave = vec![
        pop(RegId::GS),
        pop(RegId::FS),
        pop(RegId::ES),
        pop(RegId::DS),
        semantics(Operation::Nothing, "popad", vec![], vec![]),
    ];
    (enter, leave)
}

/// Bytes emitted before each RETURN but absent from allocated LIR.
pub fn return_overhead_bytes(procedure: &Procedure) -> Result<usize, Unprintable> {
    let (_enter, leave) = _frame_parts(procedure);
    let emitted: Vec<Option<select::Emitted>> =
        leave.iter().map(|one| select::emit_in(procedure.body.bits, one, 0, None, false, false, None)).collect();
    if emitted.iter().any(Option::is_none) {
        return Err(Unprintable(format!("{}: implicit return sequence is not encodable", procedure.name)));
    }
    Ok(emitted.iter().flatten().map(|one| one.code.len()).sum())
}

/// The procedure as emitted, frame included: what this prints and objbuild
/// encodes. A branch's target is still a block; `label(number, at)` names it.
/// A value lives in the frame register the description frees: there is no frame
/// to fall back to.
fn frame_held(procedure: &Procedure) -> bool {
    let registers = &procedure.registers;
    registers.free
        && procedure
            .body
            .facts
            .0
            .get::<BodyRoots>(&procedure.body)
            .of_values(procedure.body.regs(), registers.pointer)
            .contains(&procedure.body.regs().root(registers.pointer))
}

pub fn listing(
    procedure: &Procedure,
    number: usize,
) -> Result<Vec<Item>, Unprintable> {
    let registers = &procedure.registers;
    let held = frame_held(procedure);
    if procedure.body.frame_exit == Some(false) {
        let registers = llrm_target::FrameRegisters { free: false, ..registers.clone() };
        return Ok(built(procedure, number, false)?.into_iter().map(|item| spelled(item, &registers)).collect());
    }
    let (items, registers) = match frame_omitted(procedure, number, held) {
        Some(items) => (items, registers.clone()),
        None if held => {
            return Err(Unprintable(format!(
                "{}: the frame register was given to a value and the frame cannot be addressed through the stack pointer",
                procedure.name
            )));
        }
        // Nothing holds the freed register: the frame is the register's again.
        None => (built(procedure, number, false)?, llrm_target::FrameRegisters { free: false, ..registers.clone() }),
    };
    Ok(items.into_iter().map(|item| spelled(item, &registers)).collect())
}

/// What a procedure sets up and takes back, and where: the frame and each
/// callee-saved register are set up at the entry, or where `shrinkwrap` put
/// them, and taken back on the returns that path reaches. A function of the
/// procedure and whether its frame register is omitted, so `built` prints it
/// and a phase that makes it instructions asks the same question.
struct Sequences {
    /// Where the pieces go when not at the entry.
    wrap: Option<crate::backend::shrinkwrap::Wrap>,
    /// What the entry sets up: every piece no block sets up later.
    entry: Vec<Semantics>,
    /// What each return takes back of what the entry set up, the registers
    /// last set up first, then the frame.
    exit: Vec<Semantics>,
    enter: Vec<Semantics>,
    leave: Vec<Semantics>,
    kept: Vec<Semantics>,
    restores: Vec<Semantics>,
    saved: Vec<RegId>,
    count: usize,
    first: bool,
}

impl Sequences {
    fn of(
        procedure: &Procedure,
        omit: bool,
    ) -> Self {
        let (enter, mut leave) = parts(procedure, omit);
        let mut enter = probed_entry(procedure, enter);
        // The registers a `saved_first` frame pushes are part of `enter`.
        let first = procedure.body.saved_first;
        let lows = if first { Vec::new() } else { saved_of(procedure) };
        let count = lows.len();
        // Decided before the exits became instructions, which name the saved
        // registers in every return.
        let wrap = if procedure.body.frame_exit.is_some() {
            None
        } else {
            wrap_of(procedure, &procedure.body, enter.len() > count, omit)
        };
        let kept = enter.split_off(enter.len() - count);
        let restores: Vec<Semantics> = leave.drain(..count).collect();
        let saved: Vec<RegId> =
            procedure.registers.saved.iter().filter(|(_, low)| lows.contains(low)).map(|(whole, _)| *whole).collect();
        let mut sequences =
            Self { wrap, entry: Vec::new(), exit: Vec::new(), enter, leave, kept, restores, saved, count, first };
        let wrapped = |sequences: &Self, piece: crate::backend::shrinkwrap::Piece| {
            sequences.wrap.as_ref().is_some_and(|wrap| wrap.wrapped.contains(&piece))
        };
        let mut entry_items: Vec<Semantics> = Vec::new();
        let mut entry_leave: Vec<Semantics> = Vec::new();
        if !wrapped(&sequences, crate::backend::shrinkwrap::Piece::Frame) {
            entry_items.extend(sequences.enter.iter().cloned());
        }
        for (at, whole) in sequences.saved.iter().enumerate() {
            if !wrapped(&sequences, crate::backend::shrinkwrap::Piece::Saved(*whole)) {
                entry_items.push(sequences.kept[at].clone());
            }
        }
        // Taken back in the reverse of that order.
        for (at, whole) in sequences.saved.iter().enumerate().rev() {
            if !wrapped(&sequences, crate::backend::shrinkwrap::Piece::Saved(*whole)) {
                entry_leave.push(sequences.restores[sequences.count - 1 - at].clone());
            }
        }
        if !wrapped(&sequences, crate::backend::shrinkwrap::Piece::Frame) {
            entry_leave.extend(sequences.leave.iter().cloned());
        }
        sequences.entry = entry_items;
        sequences.exit = if procedure.body.frame_exit.is_some() { Vec::new() } else { entry_leave };
        sequences
    }

    /// The items that set a piece up and take it back.
    fn piece(
        &self,
        piece: &crate::backend::shrinkwrap::Piece,
    ) -> (Vec<Semantics>, Vec<Semantics>) {
        match piece {
            crate::backend::shrinkwrap::Piece::Frame => (self.enter.clone(), self.leave.clone()),
            // A `saved_first` frame's registers are in the frame's own
            // sequence.
            crate::backend::shrinkwrap::Piece::Saved(_) if self.first => (Vec::new(), Vec::new()),
            crate::backend::shrinkwrap::Piece::Saved(whole) => {
                let at = self.saved.iter().position(|one| one == whole).expect("a kept register");
                // `kept` is in the order of `saved_of`; `restores` is the
                // reverse.
                (vec![self.kept[at].clone()], vec![self.restores[self.count - 1 - at].clone()])
            }
        }
    }
}

/// The procedure's items with the frame register LIR names BP, or not where
/// `omit`.
fn built(
    procedure: &Procedure,
    number: usize,
    omit: bool,
) -> Result<Vec<Item>, Unprintable> {
    let regs = procedure.body.regs();
    let sequences = Sequences::of(procedure, omit);
    let (wrap, enter, unwrapped_leave) = (&sequences.wrap, sequences.entry.clone(), sequences.exit.clone());
    let item = |piece: &crate::backend::shrinkwrap::Piece| sequences.piece(piece);
    let blocks = &procedure.body.blocks;
    let lined = || blocks.iter().flat_map(|block| &block.insns).filter(|one| one.line.is_some());
    let first = lined().next();
    let last = lined().filter(|one| one.what.as_ref().is_none_or(|what| what.op != Operation::Return)).last();
    // The prologue is the first line's, not the previous procedure's last.
    let mut line = first.and_then(|one| one.line);
    let mut lines = 0..;
    let mut marked = |line: SrcLoc| Item::Mark(Mark::Line { line, index: lines.next().expect("unbounded") });
    // The arguments are in their registers from the first byte: what the entry
    // says of them stands before the prologue.
    let arrival = blocks.iter().flat_map(|block| &block.insns).find(|one| one.arrival());
    let arrived: Vec<Item> =
        arrival
            .into_iter()
            .flat_map(|one| {
                let defined =
                    one.delivers.iter().zip(&one.debug.defines).filter(|(_, tag)| **tag != u32::MAX).map(
                        |((_, register), tag)| Item::Mark(Mark::Def { tag: *tag, place: Place::Register(*register) }),
                    );
                defined.chain(one.debug.before.iter().map(|&note| Item::Mark(Mark::Note(note)))).collect::<Vec<_>>()
            })
            .collect();
    let cells = blocks_arguments(&procedure.body);
    // Call frame information, declared as the entry and the exits are written:
    // only for a frame with no interrupt, `enter` or stack check, which is what
    // the rows below are right for.
    let plain = llrm_x86::mc::unwind::declares(procedure.body.bits)
        && procedure.interrupt.is_none()
        && procedure.entry == 0
        && !procedure.far
        && procedure.stack_check.is_none()
        && sequences.entry.iter().all(|one| one.name.as_deref() != Some("enter"));
    let bp = reg(FRAME);
    // The DWARF numbers the directives name, from the target's table.
    let dwarf = |register: RegId| {
        regs.name(register)
            .and_then(|name| llrm_x86::mc::unwind::register_in(procedure.body.bits, name))
            .map(|number| number as u16)
    };
    let numbers = dwarf(procedure.registers.pointer).zip(dwarf(procedure.registers.stack));
    let mut cfi = numbers
        .filter(|_| plain)
        .map(|(frame, stack)| CfiState::entry(procedure.registers.slot, stack, frame, procedure.registers.stack));
    let hardware = |loc: &Loc| match loc {
        Loc::Reg(one) => regs.name(one.register).and_then(llrm_x86::mc::win64unwind::hardware),
        _ => None,
    };
    let (mut entry_items, mut after_entry) = (Vec::new(), None);
    let wrapped_frame = cfi.is_some() && wrap.is_some();
    let mut entry_saves: Vec<(u16, i64)> = Vec::new();
    if let Some(cfi) = cfi.as_mut() {
        entry_items.push(Item::Mark(Mark::Declared));
        let mut pending = Vec::new();
        let mut size = None;
        for one in &enter {
            entry_items.push(entry_item(one, procedure.probe_stack.as_deref()));
            // The directives state the allocation the probe's `sub rsp, rax`
            // makes by the size that was moved into `eax`.
            let stated = probed(one, size);
            size = size_moved(one).or(size);
            let one = stated.as_ref().unwrap_or(one);
            let marks = cfi.after(one, &bp);
            entry_items.extend(seh_after(one, &bp, &hardware));
            // The System V callee-saved registers: rbx, rbp and r12 to r15
            // (ebx, ebp, esi, edi) (the frame register's own push
            // is told apart by its name).
            if one.op == Operation::Push
                && let Loc::Reg(pushed) = &one.sources[0]
                && let Some(register) = dwarf(pushed.register)
                    .filter(|number| llrm_x86::mc::unwind::callee_saved(procedure.body.bits).contains(number))
            {
                // A callee-saved register's own offset is declared after the
                // whole prologue (`emitCalleeSavedFrameMoves`).
                pending.push(Item::Cfi(Cfi::Offset { register, offset: -cfi.depth }));
                entry_items.extend(marks);
            } else {
                entry_items.extend(marks);
            }
        }
        // In the reverse of the order they were pushed, as the pin lists them.
        entry_saves = pending
            .iter()
            .filter_map(|one| match one {
                Item::Cfi(Cfi::Offset { register, offset }) => Some((*register, *offset)),
                _ => None,
            })
            .collect();
        entry_items.extend(pending.into_iter().rev());
        after_entry = Some(cfi.clone());
    } else {
        entry_items.extend(enter.iter().map(|one| entry_item(one, procedure.probe_stack.as_deref())));
    }
    let mut split_edges: Vec<(i64, Option<i64>, Vec<crate::backend::shrinkwrap::Piece>)> = Vec::new();
    let mut out: Vec<Item> = line.map(&mut marked).into_iter().chain(arrived).chain(cells).chain(entry_items).collect();
    let body_from = out.len();
    // An item or two per instruction, a label and an alignment per block, and
    // the pieces of the frame: growing it by doubling copied every item some
    // six times.
    out.reserve(blocks.iter().map(|block| 2 * block.insns.len() + 2).sum::<usize>());
    for (index, block) in blocks.iter().enumerate() {
        if block.align > 0 {
            out.push(Item::Align(CodeAlign { log2: block.align, max_skip: block.max_skip }));
        }
        out.push(Item::Label(Label { name: label(number, block.at) }));
        if let Some(pieces) = wrap.as_ref().and_then(|wrap| wrap.opens.get(&block.at)) {
            out.extend(pieces.iter().flat_map(|piece| item(piece).0).map(Item::Semantics));
            out.extend(wrapped_frame.then_some(Item::Mark(Mark::Opened)));
        }
        let following = if index + 1 < blocks.len() { Some(blocks[index + 1].at) } else { None };
        let fallthrough = _fallthrough_jump(block, following);
        // The edge into the restore that `shrinkwrap` split: what the path took
        // on is taken back at the end of this block, before the jump that
        // leaves it (printed or not), or where it falls into the restore. The
        // paths that took nothing reach the restore's label past it.
        let closing = wrap.as_ref().and_then(|wrap| wrap.closes_after.get(&block.at));
        let last_printed = block.insns.iter().rev().find(|one| prints(one));
        let taken = wrap.as_ref().and_then(|wrap| wrap.closes_taken.get(&block.at));
        let ends_in_jump =
            last_printed.is_some_and(|one| one.what.as_ref().is_some_and(|what| what.op == Operation::Jump));
        let mut closed = false;
        for one in &block.insns {
            if let Some(pieces) =
                closing.filter(|_| ends_in_jump && last_printed.is_some_and(|last| Arc::ptr_eq(last, one)))
            {
                out.extend(pieces.iter().flat_map(|piece| item(piece).1).map(Item::Semantics));
                out.extend(wrapped_frame.then_some(Item::Mark(Mark::Closed)));
                closed = true;
            }
            if first.is_some_and(|first| Arc::ptr_eq(first, one)) {
                out.push(Item::Mark(Mark::BodyStart));
            }
            // What is said before an instruction stands there whether or not it
            // is emitted: a jump to the next block is not.
            if !one.arrival() {
                out.extend(one.debug.before.iter().map(|&note| Item::Mark(Mark::Note(note))));
            }
            if fallthrough.is_some_and(|jump| Arc::ptr_eq(jump, one)) {
                if last.is_some_and(|last| Arc::ptr_eq(last, one)) {
                    out.push(Item::Mark(Mark::BodyEnd));
                }
                continue;
            }
            let Some(what) = &one.what else {
                return Err(Unprintable(format!("{} at {}: an instruction with no semantics", procedure.name, one.at)));
            };
            if one.line.is_some() && one.line != line {
                line = one.line;
                out.extend(line.map(&mut marked));
            }
            match what.op {
                Operation::Move if _segment(regs, &what.dests[0]) && matches!(what.sources[0], Loc::Imm(_)) => {
                    // x86 has no immediate move into a segment register; the
                    // stack holds it for one instruction.
                    let Loc::Imm(source) = &what.sources[0] else { unreachable!() };
                    out.extend([
                        Item::Semantics(semantics(
                            Operation::Push,
                            "push",
                            vec![],
                            vec![Loc::Imm(ir::Imm { width: 2, ..source.clone() })],
                        )),
                        Item::Semantics(semantics(Operation::Pop, "pop", what.dests.clone(), vec![])),
                    ]);
                }
                Operation::Call => {
                    if what.indirect {
                        let pops = procedure.body.call_pops.get(&one.at).copied();
                        out.push(Item::Semantics(Semantics { pops, ..what.clone() }));
                    } else {
                        let Some(callee) = procedure.callees.get(&one.at) else {
                            return Err(Unprintable(format!(
                                "{} at {}: a call with no callee",
                                procedure.name, one.at
                            )));
                        };
                        out.push(Item::Callee(callee.clone()));
                        // Said for call frame information, which `-g` writes:
                        // no code of its own.
                        if callee.pops != 0 && callee.code.is_empty() && (first.is_some() || after_entry.is_some()) {
                            out.push(Item::Mark(Mark::Pops(callee.pops)));
                        }
                        if first.is_some() {
                            out.extend(one.clobbers.iter().map(|&register| Item::Mark(Mark::Clobbered(register))));
                        }
                    }
                }
                Operation::Return => {
                    if let Some(pieces) = wrap.as_ref().and_then(|wrap| wrap.closes.get(&block.at)) {
                        out.extend(pieces.iter().flat_map(|piece| item(piece).1).map(Item::Semantics));
                        out.extend(wrapped_frame.then_some(Item::Mark(Mark::Closed)));
                    }
                    out.extend(unwrapped_leave.iter().cloned().map(Item::Semantics));
                    // an indirect sibling call: the epilogue, then a jump
                    // through the register the callee's
                    // address is in
                    if what.is_sibling_call() && what.indirect {
                        out.push(Item::Semantics(Semantics { op: Operation::Jump, ..what.clone() }));
                        continue;
                    }
                    // a sibling call: the epilogue, then a jump to the callee
                    if what.is_sibling_call()
                        && let Some(callee) = procedure.callees.get(&one.at)
                    {
                        out.push(Item::Callee(Callee { tail: true, ..callee.clone() }));
                        continue;
                    }
                    if let Some(Loc::Imm(popped)) =
                        what.sources.first().filter(|_| procedure.interrupt.is_none() && !procedure.far)
                    {
                        if popped.value > llrm_x86::calling::RET_POPS_MOST {
                            out.extend(
                                moved_return(
                                    popped.value,
                                    procedure.registers.slot,
                                    procedure.registers.spelled(ir::STACK),
                                )
                                .into_iter()
                                .map(Item::Semantics),
                            );
                            out.push(Item::Semantics(Semantics {
                                name: Some("ret".into()),
                                sources: vec![],
                                ..what.clone()
                            }));
                            continue;
                        }
                    }
                    let name = match what.name.as_deref() {
                        _ if procedure.interrupt.is_some() => "iret".to_owned(),
                        Some(name) if !name.is_empty() => name.to_owned(),
                        _ => (if procedure.far { "retf" } else { "ret" }).to_owned(),
                    };
                    out.push(Item::Semantics(Semantics { name: Some(name.into()), ..what.clone() }));
                }
                Operation::Branch if taken.is_some_and(|_| last_printed.is_some_and(|last| Arc::ptr_eq(last, one))) => {
                    // The taken edge into the restore goes through a block of
                    // its own, past the procedure's last, that takes the
                    // pieces back (`tryToSplitRestore`).
                    let edge = cold_at(&procedure.body) + 1 + split_edges.len() as i64;
                    split_edges.push((edge, what.target, taken.cloned().unwrap_or_default()));
                    out.push(Item::Semantics(Semantics { target: Some(edge), ..what.clone() }));
                }
                _ => out.push(Item::Semantics(what.clone())),
            }
            // Where the values it made went, for a debugger: the register or
            // frame cell it wrote.
            if let Some(place) = what.dests.first().and_then(placed) {
                out.extend(one.debug.defines.iter().map(|&tag| Item::Mark(Mark::Def { tag, place })));
            } else {
                // A call's result comes back in the register it says it
                // delivers.
                out.extend(one.delivers.iter().filter(|(held, _)| one.debug.defines.contains(&held.value)).map(
                    |(held, register)| Item::Mark(Mark::Def { tag: held.value, place: Place::Register(*register) }),
                ));
            }
            if last.is_some_and(|last| Arc::ptr_eq(last, one)) {
                out.push(Item::Mark(Mark::BodyEnd));
            }
        }
        if let Some(pieces) = closing.filter(|_| !closed) {
            out.extend(pieces.iter().flat_map(|piece| item(piece).1).map(Item::Semantics));
            out.extend(wrapped_frame.then_some(Item::Mark(Mark::Closed)));
        }
        let fall = _falls_to(block, &procedure.name)?;
        if let Some(fall) = fall {
            if index + 1 == blocks.len() || blocks[index + 1].at != fall {
                out.push(Item::Semantics(Semantics {
                    target: Some(fall),
                    ..semantics(Operation::Jump, "jmp", vec![], vec![])
                }));
            }
        }
    }
    for (edge, restore, pieces) in split_edges {
        out.push(Item::Label(Label { name: label(number, edge) }));
        out.extend(pieces.iter().flat_map(|piece| item(piece).1).map(Item::Semantics));
        out.extend(wrapped_frame.then_some(Item::Mark(Mark::Closed)));
        out.push(Item::Semantics(Semantics { target: restore, ..semantics(Operation::Jump, "jmp", vec![], vec![]) }));
    }
    // Last, where nothing falls into it: the call that does not return.
    if let Some(check) = &procedure.stack_check {
        out.push(Item::Label(Label { name: label(number, cold_at(&procedure.body)) }));
        out.push(Item::Callee(Callee::new(check.handler.clone(), check.far)));
    }
    if realign_of(&procedure.body) != 0 {
        // The cells lie below the aligned top the `and` makes, `bytes` above
        // the stack pointer once the entry has taken them: addressed through
        // it, not the frame register, which the arguments keep.
        let entry = &sequences.entry;
        let bytes = (0..entry.len().saturating_sub(1))
            .find(|&at| entry[at].name.as_deref() == Some("and"))
            .and_then(|at| match entry[at + 1].sources.get(1) {
                Some(Loc::Imm(imm)) if entry[at + 1].name.as_deref() == Some("sub") => Some(imm.value),
                _ => None,
            })
            .ok_or_else(|| Unprintable(format!("{}: a realigned frame with no stack adjustment", procedure.name)))?;
        // What the body has pushed since the entry (a call's arguments) lies
        // between the stack pointer and the top: a block's pushes are taken
        // back before it ends.
        let kept = |why: &str| Unprintable(format!("{}: a realigned frame {why}", procedure.name));
        let (mut pushed, mut leaving) = (0, false);
        for item in out[body_from..].iter_mut() {
            match item {
                Item::Label(_) | Item::Align(_) if pushed != 0 => return Err(kept("with pushes across a block")),
                Item::Label(_) => leaving = false,
                Item::Callee(callee) => pushed -= callee.pops,
                Item::Semantics(what) => {
                    for place in what.dests.iter_mut().chain(what.sources.iter_mut()) {
                        if let Some(placed) = below_top(place, bytes + pushed) {
                            *place = placed;
                        }
                    }
                    match what.op {
                        // The stack pointer taken from the frame register: what
                        // follows is the epilogue's pops.
                        _ if what.dests.iter().any(writes_stack_pointer) && stack_adjustment(what).is_none() => {
                            (pushed, leaving) = (0, true);
                        }
                        // A push is a slot in 64-bit code, whatever width the
                        // immediate is spelled with.
                        Operation::Push if !leaving => pushed += procedure.registers.slot,
                        Operation::Pop if !leaving => pushed -= procedure.registers.slot,
                        Operation::Call if what.indirect => pushed -= what.pops.unwrap_or(0),
                        _ if stack_adjustment(what).is_some() => pushed += stack_adjustment(what).unwrap_or(0),
                        Operation::Return | Operation::Jump | Operation::Branch if pushed != 0 => {
                            return Err(kept(&format!("with {pushed} bytes pushed at {what:?}")));
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }
    // The stack moves wherever the code moves it, whether the sequences or the
    // body wrote the instruction; what follows a return or a jump is the
    // frame the prologue left.
    if let (Some(done), true) = (after_entry.clone(), wrapped_frame) {
        let tail = out.split_off(body_from);
        out.extend(declare_wrapped(tail, done, entry_saves, number, &bp, &|pushed| {
            dwarf(pushed).filter(|number| llrm_x86::mc::unwind::callee_saved(procedure.body.bits).contains(number))
        }));
    } else if let Some(done) = after_entry {
        let tail = out.split_off(body_from);
        let mut cfi = done.clone();
        let last_label = tail.iter().rposition(|item| matches!(item, Item::Label(_)));
        for (at, item) in tail.into_iter().enumerate() {
            let ends = match &item {
                Item::Semantics(what) => {
                    let marks = cfi.after(what, &bp);
                    let ends = matches!(what.op, Operation::Return | Operation::Jump);
                    out.push(item);
                    out.extend(marks);
                    ends
                }
                Item::Callee(Callee { tail: true, .. }) => {
                    out.push(item);
                    true
                }
                Item::Mark(Mark::Pops(bytes)) => {
                    let marks = cfi.after_pops(*bytes);
                    out.push(item);
                    out.extend(marks);
                    false
                }
                _ => {
                    out.push(item);
                    false
                }
            };
            if ends {
                cfi = done.clone();
                // Nothing in this function follows its last exit: a directive
                // there would open the next function's frame.
                if last_label.is_some_and(|label| label > at) {
                    out.push(done.restate());
                }
            }
        }
    }
    Ok(out)
}

/// The frame as call frame information sees it at a point of the code: the
/// frame address and the registers saved.
#[derive(Clone, Debug)]
struct Unwound {
    cfa: CfiState,
    saves: Vec<(u16, i64)>,
}

impl Unwound {
    fn same(
        &self,
        other: &Unwound,
    ) -> bool {
        let sorted = |one: &Unwound| {
            let mut saves = one.saves.clone();
            saves.sort_unstable();
            saves
        };
        (self.cfa.framed, self.cfa.depth, self.cfa.frame) == (other.cfa.framed, other.cfa.depth, other.cfa.frame)
            && sorted(self) == sorted(other)
    }

    /// What turns this state into `want` at the start of a block
    /// (`CFIInstrInserter::insertCFIInstrs`): the frame address, then the
    /// saves.
    fn becomes(
        &self,
        want: &Unwound,
    ) -> Vec<Item> {
        let mut out = Vec::new();
        if (self.cfa.framed, self.cfa.depth, self.cfa.frame) != (want.cfa.framed, want.cfa.depth, want.cfa.frame) {
            out.push(Item::Cfi(if want.cfa.framed {
                Cfi::DefCfa { register: want.cfa.bp, offset: want.cfa.frame }
            } else if self.cfa.framed {
                Cfi::DefCfa { register: want.cfa.sp, offset: want.cfa.depth }
            } else {
                Cfi::DefCfaOffset { offset: want.cfa.depth }
            }));
        }
        for &(register, offset) in want.saves.iter().rev() {
            if !self.saves.contains(&(register, offset)) {
                out.push(Item::Cfi(Cfi::Offset { register, offset }));
            }
        }
        for &(register, _) in self.saves.iter().rev() {
            if !want.saves.iter().any(|(one, _)| *one == register) {
                out.push(Item::Cfi(Cfi::Restore { register }));
            }
        }
        out
    }
}

/// The call frame information of a shrink-wrapped frame, whose set-up and
/// take-down lie inside the body: the directives follow the instructions that
/// move the frame, and a block that starts in another state than the code
/// before it ends in says how it changes (`CFIInstrInserter`). `entry` is the
/// state the unwrapped part of the prologue left; `label` names a block.
fn declare_wrapped(
    tail: Vec<Item>,
    entry: CfiState,
    saves: Vec<(u16, i64)>,
    number: usize,
    bp: &Loc,
    callee_saved: &dyn Fn(RegId) -> Option<u16>,
) -> Vec<Item> {
    let mut cur = Unwound { cfa: entry, saves };
    let mut incoming: std::collections::HashMap<String, Unwound> = std::collections::HashMap::new();
    let mut pending: Vec<(u16, i64)> = Vec::new();
    let mut ended = false;
    let last_label = tail.iter().rposition(|item| matches!(item, Item::Label(_)));
    let mut out = Vec::with_capacity(tail.len() + 16);
    for (at, item) in tail.into_iter().enumerate() {
        match &item {
            Item::Label(Label { name }) => {
                if !ended {
                    incoming.entry(name.clone()).or_insert_with(|| cur.clone());
                }
                out.push(item.clone());
                if let Some(want) = incoming.get(name).filter(|want| !cur.same(want)) {
                    out.extend(cur.becomes(want));
                    cur = want.clone();
                }
                ended = false;
            }
            Item::Semantics(what) => {
                let marks = cur.cfa.after(what, bp);
                if what.op == Operation::Push
                    && let Some(Loc::Reg(pushed)) = what.sources.first()
                    && let Some(register) = callee_saved(pushed.register)
                {
                    pending.push((register, -cur.cfa.depth));
                }
                if let Some(target) = what.target.filter(|_| matches!(what.op, Operation::Branch | Operation::Jump)) {
                    incoming.entry(label(number, target)).or_insert_with(|| cur.clone());
                }
                ended = matches!(what.op, Operation::Return | Operation::Jump);
                out.push(item.clone());
                out.extend(marks);
            }
            Item::Callee(Callee { tail: true, .. }) => {
                ended = true;
                out.push(item.clone());
            }
            Item::Mark(Mark::Pops(bytes)) => {
                out.push(item.clone());
                out.extend(cur.cfa.after_pops(*bytes));
            }
            Item::Mark(Mark::Opened) => {
                // A callee-saved register's own offset is declared after the
                // whole set-up, in the reverse of the order they were pushed.
                for &(register, offset) in pending.iter().rev() {
                    out.push(Item::Cfi(Cfi::Offset { register, offset }));
                    cur.saves.push((register, offset));
                }
                pending.clear();
                out.push(item.clone());
            }
            Item::Mark(Mark::Closed) => {
                // The code after a take-down that other blocks follow is the
                // frame's absence: the registers are the caller's again.
                if last_label.is_some_and(|label| label > at) {
                    for &(register, _) in cur.saves.iter().rev() {
                        out.push(Item::Cfi(Cfi::Restore { register }));
                    }
                    cur.saves.clear();
                }
                out.push(item.clone());
            }
            _ => out.push(item.clone()),
        }
    }
    out
}

/// The bytes `what` takes off the stack pointer (`sub sp, n`; `add sp, n` is
/// negative), if it is just that.
fn stack_adjustment(what: &Semantics) -> Option<i64> {
    let ([Loc::Reg(dest)], [Loc::Reg(source), Loc::Imm(ir::Imm { value, address: None, .. })]) =
        (&what.dests[..], &what.sources[..])
    else {
        return None;
    };
    if ir::root(dest.register) != ir::root(STACK) || ir::root(source.register) != ir::root(STACK) {
        return None;
    }
    match what.name.as_deref() {
        Some("sub") => Some(*value),
        Some("add") => Some(-*value),
        _ => None,
    }
}

/// `place`, a frame cell below the frame register, as `bytes` above the stack
/// pointer from the aligned top the frame's `and` made.
fn below_top(
    place: &Loc,
    bytes: i64,
) -> Option<Loc> {
    let at = place.address()?;
    let addr = at.addr.filter(|addr| addr.space == Space::Frame && addr.disp < 0)?;
    Some(place.map_address(|one| ir::AddressRef {
        through: STACK,
        addr: Some(Addr { disp: addr.disp + bytes, ..addr }),
        disp_width: 0,
        ..one
    }))
}

/// `stack_addressed`, but tuned for size only where it is no longer: `[esp+d]`
/// is a byte longer than `[ebp+d]`, and a displacement past 127 three more,
/// which a large frame's cells can cost more than the entry and return save.
fn frame_omitted(
    procedure: &Procedure,
    number: usize,
    held: bool,
) -> Option<Vec<Item>> {
    let omitted = stack_addressed(procedure, number)?;
    // The exits were made instructions for an omitted frame (`exits_inserted`):
    // the size comparison below, asked again of the body that now holds them,
    // may answer the other way and print the entry with a frame register.
    if procedure.body.frame_exit == Some(true) {
        return Some(omitted);
    }
    // Where allocation freed the frame register the pin's `hasFP` said no, and
    // the pin omits it whatever the bytes (BASIC, which never asks, compares).
    if !procedure.size || held || procedure.registers.free {
        return Some(omitted);
    }
    let bytes = |items: &[Item]| -> usize {
        items
            .iter()
            .filter_map(|item| match spelled(item.clone(), &procedure.registers) {
                Item::Semantics(what) => {
                    select::emit_in(procedure.body.bits, &what, 0, None, false, false, None).map(|one| one.code.len())
                }
                _ => None,
            })
            .sum()
    };
    let framed = built(procedure, number, false).ok()?;
    (bytes(&omitted) <= bytes(&framed)).then_some(omitted)
}

/// What removes `bytes` of arguments (`address` bytes of return address on top)
/// where `ret imm16` cannot: the return address is popped into the last slot of
/// the area, the stack raised to it, and a plain `ret` follows. LLVM's `pop
/// ecx; add esp, n; push ecx; ret` (X86ExpandPseudo, the RET pseudo) holds the
/// address in ECX, which Open Watcom's register convention keeps for its
/// caller; the stack alone is as long and clobbers nothing.
fn moved_return(
    bytes: i64,
    address: i64,
    stack: RegId,
) -> Vec<Semantics> {
    let up = bytes - address;
    let above = ir::Mem { through: stack, offset: up, disp_width: 4, ..ir::Mem::new(None, address as u32) };
    let sp = Loc::Reg(ir::Reg { register: stack, width: address as u32 });
    let raise = Loc::Imm(ir::Imm { value: up, width: 4, address: None });
    vec![
        semantics(Operation::Pop, "pop", vec![Loc::Mem(above)], vec![]),
        semantics(Operation::Binary, "add", vec![sp.clone()], vec![sp, raise]),
    ]
}

/// Whether a callee's inline code leaves the stack and the frame alone: none,
/// or bytes that are only x87 register operations (`fsin`).
fn inert(callee: &Callee) -> bool {
    callee
        .code
        .iter()
        .all(
            |part| matches!(
                part,
                InlinePart::Bytes(bytes) if crate::backend::framefree::x87_registers_only(bytes)
            ),
        )
}

/// The items of a procedure that needs no frame register, its cells addressed
/// through the stack pointer: gcc's `-fomit-frame-pointer`, as the target's
/// `frame_optional` allows. A cell the frame register held at `[bp+d]` is at
/// `[sp+d+depth-slot]`, `depth` the bytes pushed since the entry, which
/// each instruction's stack effect moves and every way into a block must agree
/// on. Where anything puts that out of reach (a stack pointer written another
/// way, inline code, an indirect call whose callee may pop, a frame register
/// read as a value) the frame stays.
fn stack_addressed(
    procedure: &Procedure,
    number: usize,
) -> Option<Vec<Item>> {
    let registers = &procedure.registers;
    if !registers.optional
        || procedure.far
        || procedure.interrupt.is_some()
        || procedure.entry != 0
        || procedure.body.bits < 32
        || (!procedure.body.variables.is_empty() && !procedure.body.cfa_variables)
        || realign_of(&procedure.body) != 0
        || procedure.callees.values().any(|one| !inert(one))
    {
        llrm_support::debug!(
            "frame",
            "{}: kept: far {} interrupt {} entry {} bits {} variables {} inline {}",
            procedure.name,
            procedure.far,
            procedure.interrupt.is_some(),
            procedure.entry,
            procedure.body.bits,
            procedure.body.variables.len(),
            procedure.callees.values().filter(|one| !inert(one)).count()
        );
        return None;
    }
    let items = match built(procedure, number, true) {
        Ok(items) => items,
        Err(error) => {
            llrm_support::debug!("frame", "{}: kept: {}", procedure.name, error.0);
            return None;
        }
    };
    // The depth at each label, from whichever edge reached it first. A label
    // only a later branch reaches (a loop entered at its test) has its
    // depth from that branch: the pass is made again with what the last one
    // learned.
    let mut known: IndexMap<String, i64> = IndexMap::default();
    let mut dead = false;
    // A block's code runs into the next label only if the next block is among
    // its successors: a call that never returns leaves none, and a jump leaves
    // another. `built` lists the blocks in the order of `body.blocks`, so the
    // block after a block in the listing is the next of that list.
    let after_dead_end: BTreeSet<String> = procedure
        .body
        .blocks
        .windows(2)
        .filter(|pair| !pair[0].succ.contains(&pair[1].at))
        .map(|pair| label(number, pair[1].at))
        .collect();
    // The passes that only learn the depths change nothing; the one that
    // settles them is made again and rewrites the items it owns, in place
    // (a pass that built a copy each time cloned every instruction once per
    // pass: 3 M of the 15.5 M the emitter cost on d_faces).
    let mut items = items;
    loop {
        let before = known.len();
        match addressed_pass(procedure, number, &mut items, &mut known, dead, &after_dead_end, false) {
            // A label taken for dead that a later branch reached was addressed
            // at the wrong depth: again, with what that branch taught.
            Ok(assumed) if assumed.iter().all(|one| !known.contains_key(one)) => {
                return addressed_pass(procedure, number, &mut items, &mut known, dead, &after_dead_end, true)
                    .ok()
                    .map(|_| items);
            }
            Ok(_) => {}
            Err(Stop::Refused) => return None,
            // What nothing reaches is dead code: no depth is wrong for it, and
            // its jumps say nothing of the live labels.
            Err(Stop::Stalled(_)) if known.len() == before && !dead => dead = true,
            Err(Stop::Stalled(label)) if known.len() == before => {
                return refused(procedure, &format!("label {label} follows a jump and none reaches it"));
            }
            Err(Stop::Stalled(_)) => {}
        }
    }
}

enum Stop {
    Refused,
    /// A label no branch seen so far reaches.
    Stalled(String),
}

fn stop(
    procedure: &Procedure,
    why: &str,
) -> Stop {
    llrm_support::debug!("frame", "{}: kept: {why} [{}]", procedure.name, procedure.body.name);
    Stop::Refused
}

/// One pass over `items` with the cells addressed through the stack pointer,
/// given the depths `known` at the labels.
fn addressed_pass(
    procedure: &Procedure,
    number: usize,
    items: &mut [Item],
    known: &mut IndexMap<String, i64>,
    dead_ok: bool,
    after_dead_end: &BTreeSet<String>,
    build: bool,
) -> Result<Vec<String>, Stop> {
    let registers = &procedure.registers;
    let slot = registers.slot;
    let mut depth: Option<i64> = Some(0);
    let mut in_dead = false;
    // The size the entry moved into a register for the stack probe.
    let mut moved: Option<i64> = None;
    let mut assumed: Vec<String> = Vec::new();
    let name_of = |target: i64| label(number, target);
    for item in items.iter_mut() {
        match item {
            Item::Align(_) | Item::Cfi(_) | Item::Seh(_) => {}
            Item::Label(label) => {
                if after_dead_end.contains(&label.name) {
                    depth = None;
                }
                match (depth, known.get(&label.name).copied()) {
                    (Some(_), Some(there)) if in_dead => {
                        depth = Some(there);
                        in_dead = false;
                    }
                    (Some(here), Some(there)) if here != there => {
                        return Err(stop(
                            procedure,
                            &format!("label {} reached at depth {here} and {there}", label.name),
                        ));
                    }
                    // Entered by a branch seen: the depth is the real one from
                    // here, and what follows teaches the labels it reaches.
                    (None, Some(there)) => {
                        depth = Some(there);
                        in_dead = false;
                    }
                    (None, None) if dead_ok => {
                        depth = Some(0);
                        in_dead = true;
                        assumed.push(label.name.clone());
                    }
                    (None, None) => return Err(Stop::Stalled(label.name.clone())),
                    // Passed as dead and not yet reached: a later branch may
                    // reach it, and then the pass is made again.
                    (Some(_), None) if in_dead => assumed.push(label.name.clone()),
                    _ => {}
                }
                if !in_dead {
                    known.insert(label.name.clone(), depth.ok_or(Stop::Refused)?);
                }
            }
            Item::Callee(callee) => {
                depth = Some(depth.ok_or(Stop::Refused)? - callee.pops);
            }
            Item::Mark(_) => {}
            Item::Semantics(what) => {
                let here = depth.ok_or(Stop::Refused)?;
                moved = size_moved(what).or(moved);
                // `pop` to memory addresses the cell with the stack already
                // taken back.
                let at = match (what.op, &what.dests[..]) {
                    (Operation::Pop, [place @ Loc::Mem(_)]) => here - width_of(place).unwrap_or(0),
                    _ => here,
                };
                // Every operand is checked; only the pass that builds rewrites
                // its places, in the item it owns.
                for place in what.dests.iter_mut().chain(what.sources.iter_mut()) {
                    let replacement = match through_stack(place, at, slot, registers.pointer, registers.free) {
                        Some(std::borrow::Cow::Owned(placed)) => Some(placed),
                        Some(std::borrow::Cow::Borrowed(_)) => None,
                        None => {
                            return Err(stop(
                                procedure,
                                &format!("the frame register is read as a value in {place:?}"),
                            ));
                        }
                    };
                    if let (true, Some(placed)) = (build, replacement) {
                        *place = placed;
                    }
                }
                match what.op {
                    Operation::Push | Operation::Pop => {
                        // A push or pop moves the stack by the width it names:
                        // a 16-bit `pop cx` takes back two bytes.
                        let (places, sign) =
                            if what.op == Operation::Push { (&what.sources, 1) } else { (&what.dests, -1) };
                        let [place] = &places[..] else { return Err(stop(procedure, &format!("{what:?}"))) };
                        match width_of(place) {
                            Some(bytes @ (2 | 4 | 8)) => depth = Some(here + sign * bytes),
                            _ => return Err(stop(procedure, &format!("{what:?}"))),
                        }
                    }
                    // An exchange of x87 registers or of general ones (not the
                    // stack pointer) and the x87 and port instructions that
                    // name no stack effect leave the stack pointer alone.
                    Operation::Exchange
                        if what.dests.iter().chain(&what.sources).all(|place| {
                            (place.st_index().is_some() || matches!(place, Loc::Mem(_)))
                                || matches!(
                                    place,
                                    Loc::Reg(reg) if procedure.body.regs().root(reg.register) != procedure.body.regs().root(registers.stack)
                                )
                        }) => {}
                    Operation::Barrier
                        if matches!(
                            what.name.as_deref(),
                            Some("fnstcw" | "fldcw" | "fnstsw" | "in" | "out")
                        ) => {}
                    // An indirect call whose frame named the bytes its callee
                    // pops (`callframe`).
                    Operation::Call if what.indirect && what.pops.is_some() => {
                        depth = Some(here - what.pops.unwrap_or(0));
                    }
                    Operation::Leave
                    | Operation::Exchange
                    | Operation::Escape
                    | Operation::Barrier
                    | Operation::Call => return Err(stop(procedure, &format!("{what:?}"))),
                    // `pushf` and its kind move the stack and carry no operand
                    // to read the amount from.
                    Operation::Nothing
                        if what
                            .name
                            .as_deref()
                            .is_some_and(|name| name.starts_with("push") || name.starts_with("pop")) =>
                    {
                        return Err(stop(procedure, &format!("{what:?}")));
                    }
                    Operation::Return => {
                        if here != 0 && !in_dead {
                            return Err(stop(procedure, &format!("a return at depth {here}")));
                        }
                        depth = None;
                    }
                    Operation::Jump | Operation::Branch => {
                        // A jump table's targets are all the labels it names.
                        if what.indirect && what.table.is_empty() {
                            return Err(stop(procedure, "an indirect jump"));
                        }
                        let targets: Vec<i64> =
                            if what.indirect { what.table.clone() } else { vec![what.target.ok_or(Stop::Refused)?] };
                        for target in targets {
                            let target = name_of(target);
                            match known.get(&target) {
                                _ if in_dead => {}
                                Some(&there) if there != here => {
                                    return Err(stop(
                                        procedure,
                                        &format!("a jump to {target} at depth {here} and {there}"),
                                    ));
                                }
                                Some(_) => {}
                                None => {
                                    known.insert(target, here);
                                }
                            }
                        }
                        if what.op == Operation::Jump {
                            depth = None;
                        }
                    }
                    _ if what.dests.iter().any(writes_stack_pointer) => {
                        // Only `add sp, n` and `sub sp, n` are understood.
                        let amount = match (&what.dests[..], &what.sources[..]) {
                            ([Loc::Reg(dest)], [Loc::Reg(source), Loc::Imm(ir::Imm { value, address: None, .. })])
                                if ir::root(dest.register) == ir::root(STACK) && ir::root(source.register) == ir::root(STACK) =>
                            {
                                *value
                            }
                            // The probe's `sub rsp, rax` takes what was moved
                            // into `eax`.
                            ([Loc::Reg(dest)], [Loc::Reg(source), Loc::Reg(_)])
                                if ir::root(dest.register) == ir::root(STACK) && ir::root(source.register) == ir::root(STACK) =>
                            {
                                moved.ok_or_else(|| stop(procedure, &format!("the stack pointer written by {what:?}")))?
                            }
                            _ => return Err(stop(procedure, &format!("the stack pointer written by {what:?}"))),
                        };
                        depth = Some(match what.name.as_deref() {
                            Some("add") => here - amount,
                            Some("sub") => here + amount,
                            _ => return Err(stop(procedure, &format!("the stack pointer written by {what:?}"))),
                        });
                    }
                    _ => {}
                }
            }
        }
    }
    Ok(assumed)
}

/// Why `procedure` keeps its frame register, for `LLRM_DEBUG=frame`.
fn refused<T>(
    procedure: &Procedure,
    why: &str,
) -> Option<T> {
    llrm_support::debug!("frame", "{}: kept: {why} [{}]", procedure.name, procedure.body.name);
    None
}

/// The width of a register or memory operand, where it has one.
fn width_of(place: &Loc) -> Option<i64> {
    match place {
        // A register is as wide as it is: its own `width` is where it was
        // selected.
        Loc::Reg(one) => Some(one.register.size() as i64),
        Loc::Imm(one) => Some(i64::from(one.width)),
        Loc::Mem(one) => Some(i64::from(one.width)),
        _ => None,
    }
}

fn writes_stack_pointer(place: &Loc) -> bool {
    matches!(
        place,
        Loc::Reg(one) if ir::root(one.register) == ir::root(STACK)
    )
}

/// `place` with a frame cell addressed through the stack pointer; none where it
/// reads the frame register as anything but a base. The frame register would
/// have held the entry's stack pointer less `slot`. A frame cell is
/// `Space::Frame`, indexed or not; a register of the frame register's name in
/// any other address is a value (`FrameRegisters::free`).
fn through_stack<'a>(
    place: &'a Loc,
    depth: i64,
    slot: i64,
    pointer: RegId,
    free: bool,
) -> Option<std::borrow::Cow<'a, Loc>> {
    let shift = |_disp: i64| depth - slot;
    let is_pointer = |one: RegId| (one == FRAME || one == pointer) && !(free && one != RegId::None);
    if matches!(place, Loc::Reg(one) if is_pointer(one.register)) {
        return None;
    }
    let Some(at) = place.address() else { return Some(std::borrow::Cow::Borrowed(place)) };
    if is_pointer(at.index_through) {
        return None;
    }
    if let Some(addr) = at.addr.filter(|addr| addr.space == Space::Frame) {
        return Some(std::borrow::Cow::Owned(place.map_address(|one| ir::AddressRef {
            through: STACK,
            addr: Some(Addr { disp: addr.disp + shift(addr.disp), ..addr }),
            disp_width: 0,
            ..one
        })));
    }
    (!is_pointer(at.through)).then(|| std::borrow::Cow::Borrowed(place))
}

/// `item` with the frame register and stack pointer LIR calls BP and SP as the
/// target has them, which is BP and SP where it is real mode.
fn spelled(
    item: Item,
    registers: &llrm_target::FrameRegisters,
) -> Item {
    let register = |one: RegId| registers.spelled(one);
    // A frame place with no register is addressed through the frame register.
    let framed = |through: RegId, addr: Option<Addr>, index: RegId| {
        if through == RegId::None && index == RegId::None && addr.is_some_and(|addr| addr.space == Space::Frame) {
            registers.pointer
        } else {
            register(through)
        }
    };
    let place = |loc: &Loc| match loc {
        Loc::Reg(one) if register(one.register) != one.register => {
            let spelled = register(one.register);
            Loc::Reg(ir::Reg { register: spelled, width: spelled.size() as u32 })
        }
        other => other.map_address(|at| ir::AddressRef {
            through: framed(at.through, at.addr, at.index_through),
            index_through: register(at.index_through),
            ..at
        }),
    };
    // Where the target spells the registers as LIR does and no frame place is
    // addressed bare, nothing changes: the operands are not rebuilt.
    let changes = |loc: &Loc| {
        let address = match loc {
            Loc::Reg(one) => return register(one.register) != one.register,
            Loc::Mem(cell) => cell.address(),
            Loc::Address(at) => *at,
            _ => return false,
        };
        framed(address.through, address.addr, address.index_through) != address.through
            || register(address.index_through) != address.index_through
    };
    if let Item::Semantics(what) = &item
        && !what.dests.iter().chain(&what.sources).any(changes)
    {
        return item;
    }
    match item {
        Item::Semantics(what) => Item::Semantics(Semantics {
            dests: what.dests.iter().map(place).collect(),
            sources: what.sources.iter().map(place).collect(),
            ..what
        }),
        other => other,
    }
}

/// The explicit edge that physical adjacency makes free, if there is one.
///
/// Frontends may keep every CFG edge explicit through allocation. Listing is
/// where block order becomes physical, and therefore the first common layer
/// that can say an unconditional edge reaches the instruction already next.
/// Whether an instruction prints: a NOTHING anchor owns a source location
/// but emits no bytes.
pub fn prints(one: &lir::Insn) -> bool {
    one.what
        .as_ref()
        .is_none_or(|what| what.op != Operation::Nothing || !matches!(what.name.as_deref().unwrap_or(""), "" | "nop"))
}

/// NOTHING anchors after the edge own source locations but emit no bytes, so
/// the last instruction that actually prints is the one that matters.
pub fn _fallthrough_jump(
    block: &lir::LirBlock,
    following: Option<i64>,
) -> Option<&Arc<lir::Insn>> {
    let following = following?;
    if block.succ != [following] {
        return None;
    }
    let last = block.insns.iter().rev().find(|one| prints(one))?;
    let what = last.what.as_ref()?;
    if what.op != Operation::Jump || what.target != Some(following) {
        return None;
    }
    Some(last)
}

/// The procedure numbered `number`'s `index`th line mark's symbol.
pub fn line_label(
    number: usize,
    index: u32,
) -> String {
    format!("L{number}_line{index}")
}

/// The procedure numbered `number`'s lines as its listing marks them: each
/// one's symbol and source line, in order.
pub fn line_starts(
    procedure: &Procedure,
    number: usize,
) -> Result<Vec<(String, u32)>, Unprintable> {
    Ok(listing(procedure, number)?
        .into_iter()
        .filter_map(|item| match item {
            Item::Mark(Mark::Line { line, index }) => Some((line_label(number, index), line.line)),
            _ => None,
        })
        .collect())
}

/// A block's label: its offset, or for a block made after layout (an edge's
/// landing, `(entry + 1) << 32` and up) the two halves of its id, which stays
/// readable instead of ten digits (`L0_1_1`, not `L0_4294967297`).
pub fn label(
    number: usize,
    at: i64,
) -> String {
    if at >= 1 << 32 {
        return format!("L{number}_{}_{}", at >> 32, at & 0xffff_ffff);
    }
    format!("L{number}_{at}")
}

pub fn _procedure(
    procedure: &Procedure,
    names: &IndexMap<(Space, i64), String>,
    number: usize,
) -> Result<Vec<String>, Unprintable> {
    _procedure_of(procedure, listing(procedure, number)?, names, number)
}

/// The procedure's text, of the `items` a listing gives.
pub fn _procedure_of(
    procedure: &Procedure,
    items: Vec<Item>,
    names: &IndexMap<(Space, i64), String>,
    number: usize,
) -> Result<Vec<String>, Unprintable> {
    let regs = procedure.body.regs();
    let mut out = vec![format!("{} proc {}", procedure.name, if procedure.far { "far" } else { "near" })];
    for item in items {
        match item {
            Item::Align(CodeAlign { log2, .. }) => out.push(format!("    align {}", 1u32 << log2)),
            Item::Label(Label { name }) => out.push(format!("{name}:")),
            Item::Mark(_) | Item::Cfi(_) | Item::Seh(_) => {}
            Item::Callee(Callee { code, .. }) if !code.is_empty() => {
                out.extend(_code(&code).into_iter().map(|line| format!("    {line}")));
            }
            Item::Callee(Callee { name, far, tail, .. }) => {
                out.push(format!(
                    "    {} {}{name}",
                    if tail { "jmp" } else { "call" },
                    if far { "far ptr " } else { "" }
                ));
            }
            Item::Semantics(item) => match _instruction(regs, &item, names, number) {
                Ok(lines) => out.extend(lines.into_iter().map(|line| format!("    {line}"))),
                Err(error) => return Err(Unprintable(format!("{}: {error}", procedure.name))),
            },
        }
    }
    out.push(format!("{} endp", procedure.name));
    Ok(out)
}

/// The successor control reaches by running off the block's end, if any.
pub fn _falls_to(
    block: &lir::LirBlock,
    name: &str,
) -> Result<Option<i64>, Unprintable> {
    let last = block.insns.iter().rev().filter_map(|one| one.what.as_ref()).find(|what| what.op != Operation::Nothing);
    if last.is_some_and(|last| matches!(last.op, Operation::Jump | Operation::Return)) {
        return Ok(None);
    }
    let taken = match last {
        Some(last) if last.op == Operation::Branch => last.target,
        _ => None,
    };
    let mut rest: Vec<i64> = block.succ.iter().copied().filter(|one| Some(*one) != taken).collect();
    if rest.is_empty() {
        rest = block.succ.to_vec();
    }
    // Both edges of a branch may meet at one block (two cases of a switch
    // emptied to the same place): it falls there.
    rest.dedup();
    if rest.len() > 1 {
        return Err(Unprintable(format!(
            "{name}: block {} leaves for {} with no instruction choosing",
            block.at,
            crate::support::pyrepr::tuple(&block.succ)
        )));
    }
    Ok(rest.first().copied())
}

/// What a body names, found in one walk of its instructions: the registers
/// (`_roots`), those a call disturbs, and what `_roots_of_values` needs of the
/// cells. `saved_of`, `parts`, `built` and `frame_held` each asked for them of
/// the same body, each with a walk that copied the instruction list.
#[derive(Debug, Default, PartialEq)]
pub struct Roots {
    /// `_roots`: the registers named as operands and as address registers.
    pub plain: RegSet,
    /// The roots a call disturbs by its convention.
    pub disturbed: RegSet,
    /// A call disturbs the stack pointer.
    pub moves_stack: bool,
    /// `_roots_of_values` but for the registers a frame cell is addressed
    /// through.
    values: RegSet,
    /// The `through` of each cell in the frame space.
    frame_through: RegSet,
}

impl Roots {
    /// `_roots` where the frame register `pointer` holds a value: a frame cell
    /// (a stack array's too) names it only as the address it is, and does not
    /// use it.
    pub fn of_values(
        &self,
        regs: Regs,
        pointer: RegId,
    ) -> RegSet {
        let mut found = self.values.clone();
        found.extend(
            self.frame_through
                .iter()
                .filter(|through| *through != FRAME && *through != pointer && *through != RegId::None)
                .map(|through| regs.root(through)),
        );
        found
    }
}

/// `Roots` as the manager keeps it: worked out once for the instructions a body
/// has.
pub struct BodyRoots;

impl crate::analysis::facts::Fact for BodyRoots {
    type Result = Roots;
    type Inputs = crate::analysis::facts::Stamp;
    const NAME: &'static str = "body-roots";

    fn run(body: &lir::LirBody) -> Roots {
        let mut found = Roots::default();
        let regs = body.regs();
        let root = |register: RegId| regs.root(register);
        let stack = root(STACK);
        for block in &body.blocks {
            for one in block.insns.iter() {
                for disturbed in one.call.iter().flat_map(|call| call.disturbs.iter().copied().map(root)) {
                    found.moves_stack |= disturbed == stack;
                    found.disturbed.insert(disturbed);
                }
                let Some(what) = &one.what else { continue };
                // An instruction that sets the stack pointer to something not
                // known at compile time (a dynamic
                // `alloca`, `stackrestore`); `add sp, n` and `sub sp, n`, the
                // cleanup after a call, are the frame's own.
                let known = matches!(
                    (what.name.as_deref(), &what.dests[..], &what.sources[..]),
                    (Some("add" | "sub"), [Loc::Reg(dest)], [Loc::Reg(source), Loc::Imm(_)])
                        if root(dest.register) == stack && root(source.register) == stack
                );
                found.moves_stack |= !known
                    && what.dests.iter().any(|place| matches!(place, Loc::Reg(reg) if root(reg.register) == stack));
                for r#where in what.dests.iter().chain(&what.sources) {
                    if let Loc::Reg(ir::Reg { register, .. }) = r#where {
                        found.plain.insert(root(*register));
                        found.values.insert(root(*register));
                    }
                    // `lea` of a cell reads the register the cell is addressed
                    // through as much as a load of it does.
                    if let Some(at) = r#where.address() {
                        found.plain.extend([at.through, at.index_through].map(root));
                    }
                    if let Loc::Mem(cell) = r#where {
                        if cell.addr.as_ref().is_some_and(|addr| addr.space == Space::Frame) {
                            found.frame_through.insert(cell.through);
                        } else {
                            found.values.insert(root(cell.through));
                        }
                        found.values.insert(root(cell.index_through));
                    }
                }
            }
        }
        found
    }

    fn inputs(body: &lir::LirBody) -> crate::analysis::facts::Stamp {
        crate::analysis::facts::Stamp::of(body)
    }

    fn held_by(
        kept: &crate::analysis::facts::Stamp,
        body: &lir::LirBody,
    ) -> bool {
        kept.differing(body).is_some_and(|changed| changed.is_empty())
    }
}

pub fn _roots(body: &lir::LirBody) -> RegSet {
    body.facts.0.get::<BodyRoots>(body).plain.clone()
}

pub fn _instruction(
    regs: Regs,
    what: &Semantics,
    names: &IndexMap<(Space, i64), String>,
    number: usize,
) -> Result<Vec<String>, Unprintable> {
    let name = what.name.as_deref().unwrap_or("");
    if what.op == Operation::Fill {
        // Its operands are the registers the instruction names in its opcode.
        // A count (and so REP) is the extra result, whatever segments the
        // target names.
        return Ok(vec![format!("{}{name}", if what.dests.len() == 3 { "rep " } else { "" })]);
    }
    if what.op == Operation::Copy {
        // Its operands are the registers the instruction names in its opcode;
        // a source read through another segment than ds says so.
        let counted = what.dests.len() == 4;
        let rep = if counted { "rep " } else { "" };
        let size = match name {
            "movsb" => "byte",
            "movsw" => "word",
            _ => "dword",
        };
        let segmented = what.sources.len() == if counted { 5 } else { 4 };
        return Ok(vec![match what.sources.get(what.sources.len().wrapping_sub(2)).filter(|_| segmented) {
            Some(Loc::Reg(one)) if !regs.is_data_segment(one.register) => {
                format!(
                    "{rep}movs {size} ptr es:[di], {size} ptr {}:[si]",
                    format!("{:?}", one.register).to_lowercase()
                )
            }
            _ => format!("{rep}{name}"),
        }]);
    }
    let dests = what.dests.iter().map(|x| _operand(regs, x, names)).collect::<Result<Vec<_>, _>>()?;
    let sources = what.sources.iter().map(|x| _operand(regs, x, names)).collect::<Result<Vec<_>, _>>()?;
    let last = |items: &[String]| items[items.len() - 1].clone();
    Ok(match what.op {
        Operation::Nothing => {
            if matches!(name, "" | "nop") {
                vec![]
            } else if name == "enter" {
                vec![format!("enter {}, {}", sources[0], sources[1])]
            } else {
                vec![name.to_owned()]
            }
        }
        // The allocator names every copy `mov`; between xmm registers it
        // is `movaps`.
        Operation::Move
            if name == "mov"
                && matches!(
                    (&what.dests[0], &what.sources[0]),
                    (Loc::Reg(into), Loc::Reg(outof)) if into.register.is_xmm() && outof.register.is_xmm()
                ) =>
        {
            vec![format!("movaps {}, {}", dests[0], sources[0])]
        }
        Operation::Move | Operation::Address => vec![format!("{name} {}, {}", dests[0], sources[0])],
        // JWasm refuses `rep bsf`; the pin prints it, so the prefix is a byte.
        Operation::Binary if name == "bsf" && what.repeat => {
            vec!["db 0F3h".to_owned(), format!("bsf {}, {}", dests[0], sources[1])]
        }
        Operation::Binary | Operation::Select => vec![format!("{name} {}, {}", dests[0], sources[1])],
        // `sqrtsd xmm0, xmm0`: an xmm unary reads a source of its own, and a
        // unary that reads a place other than the one it writes names both.
        Operation::Unary
            if what.sources.len() == 1
                && (sources[0] != dests[0] || matches!(&what.dests[0], Loc::Reg(one) if one.register.is_xmm())) =>
        {
            vec![format!("{name} {}, {}", dests[0], sources[0])]
        }
        Operation::Unary => vec![format!("{name} {}", dests[0])],
        Operation::Compare if name.starts_with('f') => {
            let memory: Vec<&String> = sources
                .iter()
                .zip(&what.sources)
                .filter(|(_, source)| matches!(source, Loc::Mem(_)))
                .map(|(text, _)| text)
                .collect();
            vec![if memory.is_empty() { name.to_owned() } else { format!("{name} {}", memory[0]) }]
        }
        Operation::Compare => {
            vec![format!("{} {}, {}", if name.is_empty() { "cmp" } else { name }, sources[0], sources[1])]
        }
        Operation::Multiply if dests.len() == 1 => {
            if sources.len() == 3 {
                vec![format!("imul {}, {}, {}", dests[0], sources[1], sources[2])]
            } else if matches!(what.sources[1], Loc::Imm(_)) {
                vec![format!("imul {}, {}, {}", dests[0], sources[0], sources[1])]
            } else {
                vec![format!("imul {}, {}", dests[0], sources[1])]
            }
        }
        Operation::Multiply | Operation::Divide => vec![format!("{name} {}", last(&sources))],
        Operation::Extend => {
            // `cvtss2sd xmm0, xmm1`, `cvtsi2sd xmm0, eax`: the SSE converts
            // name their operands; `cwd` and `cdq` name theirs in the opcode.
            if matches!(name, "movsx" | "movzx")
                || name.starts_with("cvt")
                || matches!(name, "movq" | "movd" | "movq_x" | "movd_x")
            {
                vec![format!("{} {}, {}", name.trim_end_matches("_x"), dests[0], sources[0])]
            } else {
                vec![name.to_owned()]
            }
        }
        Operation::Push => match &what.sources[0] {
            Loc::Imm(ir::Imm { address: None, width, .. })
            | Loc::Imm(ir::Imm { address: Some(Addr { space: Space::Group, .. }), width, .. }) => {
                vec![format!("push{} {}", if *width == 4 { "d" } else { "w" }, sources[0])]
            }
            _ => vec![format!("push {}", sources[0])],
        },
        Operation::Pop => vec![format!("pop {}", dests[0])],
        Operation::Exchange if name == "fxch" => vec![format!("fxch {}", dests[1])],
        Operation::Exchange => vec![format!("xchg {}, {}", dests[0], dests[1])],
        Operation::Funnel => vec![format!("{name} {}, {}, {}", dests[0], sources[1], sources[2])],
        Operation::Jump if what.indirect && sources.len() == 1 => vec![format!("jmp {}", sources[0])],
        Operation::Branch | Operation::Jump => {
            let Some(target) = what.target else {
                return Err(Unprintable(format!("{} with no target", if name.is_empty() { "jump" } else { name })));
            };
            vec![format!("{name} {}", label(number, target))]
        }
        Operation::Call if what.indirect && sources.len() == 1 => vec![format!("call {}", sources[0])],
        Operation::Barrier => {
            vec![format!("{name} {}", dests.iter().chain(&sources).cloned().collect::<Vec<_>>().join(", "))]
        }
        Operation::FloatLoad => {
            if matches!(name, "fldz" | "fld1") || sources.is_empty() {
                vec![name.to_owned()]
            } else {
                vec![format!("{name} {}", sources[0])]
            }
        }
        Operation::FloatStore => vec![format!("{name} {}", dests[0])],
        Operation::FloatArith if matches!(what.sources[what.sources.len() - 1], Loc::Mem(_)) => {
            vec![format!("{name} {}", last(&sources))]
        }
        Operation::FloatArith | Operation::FloatArithPop => vec![format!("{name} {}, {}", dests[0], last(&sources))],
        Operation::FloatUnary => vec![name.to_owned()],
        // `retf n` removes the arguments too.
        Operation::Return if !sources.is_empty() => vec![format!("{name} {}", sources[0])],
        Operation::Return => vec![name.to_owned()],
        _ => return Err(Unprintable(what.repr())),
    })
}

pub fn _code(parts: &[InlinePart]) -> Vec<String> {
    let mut out = Vec::new();
    for part in parts {
        match part {
            InlinePart::Bytes(part) => {
                for chunk in part.chunks(16) {
                    out.push(
                        "db ".to_owned()
                            + &chunk.iter().map(|byte| format!("0{byte:02x}h")).collect::<Vec<_>>().join(","),
                    );
                }
            }
            InlinePart::Fixup(kind, name, offset) if kind == "offset" => {
                out.push(format!("dw offset {name}{}", _signed(*offset)));
            }
            InlinePart::Fixup(kind, name, _) if kind == "segment" => out.push(format!("dw seg {name}")),
            InlinePart::Near(opcode, name) => {
                out.push(format!("{} near ptr {name}", if *opcode == 0xE8 { "call" } else { "jmp" }));
            }
            InlinePart::Fixup(..) => {}
        }
    }
    out
}

pub fn _segment(
    regs: Regs,
    r#where: &Loc,
) -> bool {
    matches!(
        r#where,
        Loc::Reg(one) if regs.is_segment(one.register) && !regs.is_code_segment(one.register)
    )
}

fn named(
    names: &IndexMap<(Space, i64), String>,
    address: &Addr,
) -> String {
    let key = (address.space, address.index);
    names.get(&key).cloned().unwrap_or_else(|| panic!("KeyError: ({}, {})", address.space.repr(), address.index))
}

pub fn _operand(
    regs: Regs,
    r#where: &Loc,
    names: &IndexMap<(Space, i64), String>,
) -> Result<String, Unprintable> {
    Ok(match r#where {
        Loc::Reg(one) if one.st_index().is_some() => format!("st({})", one.st_index().expect("an x87 register")),
        Loc::Reg(ir::Reg { register, .. }) => target::name_of(regs, *register),
        Loc::Imm(ir::Imm { value, address: None, .. }) => value.to_string(),
        Loc::Imm(ir::Imm { value, address: Some(address), .. }) => {
            if address.space == Space::Group {
                named(names, address)
            } else {
                format!("offset {}{}", named(names, address), _signed(address.disp + value))
            }
        }
        Loc::Mem(cell) => _memory(regs, cell, names)?,
        // A named address keeps its symbol with the registers it is reached
        // through: `lea eax, b+12[eax]`.
        Loc::Address(ir::AddressRef { addr: Some(address), index_through, scale, through, .. }) => {
            let cell = ir::Mem {
                through: *through,
                index_through: *index_through,
                scale: *scale,
                ..ir::Mem::new(Some(*address), 2)
            };
            let text = _memory(regs, &cell, names)?;
            text.strip_prefix("word ptr ").map_or(text.clone(), str::to_owned)
        }
        Loc::Address(ir::AddressRef { through, index_through: index, scale, offset, .. }) => {
            format!("[{}{}]", _registers(regs, *through, *index, *scale), _signed(*offset))
        }
        Loc::Held(_) | Loc::Field(_) => return Err(Unprintable(format!("operand {}", r#where.repr()))),
    })
}

pub fn _registers(
    regs: Regs,
    base: RegId,
    index: RegId,
    scale: i64,
) -> String {
    let mut parts = if base != RegId::None { vec![target::name_of(regs, base)] } else { vec![] };
    if index != RegId::None {
        parts.push(target::name_of(regs, index) + &(if scale != 1 { format!("*{scale}") } else { String::new() }));
    }
    parts.join("+")
}

pub fn _memory(
    regs: Regs,
    cell: &ir::Mem,
    names: &IndexMap<(Space, i64), String>,
) -> Result<String, Unprintable> {
    let size = format!("{} ptr ", SIZES.get(&cell.width).unwrap_or_else(|| panic!("KeyError: {}", cell.width)));
    // As select.operand_of: a named address carries the displacement, and
    // `offset` is only the displacement of a cell with none.
    let Some(address) = &cell.addr else {
        if cell.through == RegId::None {
            return Err(Unprintable(format!("cell {}", cell.repr())));
        }
        return Ok(format!("{size}[{}{}]", target::name_of(regs, cell.through), _signed(cell.offset)));
    };
    let registers = _registers(regs, cell.through, cell.index_through, cell.scale);
    let disp = _signed(address.disp);
    match address.space {
        Space::Frame => {
            return Ok(format!("{size}[{}{disp}]", if registers.is_empty() { "bp" } else { &registers }));
        }
        Space::Segment | Space::External => {
            let symbol = named(names, address);
            let indexed = if registers.is_empty() { String::new() } else { format!("[{registers}]") };
            let segment = if address.segment == (RegId::None).iced() {
                String::new()
            } else {
                format!("{}:", target::name_of(regs, RegId::from(address.segment)))
            };
            return Ok(format!("{size}{segment}{symbol}{disp}{indexed}"));
        }
        Space::Literal if !registers.is_empty() => {
            let segment = if address.segment == (RegId::None).iced()
                || regs.default_segment(cell.through) == Some(RegId::from(address.segment))
            {
                String::new()
            } else {
                format!("{}:", target::name_of(regs, RegId::from(address.segment)))
            };
            return Ok(format!("{size}{segment}[{registers}{disp}]"));
        }
        // A direct address, as `[disp16]` (`[disp32]` where the cell says its
        // displacement is a dword): the offset is unsigned.
        Space::Literal if registers.is_empty() && address.segment == (RegId::None).iced() => {
            let kept = if cell.disp_width >= 4 { 0xFFFF_FFFF } else { 0xFFFF };
            return Ok(format!("{size}[{}]", address.disp & kept));
        }
        Space::Far if address.segment != (RegId::None).iced() => {
            let inside = if registers.is_empty() { address.disp.to_string() } else { format!("{registers}{disp}") };
            return Ok(format!("{size}{}:[{inside}]", target::name_of(regs, RegId::from(address.segment))));
        }
        _ => {}
    }
    Err(Unprintable(format!("cell {}", cell.repr())))
}

pub fn _signed(n: i64) -> String {
    if n > 0 {
        format!("+{n}")
    } else if n < 0 {
        n.to_string()
    } else {
        String::new()
    }
}

#[cfg(test)]
mod tests {
    //! Port of `tests/test_masm.py`: the jwasm printer spells each operand as
    //! select encodes it.

    use super::*;

    fn no_names() -> IndexMap<(Space, i64), String> {
        IndexMap::default()
    }

    fn insn(
        at: i64,
        what: Semantics,
    ) -> Arc<lir::Insn> {
        Arc::new(lir::Insn::new(at, Some((at, 1)), Some(what), vec![], vec![]))
    }

    fn ax() -> Loc {
        Loc::Reg(ir::Reg { register: RegId::AX, width: 2 })
    }

    /// A copy between xmm registers printed as `mov xmm0, xmm2`, which no
    /// assembler takes: it is `movaps`.
    #[test]
    fn test_a_copy_between_xmm_registers_prints_movaps() {
        let regs = crate::backend::registerinfo::test_regs();
        let xmm = |register| Loc::Reg(ir::Reg { register, width: 16 });
        let copy = Semantics {
            name: Some("mov".into()),
            dests: vec![xmm(RegId::XMM0)],
            sources: vec![xmm(RegId::XMM2)],
            ..Semantics::new(Operation::Move)
        };
        assert_eq!(_instruction(regs, &copy, &no_names(), 0).unwrap(), ["movaps xmm0, xmm2"]);
    }

    /// `cvtss2sd` and `sqrtsd` printed no source: `cvtss2sd` alone, `sqrtsd
    /// xmm0`, which no assembler takes.
    #[test]
    fn test_an_sse_convert_and_square_root_print_their_sources() {
        let regs = crate::backend::registerinfo::test_regs();
        let xmm = |register| Loc::Reg(ir::Reg { register, width: 16 });
        let print = |op, name: &str| {
            let what = Semantics {
                name: Some(name.into()),
                dests: vec![xmm(RegId::XMM0)],
                sources: vec![xmm(RegId::XMM1)],
                ..Semantics::new(op)
            };
            _instruction(regs, &what, &no_names(), 0).unwrap()
        };
        assert_eq!(print(Operation::Extend, "cvtss2sd"), ["cvtss2sd xmm0, xmm1"]);
        assert_eq!(print(Operation::Unary, "sqrtsd"), ["sqrtsd xmm0, xmm1"]);
        let same = Semantics {
            name: Some("sqrtsd".into()),
            dests: vec![xmm(RegId::XMM0)],
            sources: vec![xmm(RegId::XMM0)],
            ..Semantics::new(Operation::Unary)
        };
        assert_eq!(_instruction(regs, &same, &no_names(), 0).unwrap(), ["sqrtsd xmm0, xmm0"]);
    }

    /// A `bsf` marked `repeat` carries the pin's `rep` prefix, which JWasm
    /// refuses before anything but a string instruction: it is the byte,
    /// then the `bsf`.
    #[test]
    fn test_bsf_prints_its_rep_prefix_as_a_byte() {
        let regs = crate::backend::registerinfo::test_regs();
        let eax = Loc::Reg(ir::Reg { register: RegId::EAX, width: 4 });
        let ecx = Loc::Reg(ir::Reg { register: RegId::ECX, width: 4 });
        let scan = |name: &str, repeat| Semantics {
            name: Some(name.into()),
            dests: vec![eax.clone()],
            sources: vec![eax.clone(), ecx.clone()],
            repeat,
            ..Semantics::new(Operation::Binary)
        };
        assert_eq!(_instruction(regs, &scan("bsf", true), &no_names(), 0).unwrap(), ["db 0F3h", "bsf eax, ecx"]);
        assert_eq!(_instruction(regs, &scan("bsf", false), &no_names(), 0).unwrap(), ["bsf eax, ecx"]);
        assert_eq!(_instruction(regs, &scan("bsr", false), &no_names(), 0).unwrap(), ["bsr eax, ecx"]);
    }

    /// A string move prints as the instruction it is: `rep` where it has a
    /// count.
    #[test]
    fn test_a_string_move_prints_with_rep_where_it_repeats() {
        let regs = crate::backend::registerinfo::test_regs();
        let held = |value| Loc::Held(crate::model::ir::Held { value, width: 2 });
        // A count is the extra result; the segments, where the target has them,
        // the last two sources.
        let results = |count: usize| (0..count).map(|one| held(10 + one as u32)).collect::<Vec<_>>();
        let semantics = |dests: usize, sources: Vec<Loc>| Semantics {
            name: Some("movsw".into()),
            dests: results(dests),
            sources,
            ..Semantics::new(Operation::Copy)
        };
        let repeated = semantics(4, vec![held(1), held(2), held(3), held(4), held(5)]);
        let single = semantics(3, vec![held(2), held(3), held(4), held(5)]);
        assert_eq!(_instruction(regs, &repeated, &no_names(), 0).unwrap(), ["rep movsw"]);
        assert_eq!(_instruction(regs, &single, &no_names(), 0).unwrap(), ["movsw"]);
        // A target with no segment registers names none: its operands are the
        // registers.
        assert_eq!(
            _instruction(regs, &semantics(4, vec![held(1), held(2), held(3)]), &no_names(), 0).unwrap(),
            ["rep movsw"]
        );
        assert_eq!(_instruction(regs, &semantics(3, vec![held(2), held(3)]), &no_names(), 0).unwrap(), ["movsw"]);
        let ss = |name: &str, sources: Vec<Loc>| Semantics {
            name: Some(name.into()),
            dests: results(4),
            sources,
            ..Semantics::new(Operation::Copy)
        };
        let segment = |register| Loc::Reg(ir::Reg { register, width: 2 });
        let through = ss("movsd", vec![held(1), held(2), held(3), segment(RegId::SS), segment(RegId::ES)]);
        assert_eq!(
            _instruction(regs, &through, &no_names(), 0).unwrap(),
            ["rep movs dword ptr es:[di], dword ptr ss:[si]"]
        );
    }

    /// `lea bx,[bp-20]` for `&n` at bp-10: lower carries the displacement in
    /// both addr and offset, and the printer added them. asset_seek wrote n
    /// ten bytes below where qglsurf then read it.
    #[test]
    fn test_frame_address_displacement_once() {
        let regs = crate::backend::registerinfo::test_regs();
        let placed = Loc::Address(ir::AddressRef {
            through: RegId::BP,
            offset: -10,
            disp_width: 1,
            ..ir::AddressRef::new(Some(Addr::new(Space::Frame, -10)))
        });
        assert_eq!(_operand(regs, &placed, &no_names()).unwrap(), "[bp-10]");
    }

    /// `es:[bx+si+4]` for field 2 of a 3-byte record: pal_install copied the
    /// palette's blue from the next entry's green.
    #[test]
    fn test_far_cell_displacement_once() {
        let regs = crate::backend::registerinfo::test_regs();
        let addr = Addr { base: (RegId::BX).iced(), segment: (RegId::ES).iced(), ..Addr::new(Space::Far, 2) };
        let cell = ir::Mem { through: RegId::BX, offset: 2, ..ir::Mem::new(Some(addr), 1) };
        assert_eq!(_operand(regs, &Loc::Mem(cell), &no_names()).unwrap(), "byte ptr es:[bx+2]");
    }

    /// FloatAlloc's `fxch st(1)` printed as `xchg st(0), st(1)`, which jwasm
    /// refuses: 122 errors over qcport.
    #[test]
    fn test_x87_exchange_is_fxch() {
        let regs = crate::backend::registerinfo::test_regs();
        let st = |index| Loc::st(index);
        let swap = semantics(Operation::Exchange, "fxch", vec![st(0), st(1)], vec![st(0), st(1)]);
        assert_eq!(_instruction(regs, &swap, &no_names(), 0).unwrap(), ["fxch st(1)"]);
    }

    /// What the interrupt entry saves, lowest slot first: each push's register
    /// name, PUSHAD's eight dwords (EDI lowest) in its place.
    fn saved_by_the_entry() -> Vec<(String, i64)> {
        let (enter, _) = _interrupt_parts(Addr::new(Space::Group, 0));
        let mut pushed = Vec::new();
        for one in &enter[..INTERRUPT_SAVED] {
            match (one.op, one.name.as_deref(), one.sources.first()) {
                (_, Some("pushad"), _) => pushed.push(
                    vec!["eax", "ecx", "edx", "ebx", "esp", "ebp", "esi", "edi"]
                        .into_iter()
                        .map(|name| (name.to_owned(), 4))
                        .collect::<Vec<_>>(),
                ),
                (Operation::Push, _, Some(Loc::Reg(register))) => {
                    pushed.push(vec![(format!("{:?}", register.register).to_lowercase(), 2)])
                }
                other => panic!("the entry saves with {other:?}"),
            }
        }
        pushed.into_iter().rev().flat_map(|group| group.into_iter().rev()).collect()
    }

    /// The handler pushed AX, BX, CX, DX, SI, DI and BP in BCC's nine words and
    /// again in PUSHAD: 14 bytes of code, 14 of stack and 14-35 cycles per
    /// interrupt, saved twice for a frame nothing else read.
    #[test]
    fn test_an_interrupt_handler_saves_each_register_once() {
        let mut names: Vec<String> = saved_by_the_entry()
            .into_iter()
            .map(|(name, size)| if size == 4 { name[1..].to_owned() } else { name })
            .collect();
        names.sort();
        let unique: BTreeSet<_> = names.iter().cloned().collect();
        assert_eq!(names.len(), unique.len(), "saved twice: {names:?}");
        assert_eq!(
            names,
            ["bp", "bx", "cx", "di", "ds", "dx", "es", "fs", "gs", "ax", "si", "sp"]
                .iter()
                .map(|one| one.to_string())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>()
        );
    }

    /// The frame the target's calling.toml states (`interrupt_frame`) is the
    /// one the entry builds, so a handler's parameters read the registers
    /// they name.
    #[test]
    fn test_the_entry_builds_the_frame_mir_states() {
        let calling = llrm_target::Target::calling(&llrm_x86_m16::M16);
        let stated = calling.interrupt().expect("an interrupt convention").interrupt_frame.clone();
        assert_eq!(saved_by_the_entry()[..], stated[..12]);
        assert_eq!(interrupt_parameters(), 2);
        assert_eq!(llrm_x86::calling::interrupt_slot(calling.interrupt().unwrap(), "ax"), Some(36));
        assert_eq!(llrm_x86::calling::interrupt_slot(calling.interrupt().unwrap(), "ds"), Some(6));
    }

    fn _printed(
        sources: Vec<Loc>,
        reserve: i64,
    ) -> Vec<String> {
        _printed_entering(sources, reserve, false)
    }

    fn _printed_entering(
        sources: Vec<Loc>,
        reserve: i64,
        enter: bool,
    ) -> Vec<String> {
        let r#move = insn(0, semantics(Operation::Move, "mov", vec![ax()], sources));
        let leave = insn(1, semantics(Operation::Return, "retf", vec![], vec![]));
        let blocks = vec![lir::LirBlock::new(1, vec![r#move, leave])];
        let body =
            lir::LirBody::new("get", 1, crate::model::lir::shared(blocks), IndexMap::default(), IndexMap::default());
        let procedure = Procedure {
            name: "_get".into(),
            public: true,
            far: true,
            body,
            reserve,
            callees: IndexMap::default(),
            interrupt: None,
            size: enter,
            entry: 0,
            stack_check: None,
            probe_stack: None,
            registers: llrm_target::Target::frame_registers(&llrm_x86_m16::M16),
        };
        _procedure(&procedure, &no_names(), 0).unwrap().iter().map(|line| line.trim().to_owned()).collect()
    }

    /// A procedure's frame, saved registers and wrap each asked the same
    /// question of its body, each with a walk of every instruction
    /// (`assemble` frame and shrink-wrap: 32.5 M of d_faces, 5.3 M of it these
    /// walks). They are worked out once for a body.
    #[test]
    fn test_what_a_body_names_is_worked_out_once_for_all_the_frame_questions() {
        let bx = Loc::Reg(ir::Reg { register: RegId::BX, width: 2 });
        let r#move = insn(0, semantics(Operation::Move, "mov", vec![ax()], vec![bx]));
        let leave = insn(1, semantics(Operation::Return, "retf", vec![], vec![]));
        let blocks = vec![lir::LirBlock::new(1, vec![r#move, leave])];
        let body =
            lir::LirBody::new("get", 1, crate::model::lir::shared(blocks), IndexMap::default(), IndexMap::default());
        let procedure = Procedure {
            name: "_get".into(),
            public: true,
            far: true,
            body,
            reserve: 4,
            callees: IndexMap::default(),
            interrupt: None,
            size: false,
            entry: 0,
            stack_check: None,
            probe_stack: None,
            registers: llrm_target::Target::frame_registers(&llrm_x86_m16::M16),
        };
        _procedure(&procedure, &no_names(), 0).unwrap();
        let _ = saved_of(&procedure);
        assert_eq!(procedure.body.facts.0.runs::<BodyRoots>(), 1);
    }

    /// A sibling call is a return that jumps: the frame is taken down, then
    /// `jmp callee` stands where `ret` would. As a `ret` it was a `ret` and the
    /// callee never ran.
    #[test]
    fn a_sibling_call_is_a_jump_after_the_epilogue() {
        let bx = Loc::Reg(ir::Reg { register: RegId::BX, width: 2 });
        let r#move = insn(0, semantics(Operation::Move, "mov", vec![ax()], vec![bx]));
        let leave = insn(1, semantics(Operation::Return, "jmp", vec![], vec![]));
        let blocks = vec![lir::LirBlock::new(1, vec![r#move, leave])];
        let body =
            lir::LirBody::new("get", 1, crate::model::lir::shared(blocks), IndexMap::default(), IndexMap::default());
        let procedure = Procedure {
            name: "_get".into(),
            public: true,
            far: false,
            body,
            reserve: 0,
            callees: IndexMap::from_iter([(1, Callee::new("_other", false))]),
            interrupt: None,
            size: false,
            entry: 0,
            stack_check: None,
            probe_stack: None,
            registers: llrm_target::Target::frame_registers(&llrm_x86_m16::M16),
        };
        let lines: Vec<String> =
            _procedure(&procedure, &no_names(), 0).unwrap().iter().map(|line| line.trim().to_owned()).collect();
        assert!(lines.iter().any(|one| one == "jmp _other"), "{lines:?}");
        assert!(!lines.iter().any(|one| one == "ret"), "{lines:?}");
    }

    /// A sibling call in a body whose frame register was omitted: the exits
    /// inserted ahead of the return (`frame_exit` set) must still end in the
    /// jump to the callee, not a `ret`.
    #[test]
    fn a_sibling_call_in_an_omitted_frame_still_jumps() {
        let bx = Loc::Reg(ir::Reg { register: RegId::EBX, width: 4 });
        let si = Loc::Reg(ir::Reg { register: RegId::ESI, width: 4 });
        let r#move = insn(0, semantics(Operation::Move, "mov", vec![si], vec![bx]));
        let leave = insn(1, semantics(Operation::Return, "jmp", vec![], vec![]));
        let blocks = vec![lir::LirBlock::new(1, vec![r#move, leave])];
        let body = lir::LirBody {
            bits: 32,
            ..lir::LirBody::new("get", 1, crate::model::lir::shared(blocks), IndexMap::default(), IndexMap::default())
        };
        let procedure = Procedure {
            name: "_get".into(),
            public: true,
            far: false,
            body,
            reserve: 0,
            callees: IndexMap::from_iter([(1, Callee::new("_other", false))]),
            interrupt: None,
            size: false,
            entry: 0,
            stack_check: None,
            probe_stack: None,
            registers: llrm_target::Target::frame_registers(&llrm_x86_m32::M32),
        };
        let procedure = exits_inserted(procedure);
        assert_eq!(procedure.body.frame_exit, Some(true), "premise: the frame is omitted and its exits inserted");
        let lines: Vec<String> =
            _procedure(&procedure, &no_names(), 0).unwrap().iter().map(|line| line.trim().to_owned()).collect();
        assert!(lines.iter().any(|one| one == "jmp _other"), "{lines:?}");
        assert!(!lines.iter().any(|one| one == "ret"), "{lines:?}");
    }

    /// x_hanoi2 `mv` -Os m32 pushed four registers before its `n == 0` exit:
    /// the size refusal of a wrap around a shared return was the 16-bit
    /// target's (a jump over the epilogue outgrows its short form), and applied
    /// everywhere. The target description states it (`frame_wrap_split_size`).
    #[test]
    fn test_a_size_build_wraps_a_shared_return_only_where_the_target_allows() {
        let esi = Loc::Reg(ir::Reg { register: RegId::ESI, width: 4 });
        let jump = |at: i64, target: i64| {
            insn(at, Semantics { name: Some("je".into()), target: Some(target), ..Semantics::new(Operation::Branch) })
        };
        let touch = insn(20, semantics(Operation::Move, "mov", vec![esi.clone()], vec![esi]));
        let ret = |at: i64| insn(at, semantics(Operation::Return, "ret", vec![], vec![]));
        let block = |at: i64, insns: Vec<Arc<lir::Insn>>, succ: Vec<i64>| lir::LirBlock {
            succ: succ.into(),
            ..lir::LirBlock::new(at, insns)
        };
        let blocks = vec![
            block(0, vec![jump(1, 3)], vec![3, 2]),
            block(2, vec![touch], vec![3]),
            block(3, vec![ret(30)], vec![]),
        ];
        let wrapped = |target: &dyn llrm_target::Target, bits: u32| {
            let body = lir::LirBody {
                bits,
                ..lir::LirBody::new(
                    "f",
                    0,
                    crate::model::lir::shared(blocks.clone()),
                    IndexMap::default(),
                    IndexMap::default(),
                )
            };
            let procedure = Procedure {
                name: "_f".into(),
                public: true,
                far: false,
                body: body.clone(),
                reserve: 0,
                callees: IndexMap::default(),
                interrupt: None,
                size: true,
                entry: 0,
                stack_check: None,
                probe_stack: None,
                registers: target.frame_registers(),
            };
            wrap_of(&procedure, &body, false, false).is_some()
        };
        assert!(wrapped(&llrm_x86_m32::M32, 32), "m32 states it");
        assert!(!wrapped(&llrm_x86_m16::M16, 16), "m16 does not");
    }

    fn through_bp() -> Loc {
        Loc::Mem(ir::Mem { through: RegId::BP, ..ir::Mem::new(Some(Addr::new(Space::Frame, 6)), 2) })
    }

    fn has(
        lines: &[String],
        text: &str,
    ) -> bool {
        lines.iter().any(|one| one == text)
    }

    /// Every procedure got `push bp; mov bp,sp` .. `mov sp,bp; pop bp`:
    /// snd_mix_loops read one global in seven instructions where bcc used
    /// two.
    #[test]
    fn test_frame_only_where_something_uses_it() {
        let bx = Loc::Reg(ir::Reg { register: RegId::BX, width: 2 });
        assert_eq!(_printed(vec![bx], 0), ["_get proc far", "L0_1:", "mov ax, bx", "retf", "_get endp"]);
        let params = _printed(vec![through_bp()], 0);
        let tail = &params[params.len() - 3..params.len() - 1];
        assert!(params[1..3] == ["push bp", "mov bp, sp"] && tail == ["pop bp", "retf"]);
        assert!(!has(&params, "mov sp, bp") && !has(&params, "leave"));
    }

    /// `mov sp,bp; pop bp` where bcc writes `leave`: 261 instructions over
    /// qcport.
    #[test]
    fn test_reserved_frame_leaves_in_one_instruction() {
        let lines = _printed(vec![through_bp()], 4);
        assert!(lines[lines.len() - 3..lines.len() - 1] == ["leave", "retf"]);
        assert!(!has(&lines, "mov sp, bp") && !has(&lines, "pop bp"));
    }

    /// Tuned for size, a frame with locals opened with `enter N,0` (4 bytes
    /// against 6, but 14 clocks against 3 on the 486, 407 QCport
    /// functions): no level opens one.
    #[test]
    fn test_no_level_opens_a_frame_with_enter() {
        for size in [true, false] {
            let entered = _printed_entering(vec![through_bp()], 4, size);
            assert_eq!(entered[1..4], ["push bp", "mov bp, sp", "sub sp, 4"], "size {size}: {entered:?}");
        }
        let bare = _printed_entering(vec![through_bp()], 0, true);
        assert_eq!(bare[1..3], ["push bp", "mov bp, sp"]);
    }

    fn _checked(
        check: Option<StackCheck>,
        reserve: i64,
    ) -> (Vec<String>, Vec<(String, String)>) {
        let r#move = insn(0, semantics(Operation::Move, "mov", vec![ax()], vec![through_bp()]));
        let leave = insn(1, semantics(Operation::Return, "retf", vec![], vec![]));
        let body = lir::LirBody::new(
            "get",
            1,
            vec![lir::LirBlock::new(1, vec![r#move, leave]).into()],
            IndexMap::default(),
            IndexMap::default(),
        );
        let procedure = Procedure {
            name: "_get".into(),
            public: true,
            far: true,
            body,
            reserve,
            callees: IndexMap::default(),
            interrupt: None,
            size: true,
            entry: 0,
            stack_check: check,
            probe_stack: None,
            registers: llrm_target::Target::frame_registers(&llrm_x86_m16::M16),
        };
        let mut names = no_names();
        let externs = stack_externs(std::slice::from_ref(&procedure), &mut names);
        (_procedure(&procedure, &names, 0).unwrap().iter().map(|line| line.trim().to_owned()).collect(), externs)
    }

    /// `-fsanitize=stack` compares SP with the word and calls the routine the
    /// runtime's description names, here made-up ones: a pass that wrote
    /// `b$pendchk` itself would not follow them. The call is last, where no
    /// block falls into it, and the default has neither.
    #[test]
    fn test_a_stack_check_names_what_the_runtime_states() {
        let check = StackCheck { limit: "FOO".into(), handler: "BAR".into(), far: true, red_zone: 0, entry: None };
        let (lines, externs) = _checked(Some(check), 4);
        assert_eq!(
            lines[1..7],
            ["push bp", "mov bp, sp", "sub sp, 4", "jb L0_2", "cmp sp, word ptr FOO", "jb L0_2"],
            "{lines:?}"
        );
        assert_eq!(lines[lines.len() - 3..], ["L0_2:", "call far ptr BAR", "_get endp"]);
        assert_eq!(externs, [("FOO".to_owned(), "byte".to_owned()), ("BAR".to_owned(), "far".to_owned())]);
        let (plain, none) = _checked(None, 4);
        assert!(
            plain.iter().all(|one| !one.contains("cmp sp") && !one.contains("BAR")) && none.is_empty(),
            "{plain:?}"
        );
        assert_eq!(plain[1..4], ["push bp", "mov bp, sp", "sub sp, 4"]);
    }

    /// A flat m32 procedure that keeps ESI, with a local at `[ebp-4]`.
    fn kept(
        check: Option<StackCheck>,
        moves_stack: bool,
    ) -> Vec<String> {
        let esi = Loc::Reg(ir::Reg { register: RegId::ESI, width: 4 });
        let cell = ir::Mem { through: RegId::EBP, ..ir::Mem::new(Some(Addr::new(Space::Frame, -4).in_slot(-4)), 4) };
        let mut insns = vec![insn(0, semantics(Operation::Move, "mov", vec![esi], vec![Loc::Mem(cell)]))];
        if moves_stack {
            let esp = Loc::Reg(ir::Reg { register: RegId::ESP, width: 4 });
            let eax = Loc::Reg(ir::Reg { register: RegId::EAX, width: 4 });
            insns.push(insn(1, semantics(Operation::Binary, "sub", vec![esp.clone()], vec![esp, eax])));
        }
        insns.push(insn(2, semantics(Operation::Return, "ret", vec![], vec![])));
        let mut body = lir::LirBody::new(
            "get",
            1,
            vec![lir::LirBlock::new(1, insns).into()],
            IndexMap::default(),
            IndexMap::default(),
        );
        body.bits = 32;
        let procedure = Procedure {
            name: "_get".into(),
            public: true,
            far: false,
            body,
            reserve: 4,
            callees: IndexMap::default(),
            interrupt: None,
            size: false,
            entry: 0,
            stack_check: check,
            probe_stack: None,
            registers: llrm_target::Target::frame_registers(&llrm_x86_m32::M32),
        };
        let procedure = saved_first(procedure);
        let mut names = no_names();
        stack_externs(std::slice::from_ref(&procedure), &mut names);
        _procedure(&procedure, &names, 0).unwrap().iter().map(|line| line.trim().to_owned()).collect()
    }

    /// A stack-checked or alloca frame kept the old order (`sub esp` before the
    /// pushes), the carve-outs of the first CSR-first change: entry-built flat
    /// m32 frames are the pin's order whatever they hold.
    #[test]
    fn test_a_stack_checked_frame_pushes_its_registers_before_it_reserves() {
        let check = StackCheck { limit: "FOO".into(), handler: "BAR".into(), far: false, red_zone: 0, entry: None };
        let lines = kept(Some(check), false);
        // No frame register here: the cells are addressed through ESP.
        assert_eq!(
            lines[1..6],
            ["push esi", "sub esp, 12", "jb L0_3", "cmp esp, dword ptr FOO", "jb L0_3"],
            "{lines:?}"
        );
    }

    #[test]
    fn test_an_alloca_frame_pops_its_registers_from_below_ebp() {
        let lines = kept(None, true);
        // The registers take 4 bytes; the 4-byte local keeps its 8-byte
        // alignment from BP.
        assert_eq!(lines[1..5], ["push ebp", "mov ebp, esp", "push esi", "sub esp, 8"], "{lines:?}");
        let at = lines.iter().position(|line| line == "ret").expect("a return");
        assert_eq!(
            lines[at - 3..=at],
            ["lea esp, word ptr [ebp-4]", "pop esi", "pop ebp", "ret"].map(String::from),
            "{lines:?}"
        );
    }

    /// Return-tail layout must count the pop/leave absent from allocated LIR.
    #[test]
    fn test_return_overhead_prices_the_implicit_frame_teardown() {
        let r#move = insn(0, semantics(Operation::Move, "mov", vec![ax()], vec![through_bp()]));
        let returned = insn(1, semantics(Operation::Return, "retf", vec![], vec![]));
        let blocks = vec![lir::LirBlock::new(1, vec![r#move, returned])];
        let body =
            lir::LirBody::new("get", 1, crate::model::lir::shared(blocks), IndexMap::default(), IndexMap::default());
        let procedure = |reserve| Procedure {
            name: "_get".into(),
            public: true,
            far: true,
            body: body.clone(),
            reserve,
            callees: IndexMap::default(),
            interrupt: None,
            size: false,
            entry: 0,
            stack_check: None,
            probe_stack: None,
            registers: llrm_target::Target::frame_registers(&llrm_x86_m16::M16),
        };

        assert_eq!(return_overhead_bytes(&procedure(0)).unwrap(), 1);
        assert_eq!(return_overhead_bytes(&procedure(4)).unwrap(), 1);
    }

    /// `ret imm16` pops at most 64 KB less one: a flat callee that pops more
    /// raises the stack over its arguments with the return address moved to
    /// the top of them (pr20621-1: a 64 KB struct by value), not `ret 65540`.
    #[test]
    fn test_a_callee_that_pops_64_kb_moves_its_return_address_over_the_arguments() {
        let popping = |bytes: i64| {
            let returned = insn(1, semantics(Operation::Return, "ret", vec![], vec![]));
            let body = lir::LirBody::new(
                "get",
                1,
                vec![lir::LirBlock::new(1, vec![returned]).into()],
                IndexMap::default(),
                IndexMap::default(),
            );
            let body = cleaned_returns(&body, bytes, 4).unwrap();
            let procedure = Procedure {
                name: "_get".into(),
                public: true,
                far: false,
                body,
                reserve: 0,
                callees: IndexMap::default(),
                interrupt: None,
                size: false,
                entry: 0,
                stack_check: None,
                probe_stack: None,
                registers: llrm_target::Target::frame_registers(&llrm_x86_m32::M32),
            };
            _procedure(&procedure, &no_names(), 0)
                .unwrap()
                .iter()
                .map(|line| line.trim().to_owned())
                .collect::<Vec<_>>()
        };
        assert!(popping(65535).iter().any(|line| line == "ret 65535"), "{:?}", popping(65535));
        let lines = popping(65540);
        assert!(
            lines.windows(3).any(|three| three == ["pop dword ptr [esp+65536]", "add esp, 65536", "ret"]),
            "{lines:?}"
        );
        assert!(
            cleaned_returns(
                &lir::LirBody::new("get", 1, Vec::new(), IndexMap::default(), IndexMap::default()),
                65540,
                2
            )
            .is_err()
        );
    }

    /// The listing opened `.model medium` whatever the target: a flat program's
    /// header is its target's.
    #[test]
    fn test_a_listing_opens_with_its_targets_header() {
        let module = Module {
            registers: crate::backend::registerinfo::test_regs(),
            object: llrm_target::object::ObjectFormat {
                formats: vec![llrm_target::object::Format::Omf],
                default: llrm_target::object::Format::Omf,
                bitness: 32,
                header: vec![".386".to_owned(), ".model flat".to_owned()],
                dwarf_version: llrm_target::object::DEFAULT_DWARF_VERSION,
                pc_relative: false,
                got_external: false,
            },
            code: "T_TEXT".into(),
            names: no_names(),
            externs: Vec::new(),
            publics: Vec::new(),
            data: Vec::new(),
            procedures: Vec::new(),
            private: BTreeSet::new(),
            far_bss: BTreeSet::new(),
            code_segments: Default::default(),
            padding: Default::default(),
            requests: BTreeSet::new(),
            debug: None,
            stack: 0,
        };
        assert!(text(&module).unwrap().starts_with(".386\n.model flat\n\n"));
    }

    /// The frame register and stack pointer are LIR's BP and SP whatever the
    /// target: a flat one listed `push bp; mov bp,sp` and `[bp+6]`, 16-bit code
    /// in a 32-bit program.
    #[test]
    fn test_a_flat_frame_is_spelled_with_its_own_registers() {
        let r#move = insn(0, semantics(Operation::Move, "mov", vec![ax()], vec![through_bp()]));
        let leave = insn(1, semantics(Operation::Return, "ret", vec![], vec![]));
        let body = lir::LirBody::new(
            "get",
            1,
            vec![lir::LirBlock::new(1, vec![r#move, leave]).into()],
            IndexMap::default(),
            IndexMap::default(),
        );
        let registers = llrm_target::FrameRegisters {
            pointer: RegId::EBP,
            stack: RegId::ESP,
            saved: Vec::new(),
            slot: 4,
            align: 4,
            optional: false,
            enter: false,
            wrap_split_size: false,
            slot_scratch: None,
            free: false,
        };
        let procedure = Procedure {
            name: "_get".into(),
            public: true,
            far: false,
            body,
            reserve: 4,
            callees: IndexMap::default(),
            interrupt: None,
            size: false,
            entry: 0,
            stack_check: None,
            probe_stack: None,
            registers,
        };
        let lines: Vec<String> =
            _procedure(&procedure, &no_names(), 0).unwrap().iter().map(|line| line.trim().to_owned()).collect();
        assert!(has(&lines, "push ebp") && has(&lines, "mov ebp, esp") && has(&lines, "sub esp, 4"), "{lines:?}");
        assert!(lines.iter().any(|one| one.contains("[ebp+6]")), "{lines:?}");
        assert!(
            !lines.iter().any(|one| one.contains("[bp") || one.contains(" bp") || one.contains(" sp")),
            "{lines:?}"
        );
    }

    /// `enter` is the target's to take for size (`frame_enter`): a target that
    /// states it gets the 4-byte frame at -Os only, where the 486's own
    /// description (false) gets push/mov/sub at both levels.
    #[test]
    fn test_a_target_that_states_frame_enter_opens_a_size_frame_with_it() {
        let body = lir::LirBody {
            bits: 32,
            ..lir::LirBody::new(
                "get",
                1,
                vec![lir::LirBlock::new(1, vec![ret()]).into()],
                IndexMap::default(),
                IndexMap::default(),
            )
        };
        let listing = |enter: bool, size: bool| {
            let registers = llrm_target::FrameRegisters {
                pointer: RegId::EBP,
                stack: RegId::ESP,
                saved: Vec::new(),
                slot: 4,
                align: 4,
                optional: false,
                enter,
                wrap_split_size: false,
                slot_scratch: None,
                free: false,
            };
            let procedure = Procedure {
                name: "_get".into(),
                public: true,
                far: false,
                body: body.clone(),
                reserve: 4,
                callees: IndexMap::default(),
                interrupt: None,
                size,
                entry: 0,
                stack_check: None,
                probe_stack: None,
                registers,
            };
            _procedure(&procedure, &no_names(), 0)
                .unwrap()
                .iter()
                .map(|line| line.trim().to_owned())
                .collect::<Vec<_>>()
        };
        assert_eq!(listing(true, true)[1], "enter 4, 0");
        assert_eq!(listing(true, false)[1..4], ["push ebp", "mov ebp, esp", "sub esp, 4"]);
        assert_eq!(listing(false, true)[1..4], ["push ebp", "mov ebp, esp", "sub esp, 4"]);
    }

    /// A flat procedure whose frame register the target may leave out, `body`
    /// its instructions.
    fn flat(
        optional: bool,
        reserve: i64,
        body: Vec<Arc<lir::Insn>>,
        callees: IndexMap<i64, Callee>,
    ) -> Vec<String> {
        flat_blocks(optional, reserve, vec![lir::LirBlock::new(1, body).into()], callees)
    }

    /// `flat`, of several blocks.
    fn flat_blocks(
        optional: bool,
        reserve: i64,
        blocks: Vec<lir::Block>,
        callees: IndexMap<i64, Callee>,
    ) -> Vec<String> {
        flat_popping(optional, reserve, blocks, callees, IndexMap::default())
    }

    /// `flat_blocks`, the bytes each indirect call's callee pops given.
    fn flat_popping(
        optional: bool,
        reserve: i64,
        blocks: Vec<lir::Block>,
        callees: IndexMap<i64, Callee>,
        call_pops: IndexMap<i64, i64>,
    ) -> Vec<String> {
        let body = lir::LirBody {
            bits: 32,
            call_pops: Arc::new(call_pops),
            ..lir::LirBody::new("get", 1, blocks, IndexMap::default(), IndexMap::default())
        };
        let registers = llrm_target::FrameRegisters {
            pointer: RegId::EBP,
            stack: RegId::ESP,
            saved: Vec::new(),
            slot: 4,
            align: 4,
            optional,
            enter: false,
            wrap_split_size: false,
            slot_scratch: None,
            free: false,
        };
        let procedure = Procedure {
            name: "_get".into(),
            public: true,
            far: false,
            body,
            reserve,
            callees,
            interrupt: None,
            size: false,
            entry: 0,
            stack_check: None,
            probe_stack: None,
            registers,
        };
        _procedure(&procedure, &no_names(), 0).unwrap().iter().map(|line| line.trim().to_owned()).collect()
    }

    fn eax() -> Loc {
        Loc::Reg(ir::Reg { register: RegId::EAX, width: 4 })
    }

    /// A cell of the frame, `disp` from the frame register.
    fn cell(disp: i64) -> Loc {
        Loc::Mem(ir::Mem { through: RegId::BP, ..ir::Mem::new(Some(Addr::new(Space::Frame, disp)), 4) })
    }

    fn ret() -> Arc<lir::Insn> {
        insn(9, semantics(Operation::Return, "ret", vec![], vec![]))
    }

    /// bench/fib's `push ebp; mov ebp,esp ... leave` cost three instructions a
    /// call. A function whose cells the stack pointer can name keeps no
    /// frame register: an argument, above the return address, is `[esp +
    /// depth + 4]` and a local `[esp + depth - 4 - n]`, `depth` the bytes
    /// pushed so far, which a push moves and a callee's pop moves back.
    #[test]
    fn test_a_frame_the_stack_pointer_can_address_has_no_frame_register() {
        let push = insn(1, semantics(Operation::Push, "push", vec![], vec![eax()]));
        let call = insn(2, semantics(Operation::Call, "call", vec![], vec![]));
        let callees = IndexMap::from_iter([(2, Callee { pops: 4, ..Callee::new("_f", false) })]);
        let body = vec![
            insn(0, semantics(Operation::Move, "mov", vec![eax()], vec![cell(8)])),
            push,
            insn(3, semantics(Operation::Move, "mov", vec![eax()], vec![cell(8)])),
            insn(4, semantics(Operation::Move, "mov", vec![eax()], vec![cell(-4)])),
            call,
            insn(5, semantics(Operation::Move, "mov", vec![eax()], vec![cell(8)])),
            ret(),
        ];
        let lines = flat(true, 8, body, callees);
        assert!(!lines.iter().any(|one| one.contains("ebp")), "{lines:?}");
        assert_eq!(
            lines.iter().filter(|one| one.starts_with("mov eax, dword ptr")).collect::<Vec<_>>(),
            [
                "mov eax, dword ptr [esp+16]",
                "mov eax, dword ptr [esp+20]",
                "mov eax, dword ptr [esp+8]",
                "mov eax, dword ptr [esp+16]"
            ],
            "{lines:?}"
        );
        assert!(has(&lines, "sub esp, 12") && has(&lines, "add esp, 12"), "{lines:?}");
    }

    /// A call that never returns ends its block with no jump after it, and the
    /// block laid out next is not entered from it. The stack depth is the
    /// CFG's: carried across such a call it made the label after it
    /// "reached at depth 4 and 0", and a function the frame register had
    /// been freed for failed the compile ("the frame register was given to
    /// a value and the frame cannot be addressed through the stack
    /// pointer", jpeg at nib -m32 -O1, llrm#1466).
    #[test]
    fn test_the_block_after_a_call_that_never_returns_is_not_entered_from_it() {
        let branch = Semantics { target: Some(3), ..semantics(Operation::Branch, "jne", vec![], vec![]) };
        let onward = Semantics { target: Some(2), ..semantics(Operation::Jump, "jmp", vec![], vec![]) };
        let blocks = vec![
            lir::LirBlock {
                succ: vec![3, 2].into(),
                ..lir::LirBlock::new(
                    1,
                    vec![
                        insn(0, semantics(Operation::Move, "mov", vec![eax()], vec![cell(8)])),
                        insn(1, branch),
                        insn(2, onward),
                    ],
                )
            },
            lir::LirBlock::new(
                2,
                vec![
                    insn(3, semantics(Operation::Push, "push", vec![], vec![eax()])),
                    insn(4, semantics(Operation::Call, "call", vec![], vec![])),
                ],
            ),
            lir::LirBlock::new(3, vec![ret()]),
        ];
        let callees = IndexMap::from_iter([(4, Callee::new("_fail", false))]);
        let lines = flat_blocks(true, 8, crate::model::lir::shared(blocks), callees);
        assert!(!lines.iter().any(|one| one.contains("ebp")), "the frame register stays: {lines:?}");
    }

    /// A target that does not let its frame register go, and a body that reads
    /// it as a value or calls through a pointer, keep the frame.
    #[test]
    fn test_a_frame_register_that_is_read_or_may_not_go_stays() {
        let read = || vec![insn(0, semantics(Operation::Move, "mov", vec![eax()], vec![cell(8)])), ret()];
        assert!(has(&flat(false, 8, read(), IndexMap::default()), "push ebp"));
        let value = vec![
            insn(
                0,
                semantics(
                    Operation::Move,
                    "mov",
                    vec![eax()],
                    vec![Loc::Reg(ir::Reg { register: RegId::EBP, width: 4 })],
                ),
            ),
            ret(),
        ];
        assert!(has(&flat(true, 8, value, IndexMap::default()), "push ebp"));
        let indirect = vec![
            insn(0, Semantics { indirect: true, ..semantics(Operation::Call, "call", vec![], vec![eax()]) }),
            ret(),
        ];
        assert!(has(&flat(true, 8, indirect, IndexMap::default()), "push ebp"));
        // The call frame named what the callee pops: the depth after it is
        // known (tail_calls_edges `through` kept `ebp` for this call).
        let framed = vec![
            insn(0, Semantics { indirect: true, ..semantics(Operation::Call, "call", vec![], vec![eax()]) }),
            ret(),
        ];
        let popping = flat_popping(
            true,
            8,
            vec![lir::LirBlock::new(1, framed).into()],
            IndexMap::default(),
            IndexMap::from_iter([(0, 0)]),
        );
        assert!(!has(&popping, "push ebp"));
        let flags = vec![
            insn(0, semantics(Operation::Nothing, "pushf", vec![], vec![])),
            insn(1, semantics(Operation::Move, "mov", vec![eax()], vec![cell(8)])),
            ret(),
        ];
        assert!(has(&flat(true, 8, flags, IndexMap::default()), "push ebp"));
        assert!(!has(&flat(true, 8, read(), IndexMap::default()), "push ebp"));
    }

    /// The frame's reserve was rounded to a word: a flat target keeps its stack
    /// in dwords, and `sub esp, 70` left the next push misaligned.
    #[test]
    fn test_the_reserve_is_a_multiple_of_the_targets_slot() {
        let r#move = insn(0, semantics(Operation::Move, "mov", vec![ax()], vec![through_bp()]));
        let leave = insn(1, semantics(Operation::Return, "ret", vec![], vec![]));
        let body = lir::LirBody::new(
            "get",
            1,
            vec![lir::LirBlock::new(1, vec![r#move, leave]).into()],
            IndexMap::default(),
            IndexMap::default(),
        );
        let registers = llrm_target::FrameRegisters {
            pointer: RegId::EBP,
            stack: RegId::ESP,
            saved: Vec::new(),
            slot: 4,
            align: 4,
            optional: false,
            enter: false,
            wrap_split_size: false,
            slot_scratch: None,
            free: false,
        };
        let procedure = Procedure {
            name: "_get".into(),
            public: true,
            far: false,
            body,
            reserve: 70,
            callees: IndexMap::default(),
            interrupt: None,
            size: false,
            entry: 0,
            stack_check: None,
            probe_stack: None,
            registers,
        };
        let lines: Vec<String> =
            _procedure(&procedure, &no_names(), 0).unwrap().iter().map(|line| line.trim().to_owned()).collect();
        assert!(has(&lines, "sub esp, 72"), "{lines:?}");
    }

    /// SI and DI were pushed and popped whole: an operand-size prefix on every
    /// save and restore, for upper halves no Borland caller keeps across a
    /// call.
    #[test]
    fn test_callee_saves_only_what_the_convention_keeps() {
        let lines = _printed(vec![Loc::Reg(ir::Reg { register: RegId::ESI, width: 4 })], 0);
        assert!(has(&lines, "push si") && has(&lines, "pop si"));
        assert!(!has(&lines, "push esi") && !has(&lines, "pop esi"));
    }

    /// `place` saved SI on every call, entered or not, though only its loop
    /// names it: the early `return 1` pushed and popped it for nothing. The
    /// save is where the loop starts.
    #[test]
    fn test_a_register_only_a_later_block_names_is_saved_there() {
        let branch = Semantics { target: Some(3), ..semantics(Operation::Branch, "je", vec![], vec![]) };
        let si = || Loc::Reg(ir::Reg { register: RegId::SI, width: 2 });
        let blocks = vec![
            lir::LirBlock {
                succ: vec![3, 2].into(),
                ..lir::LirBlock::new(
                    1,
                    vec![insn(1, semantics(Operation::Move, "mov", vec![ax()], vec![through_bp()])), insn(2, branch)],
                )
            },
            lir::LirBlock::new(
                2,
                vec![
                    insn(3, semantics(Operation::Move, "mov", vec![si()], vec![through_bp()])),
                    insn(4, semantics(Operation::Return, "ret", vec![], vec![])),
                ],
            ),
            lir::LirBlock::new(3, vec![insn(5, semantics(Operation::Return, "ret", vec![], vec![]))]),
        ];
        let body =
            lir::LirBody::new("get", 1, crate::model::lir::shared(blocks), IndexMap::default(), IndexMap::default());
        let procedure = Procedure {
            name: "_get".into(),
            public: true,
            far: false,
            body,
            reserve: 0,
            callees: IndexMap::default(),
            interrupt: None,
            size: false,
            entry: 0,
            stack_check: None,
            probe_stack: None,
            registers: llrm_target::Target::frame_registers(&llrm_x86_m16::M16),
        };
        let lines: Vec<String> =
            _procedure(&procedure, &no_names(), 0).unwrap().iter().map(|line| line.trim().to_owned()).collect();
        let at = |text: &str| lines.iter().position(|one| one == text).unwrap_or_else(|| panic!("{text} in {lines:?}"));
        assert_eq!(lines.iter().filter(|one| *one == "push si").count(), 1, "{lines:?}");
        assert!(at("push si") > at("L0_2:") && at("push si") < at("L0_3:"), "{lines:?}");
        // The return that path makes pops it; the early one has nothing to pop.
        assert_eq!(lines.iter().filter(|one| *one == "pop si").count(), 1, "{lines:?}");
        assert!(at("pop si") < at("L0_3:"), "{lines:?}");
    }

    /// Reached around the block that names it as well, a return cannot pop what
    /// was not pushed: the late block pops at its end, before it falls into the
    /// return the early exit jumps to.
    #[test]
    fn test_a_return_reached_around_the_save_has_the_pop_at_the_end_of_the_late_path() {
        let branch = Semantics { target: Some(3), ..semantics(Operation::Branch, "je", vec![], vec![]) };
        let si = || Loc::Reg(ir::Reg { register: RegId::SI, width: 2 });
        let blocks = vec![
            lir::LirBlock { succ: vec![3, 2].into(), ..lir::LirBlock::new(1, vec![insn(1, branch)]) },
            lir::LirBlock {
                succ: vec![3].into(),
                ..lir::LirBlock::new(
                    2,
                    vec![insn(3, semantics(Operation::Move, "mov", vec![si()], vec![through_bp()]))],
                )
            },
            lir::LirBlock::new(3, vec![insn(5, semantics(Operation::Return, "ret", vec![], vec![]))]),
        ];
        let body =
            lir::LirBody::new("get", 1, crate::model::lir::shared(blocks), IndexMap::default(), IndexMap::default());
        let procedure = Procedure {
            name: "_get".into(),
            public: true,
            far: false,
            body,
            reserve: 0,
            callees: IndexMap::default(),
            interrupt: None,
            size: false,
            entry: 0,
            stack_check: None,
            probe_stack: None,
            registers: llrm_target::Target::frame_registers(&llrm_x86_m16::M16),
        };
        let lines: Vec<String> =
            _procedure(&procedure, &no_names(), 0).unwrap().iter().map(|line| line.trim().to_owned()).collect();
        let at = |wanted: &str| lines.iter().position(|one| one == wanted).unwrap();
        assert!(at("L0_2:") < at("push si"), "{lines:?}");
        assert!(at("pop si") < at("L0_3:"), "{lines:?}");
        assert_eq!(lines.iter().filter(|one| *one == "pop si").count(), 1, "{lines:?}");
    }

    /// peephole's multiply by three has no address, only base, index and scale.
    #[test]
    fn test_arithmetic_lea_scales_its_index() {
        let regs = crate::backend::registerinfo::test_regs();
        let r#where =
            ir::AddressRef { through: RegId::EBX, index_through: RegId::EBX, scale: 2, ..ir::AddressRef::new(None) };
        assert_eq!(_operand(regs, &Loc::Address(r#where), &no_names()).unwrap(), "[ebx+ebx*2]");
    }

    /// `lea eax, b+12[eax]`: a named address with an index register kept no
    /// symbol in its text (`lea eax, [eax]`), so `end_of(t) - t` was 0
    /// where it was 12 (the DAG selector's `sub(lea sym+12, sym)`,
    /// `--isel=dag`).
    #[test]
    fn test_a_named_address_with_an_index_register_prints_its_symbol() {
        let regs = crate::backend::registerinfo::test_regs();
        let mut names = IndexMap::default();
        names.insert((Space::Segment, 3), "L_b1".to_owned());
        let address = Addr { index: 3, ..Addr::new(Space::Segment, 12) };
        let r#where =
            ir::AddressRef { through: RegId::None, index_through: RegId::EAX, ..ir::AddressRef::new(Some(address)) };
        assert_eq!(_operand(regs, &Loc::Address(r#where), &names).unwrap(), "L_b1+12[eax]");
    }
}

#[cfg(test)]
mod split_tests {
    use super::*;

    fn fill(size: i64) -> Datum {
        Datum::Fill(Fill { size, byte: Some(0) })
    }

    /// A 100000-byte object was one 100000-byte segment, which OMF cannot hold;
    /// cut at 64K the first piece is full.
    #[test]
    fn an_object_past_64k_is_cut_into_full_pieces_and_a_rest() {
        let label = Datum::Label(Label { name: "big".to_owned() });
        let pieces = split_data(vec![label.clone(), Datum::Bytes(vec![1; 70000]), fill(30000)], 0x10000).unwrap();
        let sizes: Vec<i64> = pieces.iter().map(|piece| data_size(piece, 0)).collect();
        assert_eq!(sizes, [0x10000, 100000 - 0x10000]);
        assert_eq!(pieces[0][0], label);
    }

    #[test]
    fn a_pointer_across_a_boundary_is_refused() {
        let pointer = Datum::Pointer(Pointer { name: "x".to_owned(), offset: 0, far: false, bytes: 2 });
        assert!(split_data(vec![fill(0xFFFF), pointer], 0x10000).is_err());
    }
}
