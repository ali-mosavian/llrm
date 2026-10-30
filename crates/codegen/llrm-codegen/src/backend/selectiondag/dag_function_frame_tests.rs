//! Frame objects, cells and addresses, string operations and invariant loads.

use super::dag_function::selected;
use super::dag_function_tests::M16_STACK_ARGS;
use crate::model::ir::Operation;
use crate::testing::m16_ir;

const GLOBAL: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
@g = global i32 7
define i32 @f() {
entry:
  %v = load i32, ptr @g
  ret i32 %v
}
";

/// A load of a global was refused for 132 functions of the corpora ("operand
/// TargetGlobalAddress before its definition"); the symbol is the memory
/// operand's address.
#[test]
fn a_load_of_a_global_names_its_symbol() {
    let module = llrm_mir::parse::module(GLOBAL).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let first = done.body.insns()[0].what.clone().expect("semantics");
    // The symbol is the operand's address: folded into the load's cell, or (as
    // selected today, sdag-c's wrapper fold being open) moved into a register.
    let named = match &first.sources[0] {
        crate::model::ir::Loc::Mem(cell) => cell.addr,
        crate::model::ir::Loc::Imm(imm) => imm.address,
        other => panic!("{other:?}"),
    };
    assert!(named.is_some_and(|a| a.index == 0), "{first:?}");
}

/// A stack temporary is a cell below BP at `-depth`, each aligned and below
/// the last; the frame reports the depth the slots take. A slot at 0 would
/// overwrite the saved frame pointer.
#[test]
fn stack_temporaries_are_cells_below_the_frame_pointer() {
    use super::dag_function::{Frame, Temporaries};
    use super::emit::FrameMap;
    use super::x86_preprocess::FrameObjects;
    let frame = Frame::new(vec![8, 12], 0);
    let mut temps = Temporaries(&frame);
    let (first, second) = (temps.create_stack_temporary(4, 4), temps.create_stack_temporary(8, 8));
    let disp = |index| match frame.cell(index, 4).expect("a cell").addr {
        Some(addr) => addr.disp,
        None => panic!("a frame cell"),
    };
    assert_eq!((first, second), (0, 1));
    assert_eq!((disp(0), disp(1)), (-4, -16), "the 8-byte slot is aligned below the first");
    assert_eq!(disp(-1), 8, "the first parameter keeps its cell");
}

const ABSOLUTE: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i8 @f() {
entry:
  %v = load volatile i8, ptr inttoptr (i32 753664 to ptr)
  ret i8 %v
}
";

/// A load from an absolute address needs its four displacement bytes; with
/// none the encoder refused the function (`os_screen` stopped compiling).
#[test]
fn an_absolute_address_has_a_four_byte_displacement() {
    let module = llrm_mir::parse::module(ABSOLUTE).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let cell =
        done.body.insns().iter().filter_map(|i| i.what.clone()).flat_map(|w| w.sources).find_map(|loc| match loc {
            crate::model::ir::Loc::Mem(cell) => Some(cell),
            _ => None,
        });
    let cell = cell.expect("a memory operand");
    assert_eq!(
        (cell.addr.map(|a| (a.space, a.disp)), cell.disp_width),
        (Some((crate::model::ir::Space::Literal, 753664)), 4),
        "{cell:?}"
    );
}

const LOCAL: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i32 @f(i32 %a) {
entry:
  %p = alloca i32
  store i32 %a, ptr %p
  %v = load volatile i32, ptr %p
  ret i32 %v
}
";

/// A local is a cell below the frame pointer, written and read at one
/// displacement (the load is volatile: a plain one is forwarded from the
/// store); frame object 0 was `Emit(Unsupported(frame index 0))` in
/// 10 functions of the corpora.
#[test]
fn a_local_is_a_frame_cell_below_bp() {
    let module = llrm_mir::parse::module(LOCAL).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let cells: Vec<i64> = done
        .body
        .insns()
        .iter()
        .filter_map(|i| i.what.as_ref())
        .flat_map(|w| w.dests.iter().chain(w.sources.iter()))
        .filter_map(|loc| match loc {
            crate::model::ir::Loc::Mem(cell) => cell.addr.map(|a| a.disp),
            _ => None,
        })
        .collect();
    let locals: Vec<i64> = cells.iter().copied().filter(|d| *d < 0).collect();
    assert_eq!(locals, [-4, -4], "the store and the load: {cells:?}");
    assert_eq!(done.depth, 4);
    assert_eq!(done.extents, [(-4, 4)]);
}

const BIG_COPY: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
@a = global [200 x i8] zeroinitializer
@b = global [200 x i8] zeroinitializer
declare void @llvm.memcpy.p0.p0.i32(ptr, ptr, i32, i1)
declare void @llvm.memset.p0.i32(ptr, i8, i32, i1)
define void @f() {
entry:
  call void @llvm.memcpy.p0.p0.i32(ptr @a, ptr @b, i32 200, i1 false)
  call void @llvm.memset.p0.i32(ptr @a, i8 1, i32 200, i1 false)
  ret void
}
";

/// A long copy or fill is `rep movs` / `rep stos` with ECX, ESI and EDI set by
/// the glued copies: the emitter refused `REP_MOVSB_32` and `REP_STOSB_32` in
/// 14 programs of the m32 execution run.
#[test]
fn a_long_copy_and_fill_are_rep_string_operations() {
    let module = llrm_mir::parse::module(BIG_COPY).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let strings: Vec<_> = insns
        .iter()
        .filter_map(|i| i.what.as_ref())
        .filter(|w| matches!(
            w.op,
            crate::model::ir::Operation::Copy | crate::model::ir::Operation::Fill
        ))
        .collect();
    assert_eq!(strings.len(), 2, "{strings:?}");
    assert!(strings.iter().all(|w| w.sources.len() == 3), "count, source and destination are operands: {strings:?}");
}

const FRAME_PLUS_ONE: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i32 @f() {
entry:
  %l = alloca [8 x i8], align 4
  store i32 5, ptr %l
  %p = getelementptr i8, ptr %l, i32 1
  %v = load i32, ptr %p
  ret i32 %v
}
";

/// A load at a frame object plus one was `[esp]`: the displacement was added to
/// an `offset` the cell's `addr` overrides (bitfield_overhang_init and four
/// more programs printed wrong values).
#[test]
fn a_frame_object_plus_a_displacement_keeps_the_displacement() {
    let module = llrm_mir::parse::module(FRAME_PLUS_ONE).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let disps: Vec<i64> = insns
        .iter()
        .filter_map(|i| i.what.as_ref())
        .flat_map(|w| w.dests.iter().chain(&w.sources))
        .filter_map(|l| match l {
            crate::model::ir::Loc::Mem(m) => m.addr.map(|a| a.disp),
            _ => None,
        })
        .collect();
    assert_eq!(disps.len(), 2, "{disps:?}");
    assert_eq!(disps[1] - disps[0], 1, "the load is one byte past the store: {disps:?}");
}

const INDEXED_LOAD: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i8 @f(ptr %s, i32 %n) {
entry:
  %a = getelementptr i8, ptr %s, i32 %n
  %b = getelementptr i8, ptr %a, i32 1
  %v = load i8, ptr %b
  ret i8 %v
}
";

/// A based cell with an index must carry a literal address: without one the
/// later passes read `[base]` and dropped the index, so `strlen_idiom` looped
/// forever.
#[test]
fn an_indexed_cell_carries_a_literal_address() {
    let module = llrm_mir::parse::module(INDEXED_LOAD).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let cell = insns
        .iter()
        .filter_map(|i| i.what.as_ref())
        .flat_map(|w| w.sources.iter())
        .find_map(|l| match l {
            crate::model::ir::Loc::Mem(m) if m.index.is_some() => Some(m.clone()),
            _ => None,
        })
        .expect("an indexed load");
    assert!(cell.base.is_some() && cell.addr.is_some(), "{cell:?}");
    assert_eq!(cell.offset, 1);
}

/// An index that makes up the rest of a displacement past the object
/// (`[ebp+ecx+126]` for 8192 bytes below BP) is a frame position the layout
/// took for the arguments' and left unmoved, so fill_nest read 12 bytes off;
/// the cell now names the object it was made from, and the layout moves it with
/// the object. It was a `lea` of the object and a cell based on it (shellsort
/// m32 -O2: 14 `lea reg,[esp]` per iteration in matmul -O3, +25.7% executed
/// against the pin, which addresses `[esp+ecx*2-32]`).
#[test]
fn a_displacement_past_a_frame_object_names_the_object_it_was_made_from() {
    let text = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i32 @f() {
b1:
  %o = alloca [8192 x i8]
  br label %b10
b9:
  ret i32 %s2
b10:
  %n = phi i32 [ -8192, %b1 ], [ %nx, %b10 ]
  %acc = phi i32 [ %s2, %b10 ], [ 0, %b1 ]
  %g1 = getelementptr i8, ptr %o, i32 %n
  %g2 = getelementptr i8, ptr %g1, i32 8192
  %l1 = load i16, ptr %g2
  %g3 = getelementptr i8, ptr %o, i32 %n
  %g4 = getelementptr i8, ptr %g3, i32 8318
  %l2 = load i16, ptr %g4
  %x1 = sext i16 %l1 to i32
  %x2 = sext i16 %l2 to i32
  %s1 = add i32 %acc, %x1
  %s2 = add i32 %s1, %x2
  %nx = add nsw i32 %n, 128
  %exitcond = icmp ne i32 %nx, 0
  br i1 %exitcond, label %b10, label %b9
}
";
    let module = llrm_mir::parse::module(text).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let whats: Vec<_> = insns.iter().filter_map(|i| i.what.as_ref()).collect();
    assert!(!whats.iter().any(|w| w.op == crate::model::ir::Operation::Address), "no address of the object is made");
    let past = whats
        .iter()
        .flat_map(|w| w.sources.iter())
        .find_map(|l| match l {
            crate::model::ir::Loc::Mem(m) if m.index.is_some() && m.addr.is_some_and(|addr| addr.disp == 126) => {
                Some(m.clone())
            }
            _ => None,
        })
        .unwrap_or_else(|| {
            panic!(
                "the cell past the object: {:#?}",
                whats
                    .iter()
                    .flat_map(|w| w.sources.iter())
                    .filter(|l| matches!(l, crate::model::ir::Loc::Mem(m) if m.index.is_some()))
                    .collect::<Vec<_>>()
            )
        });
    let addr = past.addr.expect("a frame address");
    assert_eq!(addr.slot_home(), Some(-8192), "{past:?}");
}

const INT_TO_FLOAT: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define float @f(i32 %a) {
entry:
  %r = sitofp i32 %a to float
  ret float %r
}
";

/// A legalizer's scratch slot (an integer stored to convert it) is a frame
/// object the driver makes: the emitter refused its index as "frame index 6"
/// in 11 programs of the m32 execution run. Until x87 is emitted the function
/// is refused for its load, not for the slot.
#[test]
fn a_legalizers_stack_slot_is_a_frame_object() {
    let module = llrm_mir::parse::module(INT_TO_FLOAT).expect("parses");
    let why = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .err()
        .map(|e| e.0);
    assert!(why.as_deref().is_none_or(|why| !why.contains("frame index")), "{why:?}");
}

const BYTE_MULTIPLY: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define cc 1002 i8 @f(i8 zeroext %a, i8 zeroext %b) {
entry:
  %r = mul i8 %a, %b
  ret i8 %r
}
";

/// A byte multiply is `mul r8`: AL times the operand into AX. The copy of AL
/// the matcher makes was invisible to the emitter (its glue users came from
/// the DAG's use lists, which the matcher's nodes are not in, and its register
/// was a pin number), so idiom_edges and byte_multiply's `main` were refused.
#[test]
fn a_byte_multiply_is_mul_on_al() {
    let module = llrm_mir::parse::module(BYTE_MULTIPLY).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let mul = insns.iter().filter_map(|i| i.what.as_ref()).find(|w| w.name.as_deref() == Some("mul")).expect("a mul");
    assert!(mul.dests.iter().chain(&mul.sources).all(|l| matches!(l, crate::model::ir::Loc::Held(_))), "{mul:?}");
}

const GLOBAL_INDEXED: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
@a = global [64 x i8] zeroinitializer
define ptr @f(i32 %i, i32 %j) {
entry:
  %p = getelementptr i8, ptr @a, i32 %i
  %q = getelementptr i8, ptr %p, i32 %j
  ret ptr %q
}
define i8 @g(i32 %i) {
entry:
  %p = getelementptr i8, ptr @a, i32 %i
  %v = load i8, ptr %p
  ret i8 %v
}
";

/// A cell of a symbol has a displacement field to carry the relocation: a `lea`
/// of a symbol and two registers had none and the object encoder refused it
/// ("no field for", memfuncs_runtime did not assemble).
#[test]
fn a_symbol_cell_has_a_displacement_field() {
    use crate::model::ir::{Loc, Operation, Space};
    let module = llrm_mir::parse::module(GLOBAL_INDEXED).expect("parses");
    for name in ["g"] {
        let done =
            selected(&module, name, &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
                .unwrap_or_else(|e| panic!("{}", e.0));
        let insns = done.body.insns();
        let cells: Vec<_> = insns
            .iter()
            .filter_map(|i| i.what.as_ref())
            .flat_map(|w| w.sources.iter().chain(&w.dests))
            .filter_map(|l| match l {
                Loc::Mem(m) if m.addr.is_some_and(|a| a.space == Space::Segment) => Some(m.clone()),
                _ => None,
            })
            .collect();
        assert!(!cells.is_empty(), "{name} has a cell of the symbol");
        assert!(cells.iter().all(|m| m.disp_width >= 2), "{name}: {cells:?}");
        let _ = Operation::Move;
    }
}

/// The stack pointer is a word at real mode: the `add sp` after a call named
/// it a dword and the LIR verifier refused it (far_return -O0).
#[test]
fn the_stack_pointer_is_a_word_at_m16() {
    use crate::model::ir::{Loc, Operation};
    let module = llrm_mir::parse::module(M16_STACK_ARGS).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let adjust = insns
        .iter()
        .filter_map(|i| i.what.as_ref())
        .find(|w| {
            w.op == Operation::Binary
                && matches!(
                    &w.dests[..],
                    [Loc::Reg(r)] if r.register == llrm_lir::registers::RegId::SP
                )
        })
        .expect("an add of the stack pointer");
    assert!(matches!(&adjust.dests[..], [Loc::Reg(r)] if r.width == 2), "{adjust:?}");
}

/// A load marked `!invariant.load` selected by the DAG selector carried no
/// flag to LIR (`MemInfo` had none), so MachineCSE and the sinker took it for a
/// load that a store may change: the pin's `visitLoad` marks it and
/// `isCSECandidate` takes it.
#[test]
fn a_load_marked_invariant_is_an_invariant_load_in_lir() {
    let flags = |mark: &str| {
        let text = format!(
            "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
@tab = global [4 x i32] [i32 1, i32 2, i32 3, i32 4]
define i32 @f(i32 %n) {{
entry:
  %p = getelementptr [4 x i32], ptr @tab, i32 0, i32 %n
  %a = load i32, ptr %p{mark}
  ret i32 %a
}}
!0 = !{{}}
"
        );
        let module = llrm_mir::parse::module(&text).expect("parses");
        let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
            .unwrap_or_else(|e| panic!("{}", e.0));
        done.body
            .blocks
            .iter()
            .flat_map(|block| block.insns.iter())
            .filter(|one| {
                one.what
                    .as_ref()
                    .is_some_and(
                        |what| what.op == Operation::Move
                            && matches!(
                                what.sources.as_slice(),
                                [crate::model::ir::Loc::Mem(cell)] if cell.addr.as_ref().is_some_and(|a| a.space == crate::model::ir::Space::Segment)
                            ),
                    )
            })
            .map(|one| one.invariant())
            .collect::<Vec<bool>>()
    };
    assert_eq!(flags(""), [false], "the premise: one load of the cell, not invariant");
    assert_eq!(flags(", !invariant.load !0"), [true]);
}

const M16_STACK: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32\"
define void @f(i32 %a, i64 %b, i32 %c) addrspace(1) {
entry:
  ret void
}
";

/// At m16 an `i32` is a register type, so a stack `i32` is one cell and an
/// `i64` two (the second half four bytes up). The driver counted two cells for
/// an `i32` and one for an `i64` (widest legal integer assumed 16 bits): the
/// high half of an `i64` argument was read from the next
/// parameter (regparm3_matrix_4/_22/_31 gave wrong values).
#[test]
fn an_i32_is_one_stack_cell_and_an_i64_two_at_m16() {
    let module = llrm_mir::parse::module(M16_STACK).expect("parses");
    let layout = llrm_mir::datalayout::DataLayout::parse(module.datalayout.as_deref().unwrap()).unwrap();
    let (id, _, function) = module.functions().next().expect("a function");
    let convention = crate::backend::isel::convention(&module, &layout, id, &llrm_x86_m16::M16).expect("a convention");
    let cells = super::dag_function::stack_cells(&module, function, &convention, 32);
    assert_eq!(cells.len(), 4, "a: one, b: two, c: one: {cells:?}");
    assert_eq!(cells[2] - cells[1], 4, "the high half of b is four bytes above its low half: {cells:?}");
    assert!(cells[3] > cells[2], "{cells:?}");
}

fn memmove_ir(metadata: &str) -> String {
    format!(
        "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
declare void @llvm.memmove.p0.p0.i32(ptr, ptr, i32, i1)
define void @f(ptr %d, ptr %s) {{
entry:
  call void @llvm.memmove.p0.p0.i32(ptr %d, ptr %s, i32 78000, i1 false){metadata}
  ret void
}}
!0 = !{{}}
"
    )
}

fn select_m32(ir: &str) -> Result<crate::backend::isel::Selected, String> {
    let module = llrm_mir::parse::module(ir).expect("parses");
    selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .map_err(|e| e.0)
}

/// A `memmove` a pass proved forward is `rep movsb`, one proved backward is
/// `std; rep movsd; cld` from the last dword, where the runtime has no
/// `memmove` (`copyw.nib`'s scroll up and down at m32); one proved neither way
/// is still refused. The backward copy went a byte at a time at m32: scroll
/// bq32 -Os ran 91270 instructions beside the old selector's 55165.
#[test]
fn a_memmove_with_a_proved_direction_is_a_string_copy() {
    assert!(select_m32(&memmove_ir(", !llrm.forward !0")).is_ok());
    let backward = select_m32(&memmove_ir(", !llrm.backward !0")).unwrap_or_else(|why| panic!("{why}"));
    assert!(backward.inline.values().any(|bytes| bytes[..] == [0xFD, 0xF3, 0xA5, 0xFC]), "{:?}", backward.inline);
    let refused = select_m32(&memmove_ir("")).err().expect("no direction proved");
    assert!(refused.contains("memmove"), "{refused}");
}

/// A backward `memmove` of a size that is no multiple of four goes a byte at a
/// time at m32: a flat target names no register to carry a dword copy's tail.
#[test]
fn a_backward_memmove_at_m32_of_an_odd_size_moves_bytes() {
    let ir = memmove_ir(", !llrm.backward !0").replace("78000", "78001");
    let backward = select_m32(&ir).unwrap_or_else(|why| panic!("{why}"));
    let bytes = [0x1E, 0x07, 0xFD, 0xF3, 0xA4, 0xFC];
    assert!(backward.inline.values().any(|code| code[..] == bytes), "{:?}", backward.inline);
}

const SCALED_INDEX_AT_M16: &str = "target datalayout = \"@m16-datalayout@-ni:2:3\"
declare cc 1006 void @\"_far_get@3\"(ptr, ptr addrspace(1))
declare cc 1006 i16 @\"_con_gfx_trim@3\"(ptr addrspace(1))
declare cc 1006 i16 @\"_con_back_load@3\"(ptr addrspace(1), ptr)
declare cc 1006 i16 @\"_con_chars_load@3\"(ptr addrspace(1), ptr)
declare cc 1006 void @\"_con_text@3\"(ptr addrspace(1), i32, i16, ptr)
declare cc 1006 void @\"_con_back_draw@3\"(ptr addrspace(1), i32, i16)
declare i16 @_strlen(ptr) addrspace(1)
declare ptr @_strcpy(ptr, ptr) addrspace(1)
declare i16 @llvm.smin.i16(i16, i16)
define cc 1005 void @\"_con_draw@3\"(ptr addrspace(1) noundef nocapture %0, ptr noundef %1, i32 noundef %2, i16 noundef %3, float noundef %4) addrspace(1) \"llrm-fp-eval\"=\"x87-extended\" {
b1:
  %5 = alloca [82 x i8]
  %6 = alloca [80 x i8]
  %7 = getelementptr inbounds nuw i8, ptr addrspace(1) %0, i16 2
  %8 = load float, ptr addrspace(1) %7
  %9 = fcmp ole float %8, 0.000000e+00
  br i1 %9, label %b3, label %b2

b2:
  %10 = tail call cc 1006 i16 @\"_con_back_load@3\"(ptr addrspace(1) noundef %0, ptr noundef %1)
  %11 = icmp eq i16 %10, 0
  br i1 %11, label %b9, label %b18

b3:
  %12 = getelementptr inbounds nuw i8, ptr addrspace(1) %0, i16 8
  %13 = load i16, ptr addrspace(1) %12
  %14 = icmp slt i16 %13, 4
  %15 = select i1 %14, i16 %13, i16 4
  %16 = tail call cc 1006 i16 @\"_con_gfx_trim@3\"(ptr addrspace(1) noundef %0)
  %17 = icmp eq i16 %16, 0
  br i1 %17, label %b9, label %b7

b7:
  %18 = tail call cc 1006 i16 @\"_con_chars_load@3\"(ptr addrspace(1) noundef %0, ptr noundef %1)
  %19 = icmp ne i16 %18, 0
  %20 = icmp sgt i16 %15, 0
  %or.cond = select i1 %19, i1 %20, i1 false
  br i1 %or.cond, label %110, label %b9

b9:
  ret void

b14:
  %21 = phi i16 [ %114, %110 ], [ %45, %b17 ]
  %22 = phi i16 [ %113, %110 ], [ %44, %b17 ]
  %23 = phi i16 [ 0, %110 ], [ %43, %b17 ]
  %24 = getelementptr i8, ptr addrspace(1) %0, i16 6
  %25 = load i16, ptr addrspace(1) %24
  %26 = add i16 %21, %25
  %27 = icmp slt i16 %26, 0
  %28 = select i1 %27, i16 15, i16 0
  %29 = add i16 %28, %25
  %30 = add i16 %29, %21
  %31 = lshr i16 %30, 4
  %32 = shl nuw i16 %31, 4
  %33 = sub i16 %25, %32
  %34 = add i16 %21, %33
  %35 = shl nsw i16 %34, 2
  %36 = getelementptr i8, ptr addrspace(1) %0, i16 %35
  %37 = getelementptr i8, ptr addrspace(1) %36, i16 1292
  %38 = load float, ptr addrspace(1) %37
  %39 = fcmp ole float %38, 0.000000e+00
  br i1 %39, label %b17, label %b15

b15:
  %40 = mul nsw i16 %34, 80
  %41 = getelementptr inbounds i8, ptr addrspace(1) %111, i16 %40
  call cc 1006 void @\"_far_get@3\"(ptr noundef %5, ptr addrspace(1) noundef %41)
  call cc 1006 void @\"_con_text@3\"(ptr addrspace(1) noundef %0, i32 noundef %2, i16 noundef %23, ptr noundef %5)
  %42 = add nsw i16 %23, 8
  br label %b17

b17:
  %43 = phi i16 [ %23, %b14 ], [ %42, %b15 ]
  %44 = add i16 %22, -1
  %45 = add i16 %21, 1
  %46 = icmp samesign ugt i16 %44, 1
  br i1 %46, label %b14, label %b9

b18:
  %47 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %48 = load float, ptr addrspace(1) %47
  %49 = fptosi float %48 to i16
  %50 = add nsw i16 %49, -200
  tail call cc 1006 void @\"_con_back_draw@3\"(ptr addrspace(1) noundef %0, i32 noundef %2, i16 noundef %50)
  %51 = icmp slt i16 %49, 16
  br i1 %51, label %b9, label %b20

b20:
  %52 = tail call cc 1006 i16 @\"_con_chars_load@3\"(ptr addrspace(1) noundef %0, ptr noundef %1)
  %53 = icmp eq i16 %52, 0
  br i1 %53, label %b9, label %b22

b22:
  %54 = add nsw i16 %49, -16
  %55 = lshr i16 %54, 3
  %56 = getelementptr inbounds nuw i8, ptr addrspace(1) %0, i16 8
  %57 = load i16, ptr addrspace(1) %56
  %58 = getelementptr inbounds nuw i8, ptr addrspace(1) %0, i16 10
  %59 = load i16, ptr addrspace(1) %58
  %60 = sub nsw i16 %57, %59
  %61 = icmp sgt i16 %55, %60
  %62 = select i1 %61, i16 %60, i16 %55
  %63 = icmp sgt i16 %62, 0
  br i1 %63, label %115, label %b27

b27:
  store i8 93, ptr %5
  %64 = getelementptr inbounds nuw i8, ptr addrspace(1) %0, i16 1356
  call cc 1006 void @\"_far_get@3\"(ptr noundef %6, ptr addrspace(1) noundef nonnull %64)
  %65 = icmp slt i16 %3, 0
  %66 = select i1 %65, i16 7, i16 0
  %67 = add i16 %3, %66
  %68 = ashr i16 %67, 3
  %69 = call addrspace(1) i16 @_strlen(ptr noundef nonnull %6)
  %70 = add nsw i16 %68, -4
  %71 = icmp sgt i16 %69, %70
  %72 = sub nsw i16 %69, %70
  %73 = select i1 %71, i16 %72, i16 0
  %74 = getelementptr inbounds nuw i8, ptr %6, i16 %73
  %75 = getelementptr inbounds nuw i8, ptr %5, i16 1
  %76 = call addrspace(1) ptr @_strcpy(ptr noundef nonnull %75, ptr noundef nonnull %74)
  %77 = call addrspace(1) i16 @_strlen(ptr noundef nonnull %5)
  %78 = fmul float %4, 4.000000e+00
  %79 = fptosi float %78 to i32
  %80 = trunc i32 %79 to i8
  %81 = and i8 %80, 1
  %82 = or i8 %81, 10
  %83 = getelementptr i8, ptr %5, i16 %77
  store i8 %82, ptr %83
  %84 = getelementptr i8, ptr %83, i16 1
  store i8 0, ptr %84
  call cc 1006 void @\"_con_text@3\"(ptr addrspace(1) noundef %0, i32 noundef %2, i16 noundef %54, ptr noundef %5)
  br label %b9

b28:
  %85 = phi i16 [ %124, %115 ], [ %108, %b28 ]
  %86 = phi i16 [ %123, %115 ], [ %107, %b28 ]
  %87 = phi i16 [ %117, %115 ], [ %106, %b28 ]
  %88 = getelementptr i8, ptr addrspace(1) %0, i16 6
  %89 = load i16, ptr addrspace(1) %88
  %90 = getelementptr i8, ptr addrspace(1) %0, i16 10
  %91 = load i16, ptr addrspace(1) %90
  %92 = sub i16 %89, %91
  %93 = add i16 %85, %92
  %94 = icmp slt i16 %93, 0
  %95 = select i1 %94, i16 15, i16 0
  %96 = sub i16 %89, %91
  %97 = add i16 %95, %89
  %98 = add i16 %97, %85
  %99 = sub i16 %98, %91
  %100 = lshr i16 %99, 4
  %101 = shl nuw i16 %100, 4
  %102 = sub i16 %96, %101
  %103 = add i16 %85, %102
  %104 = mul nsw i16 %103, 80
  %105 = getelementptr inbounds i8, ptr addrspace(1) %118, i16 %104
  call cc 1006 void @\"_far_get@3\"(ptr noundef %5, ptr addrspace(1) noundef %105)
  call cc 1006 void @\"_con_text@3\"(ptr addrspace(1) noundef %0, i32 noundef %2, i16 noundef %87, ptr noundef %5)
  %106 = add nsw i16 %87, 8
  %107 = add i16 %86, -1
  %108 = add i16 %85, 1
  %109 = icmp samesign ugt i16 %107, 1
  br i1 %109, label %b28, label %b27

110:
  %111 = getelementptr inbounds nuw i8, ptr addrspace(1) %0, i16 12
  %112 = call i16 @llvm.smin.i16(i16 %13, i16 4)
  %113 = add nsw i16 %112, 1
  %114 = sub i16 16, %112
  br label %b14

115:
  %116 = shl nuw nsw i16 %62, 3
  %117 = sub nsw i16 %54, %116
  %118 = getelementptr inbounds nuw i8, ptr addrspace(1) %0, i16 12
  %119 = add i16 %49, -16
  %120 = lshr i16 %119, 3
  %121 = sub i16 %57, %59
  %122 = call i16 @llvm.smin.i16(i16 %120, i16 %121)
  %123 = add nsw i16 %122, 1
  %124 = sub i16 16, %122
  br label %b28
}
";

/// A scaled index beside a base is the 386's 32-bit address form. The `lea`
/// kept a literal place (`abs`) beside the scale, which no encoding takes: the
/// QCport modules console and screen failed to compile once their float
/// functions were selected (`[abs+bx+si*8]`, `_con_draw@3`).
#[test]
fn a_scaled_lea_at_m16_has_no_literal_place() {
    use crate::model::ir::{Loc, Operation};
    let module = llrm_mir::parse::module(m16_ir(SCALED_INDEX_AT_M16)).expect("parses");
    let done =
        selected(&module, "_con_draw@3", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
            .unwrap_or_else(|e| panic!("{}", e.0));
    let mut scaled = 0;
    for insn in done.body.insns().iter() {
        let Some(what) = insn.what.as_ref().filter(|w| w.op == Operation::Address) else { continue };
        let (scale, place) = match &what.sources[..] {
            [Loc::Address(a)] => (a.scale, a.addr),
            [Loc::Mem(m)] => (m.scale, m.addr),
            _ => continue,
        };
        scaled += 1;
        assert!(scale == 1 || place.is_none(), "{what:?}");
    }
    // (its word indexes are shifted first, so none stays scaled)
    assert!(scaled > 0, "the function has a lea");
}

/// 16-bit addressing has no scale. A `lea` of a base and a word index scaled by
/// two kept the scale on the word registers; the encoder dropped it, so the
/// address lost its doubling (Nib `floats.shift` moved the wrong limbs, and
/// `print(1.989e30)` printed `1.988999999999999901909255192576e+30`).
#[test]
fn a_lea_at_m16_does_not_scale_a_word_index() {
    use crate::model::ir::{Loc, Operation};
    let module = llrm_mir::parse::module(
        "target datalayout = \"e-p:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3\"
define ptr @f(ptr %p, i16 %i, i1 %c) {
  %w = lshr i16 %i, 4
  br i1 %c, label %a, label %b
a:
  %m = mul nsw i16 %w, -2
  %q = getelementptr i8, ptr %p, i16 %m
  %r = getelementptr i8, ptr %q, i16 574
  ret ptr %r
b:
  ret ptr %p
}
",
    )
    .expect("parses");
    let done = selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let mut leas = 0;
    for insn in done.body.insns().iter() {
        let Some(what) = insn.what.as_ref().filter(|w| w.op == Operation::Address) else { continue };
        let [Loc::Mem(cell)] = &what.sources[..] else { continue };
        leas += 1;
        let word_index = cell.index.is_some_and(|index| index.width == 2);
        assert!(cell.scale == 1 || !word_index, "a word index scaled by {}: {what:?}", cell.scale);
    }
    assert!(leas > 0, "the function has a lea");
}

/// A negative displacement of a cell is negative. It was the constant's 32 bits
/// unsigned (`[bx+si+4294967292]`), which the encoder wrote as a word where a
/// byte would do: `-4` cost a byte in every limb loop of Nib `floats`.
#[test]
fn a_negative_displacement_at_m16_is_negative() {
    use crate::model::ir::Loc;
    let module = llrm_mir::parse::module(
        "target datalayout = \"e-p:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3\"
define i16 @f(ptr %p, i16 %i) {
  %q = getelementptr i8, ptr %p, i16 %i
  %r = getelementptr i8, ptr %q, i16 -4
  %v = load i16, ptr %r
  ret i16 %v
}
",
    )
    .expect("parses");
    let done = selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let mut cells = 0;
    for insn in done.body.insns().iter() {
        let Some(what) = insn.what.as_ref() else { continue };
        for source in &what.sources {
            let Loc::Mem(cell) = source else { continue };
            if cell.index.is_none() {
                continue;
            }
            cells += 1;
            assert_eq!(cell.offset, -4, "{what:?}");
        }
    }
    assert!(cells > 0, "the function reads an indexed cell");
}

/// `fexp2` pushes two slots above its operand and `flog2` one: with six doubles
/// live the `fld1` overflowed the x87 stack (`pow_stack_depth.bas` died with
/// "Overflow" at run time under `--isel=dag`). The sequence says it clobbers
/// the stack's top slots, as the old selector marks it.
#[test]
fn a_sequence_that_pushes_above_its_operand_clobbers_the_top_stack_registers() {
    use llrm_lir::registers::RegId;
    let text = "target datalayout = \"e-p:32:32-i64:32-f80:32-n8:16:32\"
declare double @llvm.exp2.f64(double)
declare double @llvm.log2.f64(double)
define double @f(double %x) {
  %a = call double @llvm.exp2.f64(double %x)
  %b = call double @llvm.log2.f64(double %a)
  ret double %b
}
";
    let module = llrm_mir::parse::module(text).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let clobbers = |name: &str| -> Vec<RegId> {
        let one = done
            .body
            .blocks
            .iter()
            .flat_map(|block| block.insns.iter())
            .find(|one| one.what.as_ref().and_then(|what| what.name.as_deref()) == Some(name))
            .unwrap_or_else(|| panic!("no {name}"));
        one.clobbers.iter().copied().collect()
    };
    assert_eq!(clobbers("fexp2"), [RegId::MM5, RegId::MM6]);
    assert_eq!(clobbers("flog2"), [RegId::MM6]);
}

const M16_CLEAR_16: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32\"
@g = global [16 x i8] zeroinitializer
declare void @llvm.memset.p0.i16(ptr, i8, i16, i1)
define void @f() {
entry:
  call void @llvm.memset.p0.i16(ptr @g, i8 0, i16 16, i1 false)
  ret void
}
";

const M16_CLEAR_54: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32\"
@g = global [54 x i8] zeroinitializer
declare void @llvm.memset.p0.i16(ptr, i8, i16, i1)
define void @f() {
entry:
  call void @llvm.memset.p0.i16(ptr @g, i8 0, i16 54, i1 false)
  ret void
}
";

/// A QB function with three `DIM x(3) AS DOUBLE` cleared its 54 bytes with 14
/// dword stores at -Os, `nbody.bas` +309 B over the old selector: the size
/// limit of an inline fill read the `optsize` attribute, which no front end
/// sets, and not the build's level. At -Os the 54 bytes are one `rep stos`.
#[test]
fn at_os_a_54_byte_clear_is_a_rep_string_operation() {
    let module = llrm_mir::parse::module(M16_CLEAR_54).expect("parses");
    let mut cpu = crate::backend::cpu::profile("386").expect("a cpu").clone();
    cpu.size = true;
    let done = selected(&module, "f", &llrm_x86_m16::M16, &cpu, &[]).unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let fills =
        insns.iter().filter_map(|i| i.what.as_ref()).filter(|w| w.op == crate::model::ir::Operation::Fill).count();
    let stores = insns
        .iter()
        .filter_map(|i| i.what.as_ref())
        .filter(|w| matches!(w.dests.as_slice(), [crate::model::ir::Loc::Mem(_)]))
        .count();
    assert_eq!((fills, stores), (1, 0), "a fill and no store");
}

const M16_CLEAR_24: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-i32:16-i64:16-n8:16:32\"

@g = global [24 x i8] zeroinitializer

declare void @llvm.memset.p0.i16(ptr, i8, i16, i1)

define void @f() {
entry:
  call void @llvm.memset.p0.i16(ptr @g, i8 0, i16 24, i1 false)
  ret void
}
";

/// A 24-byte local array cleared by six dword stores at -Os (24 B of code)
/// where the old selector's `rep stos` is 12: the inline limit was the pin's
/// 8 stores, not the target's size-tuned ratio (nbody_single.nib: 96 B of
/// `mov [bp-N],esi` for four arrays, and 610 more `mov` stores over the
/// corpus under dag). Real mode takes four stores inline at -Os.
#[test]
fn at_os_a_24_byte_clear_is_a_rep_string_operation() {
    let module = llrm_mir::parse::module(M16_CLEAR_24).expect("parses");
    let mut cpu = crate::backend::cpu::profile("486").expect("a cpu").clone();
    cpu.size = true;
    let done = selected(&module, "f", &llrm_x86_m16::M16, &cpu, &[]).unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let fills =
        insns.iter().filter_map(|i| i.what.as_ref()).filter(|w| w.op == crate::model::ir::Operation::Fill).count();
    let stores = insns
        .iter()
        .filter_map(|i| i.what.as_ref())
        .filter(|w| matches!(w.dests.as_slice(), [crate::model::ir::Loc::Mem(_)]))
        .count();
    assert_eq!((fills, stores), (1, 0), "a fill and no store");
}

const M16_COPY_12: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-i32:16-i64:16-n8:16:32\"

@a = global [12 x i8] zeroinitializer
@b = global [12 x i8] zeroinitializer

declare void @llvm.memcpy.p0.p0.i16(ptr, ptr, i16, i1)

define void @f() {
entry:
  call void @llvm.memcpy.p0.p0.i16(ptr @a, ptr @b, i16 12, i1 false)
  ret void
}
";

/// A 12-byte struct copy (a `vec3` of floats) was three dword load and store
/// pairs at -Os, 24 bytes of code where `rep movs` is about 16 (QCport
/// `_ent_mover_push`: 21 stores and 15 loads). Real mode takes two pairs
/// inline at size.
#[test]
fn at_os_a_12_byte_copy_is_a_rep_string_operation() {
    let module = llrm_mir::parse::module(M16_COPY_12).expect("parses");
    let mut cpu = crate::backend::cpu::profile("486").expect("a cpu").clone();
    cpu.size = true;
    let done = selected(&module, "f", &llrm_x86_m16::M16, &cpu, &[]).unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let copies =
        insns.iter().filter_map(|i| i.what.as_ref()).filter(|w| w.op == crate::model::ir::Operation::Copy).count();
    let stores = insns
        .iter()
        .filter_map(|i| i.what.as_ref())
        .filter(|w| matches!(w.dests.as_slice(), [crate::model::ir::Loc::Mem(_)]))
        .count();
    assert_eq!((copies, stores), (1, 0), "a copy and no store");
}

/// A 16-byte clear at m16 was eight word stores: the widest store was the
/// pointer's. The pin's `getOptimalMemOpType` gives `i32` there, and clang
/// `-m16 -O2` clears 16 bytes with four `movl $0` (rep_count: 1046 bytes
/// against the old selector's 486).
#[test]
fn a_16_byte_clear_at_m16_is_four_dword_stores() {
    let module = llrm_mir::parse::module(M16_CLEAR_16).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let stores: Vec<u32> = done
        .body
        .insns()
        .iter()
        .filter_map(|i| i.what.as_ref())
        .filter(|w| w.name.as_deref() == Some("mov"))
        .filter_map(|w| match w.dests.as_slice() {
            [crate::model::ir::Loc::Mem(cell)] => Some(cell.width),
            _ => None,
        })
        .collect();
    assert_eq!(stores, [4, 4, 4, 4], "store widths");
}

const VOLATILE_COPY: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
@a = global [7 x i8] zeroinitializer
@b = global [7 x i8] zeroinitializer
declare void @llvm.memcpy.p0.p0.i32(ptr, ptr, i32, i1)
define void @f() {
entry:
  call void @llvm.memcpy.p0.p0.i32(ptr @a, ptr @b, i32 7, i1 true)
  ret void
}
";

/// A volatile `memcpy` was refused ("volatile memory intrinsic"). Its accesses
/// stay as written: seven bytes are a dword, a word and a byte, not two
/// overlapping dwords as a plain copy makes them.
#[test]
fn a_volatile_copy_is_the_accesses_it_names_and_no_more() {
    // The profile prices on the 16-bit machine, whose table limits a copy to
    // two moves at every level: this one is about volatile accesses, not that.
    llrm_x86_m16::with_pin_tuning(|| {
        let module = llrm_mir::parse::module(VOLATILE_COPY).expect("parses");
        let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
            .unwrap_or_else(|e| panic!("{}", e.0));
        let mut widths: Vec<u32> = done
            .body
            .insns()
            .iter()
            .filter_map(|i| i.what.as_ref())
            .filter(|w| w.name.as_deref() == Some("mov"))
            .filter_map(|w| match w.dests.as_slice() {
                [crate::model::ir::Loc::Mem(cell)] => Some(cell.width),
                _ => None,
            })
            .collect();
        widths.sort_unstable();
        assert_eq!(widths, [1, 2, 4], "the stores: {widths:?}");
        assert!(done.body.insns().iter().filter(|i| i.volatile).count() >= 3, "volatile accesses");
    });
}

const VOLATILE_LOAD: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
@v = global i32 0
@w = global i32 0
define void @f() {
entry:
  %a = load volatile i32, ptr @v
  store volatile i32 %a, ptr @w
  ret void
}
";

/// The DAG path dropped the volatile mark on the LIR instruction of a volatile
/// load or store: the machine passes that skip volatile accesses
/// (`MachineLICM`, `MachineCSE`, load folding, the store combiner) saw plain
/// ones. (A volatile load the matcher folds into an ALU instruction still
/// loses it: the folded node is gone.)
#[test]
fn a_volatile_load_and_store_are_volatile_instructions() {
    let module = llrm_mir::parse::module(VOLATILE_LOAD).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let volatile = done.body.insns().iter().filter(|i| i.volatile).count();
    assert_eq!(volatile, 2, "the load and the store");
}

const ALLOCA_COUNT: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i32 @f() {
entry:
  %p = alloca i32, i32 4
  store i32 7, ptr %p
  %v = load i32, ptr %p
  ret i32 %v
}
";

/// An `alloca i32, i32 4` was refused ("an alloca with a count"); the frame
/// object is the type's size times the constant count, as
/// `FunctionLoweringInfo` has it.
#[test]
fn an_alloca_with_a_constant_count_is_that_many_elements() {
    let module = llrm_mir::parse::module(ALLOCA_COUNT).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    assert_eq!(done.depth, 16);
    assert_eq!(done.extents, [(-16, 16)]);
}

const VOLATILE_FOLD: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
@v = global i32 0
define i32 @f(i32 %a) {
entry:
  %b = load volatile i32, ptr @v
  %s = add i32 %a, %b
  ret i32 %s
}
";

/// A volatile load the matcher folds into an `add` (`add eax, [v]`) lost its
/// mark: the folded node was gone, and `MachineLICM`, `MachineCSE` and the
/// coalescer saw a plain instruction. The machine node carries the flag of the
/// memory operands it folded (`MachineMemOperand::isVolatile`).
#[test]
fn a_volatile_load_folded_into_an_add_is_a_volatile_instruction() {
    let module = llrm_mir::parse::module(VOLATILE_FOLD).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let folded: Vec<_> = insns
        .iter()
        .filter(|i| {
            i.what.as_ref().is_some_and(|w| {
                w.name.as_deref() == Some("add") && w.sources.iter().any(|s| matches!(s, crate::model::ir::Loc::Mem(_)))
            })
        })
        .collect();
    assert_eq!(folded.len(), 1, "premise: the load is folded into the add");
    assert!(folded[0].volatile, "the folded add is volatile");
}

const M16_CLEAR_FRAME_18: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-i32:16-i64:16-n8:16:32\"
@g = global [18 x i8] zeroinitializer
declare void @llvm.memset.p0.i16(ptr, i8, i16, i1)
declare void @use(ptr)
define void @f() {
entry:
  %a = alloca [18 x i8], align 2
  call void @llvm.memset.p0.i16(ptr %a, i8 0, i16 18, i1 false)
  call void @use(ptr %a)
  ret void
}
define void @h() {
entry:
  call void @llvm.memset.p0.i16(ptr @g, i8 0, i16 18, i1 false)
  ret void
}
";

/// Counts of the fills and of the stores a function selects to at -Os, m16.
fn fills_and_stores(name: &str) -> (usize, usize) {
    let module = llrm_mir::parse::module(M16_CLEAR_FRAME_18).expect("parses");
    let mut cpu = crate::backend::cpu::profile("386").expect("a cpu").clone();
    cpu.size = true;
    let done = selected(&module, name, &llrm_x86_m16::M16, &cpu, &[]).unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let whats = || insns.iter().filter_map(|i| i.what.as_ref());
    let fills = whats().filter(|w| w.op == crate::model::ir::Operation::Fill).count();
    let stores = whats()
        .filter(|w| w.name.as_deref() == Some("mov") && matches!(w.dests.as_slice(), [crate::model::ir::Loc::Mem(_)]))
        .count();
    (fills, stores)
}

/// An 18-byte clear at m16 -Os was five stores through BP (`sieve.bas` +44 B
/// over the old selector, bench `sieve` 893 -> 937): the pin counts stores, but
/// in 16-bit code they are longer than the fill. `memset_stores_size` makes it
/// a fill, for a local and for a global.
#[test]
fn at_os_an_18_byte_clear_is_a_fill() {
    assert_eq!(fills_and_stores("f").0, 1, "a fill for the local");
    assert_eq!(fills_and_stores("h").0, 1, "a fill for the global");
}

/// Fills and stores a `memset` of `length` bytes of `value` to a local becomes
/// at -Os, m16, on a 486.
fn memset_shape(
    length: u32,
    value: i32,
) -> (usize, usize) {
    let text = format!(
        "target datalayout = \"e-p:16:16-p1:32:16:16:16-i32:16-i64:16-n8:16:32\"
declare void @llvm.memset.p0.i16(ptr, i8, i16, i1)
declare void @use(ptr)
define void @f() {{
entry:
  %a = alloca [{length} x i8], align 2
  call void @llvm.memset.p0.i16(ptr %a, i8 {value}, i16 {length}, i1 false)
  call void @use(ptr %a)
  ret void
}}
"
    );
    let module = llrm_mir::parse::module(&text).expect("parses");
    let mut cpu = crate::backend::cpu::profile("486").expect("a cpu").clone();
    cpu.size = true;
    let done = selected(&module, "f", &llrm_x86_m16::M16, &cpu, &[]).unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let whats = || insns.iter().filter_map(|i| i.what.as_ref());
    let fills = whats().filter(|w| w.op == crate::model::ir::Operation::Fill).count();
    let stores = whats()
        .filter(|w| w.name.as_deref() == Some("mov") && matches!(w.dests.as_slice(), [crate::model::ir::Loc::Mem(_)]))
        .count();
    (fills, stores)
}

/// A 10-byte clear at m16 -Os was a `rep stosb` on bytes alone (19 B of
/// stores against 16): 57 clocks of a 486 for 3 bytes, 17 clocks a byte
/// against the target's 10.
#[test]
fn at_os_a_10_byte_clear_is_stores_not_rep_stos() {
    assert_eq!(memset_shape(10, 0).0, 0);
}

/// sc.c's 14-byte zero fill: 7 bytes saved for 66 clocks, 9.4 a byte, pay.
#[test]
fn at_os_a_14_byte_zero_fill_stays_rep_stos() {
    assert_eq!(memset_shape(14, 0).0, 1);
}

/// Past the break-even length the fill is smaller by more than it costs.
#[test]
fn at_os_a_long_fill_is_rep_stos() {
    assert_eq!(memset_shape(54, 0).0, 1);
}

/// A cell exactly one past a frame object's end is the next object's first
/// byte; it was tagged with the nearer slot (`home_of`: the next object), so
/// the relayout moved it under that object: matmul -O3 at m32 loaded
/// `[esp+eax]` where `[esp+eax+396]` was the first element of `a`, and
/// returned 318236 for 353712. The cell names the object it was made from.
#[test]
fn a_cell_one_past_a_frame_object_keeps_its_own_slot() {
    let text = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i32 @f() {
b1:
  %o = alloca [8192 x i8]
  %p = alloca [8192 x i8]
  br label %b10
b9:
  ret i32 %s1
b10:
  %n = phi i32 [ -8192, %b1 ], [ %nx, %b10 ]
  %acc = phi i32 [ %s1, %b10 ], [ 0, %b1 ]
  %g1 = getelementptr i8, ptr %p, i32 %n
  %g2 = getelementptr i8, ptr %g1, i32 8192
  %l1 = load i16, ptr %g2
  %x1 = sext i16 %l1 to i32
  %s1 = add i32 %acc, %x1
  %nx = add nsw i32 %n, 128
  %exitcond = icmp ne i32 %nx, 0
  br i1 %exitcond, label %b10, label %b9
}
";
    let module = llrm_mir::parse::module(text).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let cell = insns
        .iter()
        .filter_map(|i| i.what.as_ref())
        .flat_map(|w| w.sources.iter())
        .find_map(|l| match l {
            crate::model::ir::Loc::Mem(m) if m.index.is_some() => Some(m.clone()),
            _ => None,
        })
        .expect("an indexed load");
    let home = cell.addr.expect("a frame address").slot_home();
    let lowest = done.extents.iter().map(|(start, _)| *start).min();
    assert_eq!(home, lowest, "the cell belongs to %p, the lower object: {cell:?} in {:?}", done.extents);
}

/// `select (a < b), 1, 2` was `setl; mov 2; sub` (a load of the constant and a
/// subtract from it: bintree m32 -O2 +3.8% clocks against the pin); the
/// pin's `combineSubSetcc` makes it `setge; inc`, the inverted condition plus
/// `C - 1`.
#[test]
fn a_select_of_one_and_two_is_the_inverted_setcc_plus_one() {
    let text = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i32 @f(i32 %a, i32 %b) {
entry:
  %c = icmp slt i32 %a, %b
  %s = select i1 %c, i32 1, i32 2
  ret i32 %s
}
";
    let module = llrm_mir::parse::module(text).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let names: Vec<_> = done
        .body
        .insns()
        .iter()
        .filter_map(|i| i.what.as_ref())
        .map(|w| w.name.as_deref().unwrap_or("").to_owned())
        .collect();
    assert!(names.iter().any(|n| n == "setge"), "{names:?}");
    assert!(names.iter().any(|n| n == "inc"), "{names:?}");
}

/// At -O2 the same copy is `rep movsd` too: m16's table states the -Os limit
/// (2, SD225) at every level, where the pin's `MaxStoresPerMemcpy` (8) would
/// make three dword load and store pairs.
#[test]
fn at_o2_a_12_byte_copy_is_a_string_move_at_m16() {
    let module = llrm_mir::parse::module(M16_COPY_12).expect("parses");
    let cpu = crate::backend::cpu::profile("486").expect("a cpu").clone();
    let done = selected(&module, "f", &llrm_x86_m16::M16, &cpu, &[]).unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let copies =
        insns.iter().filter_map(|i| i.what.as_ref()).filter(|w| w.op == crate::model::ir::Operation::Copy).count();
    let stores = insns
        .iter()
        .filter_map(|i| i.what.as_ref())
        .filter(|w| matches!(w.dests.as_slice(), [crate::model::ir::Loc::Mem(_)]))
        .count();
    assert_eq!((copies, stores), (1, 0), "a string move and no stores");
}

const M16_FAR_COPY_12: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-i32:16-i64:16-n8:16:32\"

@far = addrspace(1) global [12 x i8] zeroinitializer
@near = global [12 x i8] zeroinitializer

declare void @llvm.memcpy.p0.p1.i16(ptr, ptr addrspace(1), i16, i1)

define void @f() {
entry:
  call void @llvm.memcpy.p0.p1.i16(ptr @near, ptr addrspace(1) @far, i16 12, i1 false)
  ret void
}
";

const M16_FAR_TO_FAR_COPY: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-i32:16-i64:16-n8:16:32\"

declare void @llvm.memcpy.p1.p1.i16(ptr addrspace(1), ptr addrspace(1), i16, i1)

define void @f(ptr addrspace(1) %d, ptr addrspace(1) %s) {
entry:
  call void @llvm.memcpy.p1.p1.i16(ptr addrspace(1) %d, ptr addrspace(1) %s, i16 12, i1 false)
  ret void
}
";

/// A 12-byte copy from a far pointer at -Os was `rep movsb` with cx = 12
/// (QCport atk_army): the near copy of the same size is `rep movsd` with cx =
/// 3, and the segment override does not change the unit.
#[test]
fn at_os_a_12_byte_far_copy_moves_dwords() {
    let module = llrm_mir::parse::module(M16_FAR_COPY_12).expect("parses");
    let mut cpu = crate::backend::cpu::profile("486").expect("a cpu").clone();
    cpu.size = true;
    let done = selected(&module, "f", &llrm_x86_m16::M16, &cpu, &[]).unwrap_or_else(|e| panic!("{}", e.0));
    let moves: Vec<String> = done
        .body
        .insns()
        .iter()
        .filter_map(|i| i.what.as_ref())
        .filter(|w| w.op == crate::model::ir::Operation::Copy)
        .filter_map(|w| w.name.as_deref().map(str::to_owned))
        .collect();
    assert_eq!(moves, ["movsd"], "one string move of dwords");
}

/// `(x >> 16)` of a loaded `i64` was `shrd` of the two halves loaded apart
/// (`f_lshr_16` at m16, 8 instructions against the pin's 6): the pin's
/// `visitFunnelShift` joins the funnel of consecutive loads into one load at
/// the byte offset, and the high half is a narrower load.
#[test]
fn a_funnel_of_consecutive_loads_is_one_load_at_the_offset() {
    let text = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i64 @f(ptr %p) {
entry:
  %v = load i64, ptr %p
  %r = lshr i64 %v, 16
  ret i64 %r
}
";
    let module = llrm_mir::parse::module(text).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let names: Vec<_> = done
        .body
        .insns()
        .iter()
        .filter_map(|i| i.what.as_ref())
        .map(|w| w.name.as_deref().unwrap_or("").to_owned())
        .collect();
    assert!(!names.iter().any(|n| n == "shrd"), "{names:?}");
}

/// A far string move left the dword each pointer was joined into for the string
/// registers (`or(movzx off, shl sel, 16)`) in the body, read by no one. Its
/// `shl` read the selector at four bytes, the allocator held the word in a
/// dword register, and QCport gib/ent -Os grew by dword spills and
/// `mov eax,[bp-N]; mov ax,..; mov [bp-N],eax` inserts (gib +161 B).
#[test]
fn a_far_string_move_leaves_no_dword_join_once_dead_code_is_gone() {
    let module = llrm_mir::parse::module(M16_FAR_TO_FAR_COPY).expect("parses");
    let mut cpu = crate::backend::cpu::profile("486").expect("a cpu").clone();
    cpu.size = true;
    let done = selected(&module, "f", &llrm_x86_m16::M16, &cpu, &[]).unwrap_or_else(|e| panic!("{}", e.0));
    let joins = |body: &crate::model::lir::LirBody| {
        body.insns().iter().filter_map(|i| i.what.as_ref()).filter(|w| w.name.as_deref() == Some("shl")).count()
    };
    assert!(joins(&done.body) > 0, "the premise: selection builds the joined dword");
    let cleaned = crate::backend::deadmi::eliminated(&done.body);
    assert_eq!(joins(&cleaned), 0, "the unread join is gone");
}

/// The register file was parsed for each block (`Target::scalar_sse` asked per
/// `select_block`): 226 M of the 490 M instructions of `dag isel` on joins128,
/// 147 k instructions for each of 1,542 blocks.
#[test]
fn the_register_file_is_not_parsed_per_block() {
    let parses = |blocks: usize| {
        let mut ir = String::from(
            "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"\ndefine i32 @f(i32 %x) {\nentry:\n  br label %b0\n",
        );
        for at in 0..blocks {
            let next = if at + 1 < blocks { format!("b{}", at + 1) } else { "end".to_owned() };
            ir.push_str(&format!("b{at}:\n  %v{at} = add i32 %x, {at}\n  br label %{next}\n"));
        }
        ir.push_str("end:\n  ret i32 %x\n}\n");
        let before = llrm_target::registers::parsed();
        select_m32(&ir).unwrap_or_else(|why| panic!("{why}"));
        llrm_target::registers::parsed() - before
    };
    // the first compile reads what the target caches once
    parses(3);
    assert_eq!(parses(30), parses(90));
}

/// A word divided by two is `x + (x >>u 15)` shifted once: the pin folds
/// `srl (sra x, y), 15` to `srl x, 15`. The DAG kept both shifts and widened
/// each (`movsx; sar 15; movzx; shr 15`), 2 more instructions in the bit
/// reversal of stft_fixed bq32 -Os.
#[test]
fn a_word_halved_signed_shifts_the_sign_down_once() {
    let ir = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i16 @f(i16 %x) {
  %r = sdiv i16 %x, 2
  ret i16 %r
}
";
    let done = select_m32(ir).unwrap_or_else(|why| panic!("{why}"));
    let shifts = done
        .body
        .insns()
        .iter()
        .filter_map(|one| one.what.as_ref())
        .filter(|w| matches!(w.name.as_deref(), Some("sar" | "shr")))
        .count();
    assert_eq!(shifts, 2, "{:?}", done.body.insns());
}

/// The pin's `(i16 0)` is the low part of a `MOV32r0`: `xor edi, edi` at m32,
/// 2 bytes. Narrowed to a word (`xor di, di`, 66-prefixed) it grew 1 B per
/// zero (QB `frames` -Os bq32).
#[test]
fn a_zero_word_at_m32_is_a_dword_xor() {
    use crate::model::ir::Loc;
    let ir = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i16 @f() {
  ret i16 0
}
";
    let done = select_m32(ir).unwrap_or_else(|why| panic!("{why}"));
    let zero = done.body.insns().iter().filter_map(|one| one.what.clone()).find(|what| {
        what.name.as_deref() == Some("mov") && matches!(what.sources.as_slice(), [Loc::Imm(imm)] if imm.value == 0)
    });
    let zero = zero.expect("a zero");
    assert!(matches!(zero.dests.as_slice(), [Loc::Held(held)] if held.width == 4), "{zero:?}");
}

/// The width in bytes of the register of the `rep stos` a `memset` of the size
/// `%s` (defined by `size`, from the argument `%n`) bytes of `value` becomes at
/// m16 on `cpu` (`None`: no fill).
fn dynamic_fill_unit_of(
    size: &str,
    value: u8,
    cpu: &str,
) -> Option<String> {
    let text = format!(
        "target datalayout = \"e-p:16:16-p1:32:16:16:16-i32:16-i64:16-n8:16:32\"
declare void @llvm.memset.p0.i16(ptr, i8, i16, i1)
define void @f(ptr %p, i16 %n) {{
entry:
  {size}
  call void @llvm.memset.p0.i16(ptr %p, i8 {value}, i16 %s, i1 false)
  ret void
}}
"
    );
    let module = llrm_mir::parse::module(&text).expect("parses");
    let cpu = crate::backend::cpu::profile(cpu).expect("a cpu");
    let done = selected(&module, "f", &llrm_x86_m16::M16, cpu, &[]).unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    insns
        .iter()
        .filter_map(|i| i.what.as_ref())
        .find(|w| w.op == crate::model::ir::Operation::Fill)
        .and_then(
            |w| w.sources
                .iter()
                .find_map(
                    |source| match source {
                        crate::model::ir::Loc::Held(held) => Some(held.width.to_string()),
                        _ => None,
                    },
                ),
        )
}

fn dynamic_fill_unit(shift: u32) -> Option<String> {
    dynamic_fill_unit_of(&format!("%s = shl nuw i16 %n, {shift}"), 0, "486")
}

/// sieve bq16: `memset(p, 0, n << 1)` of a 2-byte array was `rep stosb` of
/// 2048 bytes where `rep stosw` of 1024 stores half the operands: 1,024 of the
/// 2,706 over main.
#[test]
fn a_fill_of_whole_words_is_rep_stosw() {
    let unit = dynamic_fill_unit(1).expect("a fill");
    assert_eq!(unit, "2");
}

/// A size that is no multiple of 2 stays bytes.
#[test]
fn a_fill_of_unknown_size_stays_rep_stosb() {
    assert_eq!(dynamic_fill_unit(0).as_deref(), Some("1"));
}

/// The unit is only as wide as the known bits of the size allow: `n` and `n |
/// 1` are bytes, `n & -2` and `n << 1` words, `n << 2` dwords on the lowest
/// profile m16 has (the 386; there is no 8086 or 286 profile, llrm-cc rejects
/// `-march=i286`), so no profile gets a unit its registers lack.
#[test]
fn the_unit_of_a_dynamic_fill_follows_the_known_bits_of_its_size() {
    let of = |size: &str, cpu: &str| dynamic_fill_unit_of(size, 0, cpu).expect("a fill");
    assert_eq!(of("%s = add i16 %n, 0", "486"), "1");
    assert_eq!(of("%s = or i16 %n, 1", "486"), "1");
    assert_eq!(of("%s = and i16 %n, -2", "486"), "2");
    assert_eq!(of("%s = shl nuw i16 %n, 2", "386"), "4");
    assert_eq!(of("%s = shl nuw i16 %n, 3", "486"), "4");
}

/// A non-zero fill byte is splatted across the unit (`0x0101`, `0x01010101`):
/// the same `rep stos` for any constant byte, and a byte fill for a byte we
/// cannot splat at compile time.
#[test]
fn a_constant_fill_byte_is_splatted_into_the_unit() {
    assert_eq!(dynamic_fill_unit_of("%s = shl nuw i16 %n, 1", 7, "486").as_deref(), Some("2"));
}

/// sieve m32 -O2 grew 14 bytes when a `memset` of `n << 1` became `rep stosw`
/// (an operand-size prefix in 32-bit code, and a shift pair): a word is no unit
/// there. Returns the register width of the fill, and the names of the shifts
/// that make its count.
fn fill_at_m32(shift: u32) -> (Option<String>, usize) {
    let text = format!(
        "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
declare void @llvm.memset.p0.i32(ptr, i8, i32, i1)
define void @f(ptr %p, i32 %n) {{
entry:
  %s = shl nuw i32 %n, {shift}
  call void @llvm.memset.p0.i32(ptr %p, i8 0, i32 %s, i1 false)
  ret void
}}
"
    );
    let module = llrm_mir::parse::module(&text).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let whats = || insns.iter().filter_map(|i| i.what.as_ref());
    let unit = whats()
        .find(|w| w.op == crate::model::ir::Operation::Fill)
        .and_then(
            |w| w.sources
                .iter()
                .find_map(
                    |source| match source {
                        crate::model::ir::Loc::Held(held) => Some(held.width.to_string()),
                        _ => None,
                    },
                ),
        );
    (unit, whats().filter(|w| matches!(w.name.as_deref(), Some("shr" | "shl" | "sar"))).count())
}

#[test]
fn at_m32_a_fill_of_whole_words_stays_bytes() {
    assert_eq!(fill_at_m32(1).0.as_deref(), Some("1"));
}

#[test]
fn at_m32_a_fill_of_whole_dwords_is_rep_stosd_with_no_shift() {
    assert_eq!(fill_at_m32(2), (Some("4".to_owned()), 0));
}

/// `n << 1` bytes of zero at m16 is `cx = n`: the shift that built the size and
/// the one that halves it cancel (`add cx, cx ; shr cx, 1` was 2 bytes + 2).
#[test]
fn a_fill_of_n_shl_1_counts_n_words_with_no_shift_pair() {
    let text = "target datalayout = \"e-p:16:16-p1:32:16:16:16-i32:16-i64:16-n8:16:32\"
declare void @llvm.memset.p0.i16(ptr, i8, i16, i1)
define void @f(ptr %p, i16 %n) {
entry:
  %s = shl nuw i16 %n, 1
  call void @llvm.memset.p0.i16(ptr %p, i8 0, i16 %s, i1 false)
  ret void
}
";
    let module = llrm_mir::parse::module(text).expect("parses");
    let cpu = crate::backend::cpu::profile("486").expect("a cpu");
    let done = selected(&module, "f", &llrm_x86_m16::M16, cpu, &[]).unwrap_or_else(|e| panic!("{}", e.0));
    let shifts = done
        .body
        .insns()
        .iter()
        .filter_map(|i| i.what.as_ref())
        .filter(|w| matches!(w.name.as_deref(), Some("shr" | "shl" | "sar" | "add")))
        .count();
    assert_eq!(shifts, 0);
}
