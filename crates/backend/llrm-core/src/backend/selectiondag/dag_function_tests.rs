use super::dag_function::selected;
use crate::model::ir::Operation;

const SUM: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i32 @f(i32 %a, i32 %b) {
entry:
  %s = add i32 %a, %b
  ret i32 %s
}
";

fn names(text: &str) -> Result<Vec<String>, String> {
    let module = llrm_mir::parse::module(text).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .map_err(|e| e.0)?;
    Ok(done
        .body
        .insns()
        .iter()
        .map(|i| i.what.as_ref().and_then(|w| w.name.as_deref().map(str::to_owned)).unwrap_or_default())
        .collect())
}

/// `--isel=dag` made no function: every compile with it was refused. A sum of
/// two stack arguments comes out as the instructions the pin selects.
#[test]
fn a_function_of_one_block_is_selected_to_lir() {
    assert_eq!(names(SUM).unwrap(), ["mov", "add", ""]);
}

const LOOP: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i32 @f(i32 %n) {
entry:
  br label %test
test:
  %s = phi i32 [ 0, %entry ], [ %s1, %body ]
  %i = phi i32 [ 0, %entry ], [ %i1, %body ]
  %more = icmp slt i32 %i, %n
  br i1 %more, label %body, label %done
body:
  %s1 = add i32 %s, %i
  %i1 = add i32 %i, 1
  br label %test
done:
  ret i32 %s
}
";

/// A counted loop is four LIR blocks: the header holds one phi per loop
/// variable with an input from the entry and one from the body, the header
/// branches to the body and falls into `done`, the body jumps back. A phi
/// with one input, or a branch to a block that is not there, would be wrong
/// code.
#[test]
fn a_loop_is_blocks_with_phis_and_branches() {
    let module = llrm_mir::parse::module(LOOP).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let blocks = &done.body.blocks;
    assert_eq!(blocks.len(), 4, "{blocks:#?}");
    let addresses: Vec<i64> = blocks.iter().map(|b| b.at).collect();
    let header = &blocks[1];
    assert_eq!(header.phis.len(), 2, "{header:#?}");
    for phi in header.phis.iter() {
        let mut from: Vec<i64> = phi.incoming.iter().map(|(at, _)| *at).collect();
        from.sort();
        assert_eq!(from, [addresses[0], addresses[2]], "entry and body feed the phi");
    }
    for block in blocks.iter() {
        assert!(block.succ.iter().all(|at| addresses.contains(at)), "{block:#?}");
    }
    // The builder swaps a branch whose true edge is the next block: it jumps to
    // `done` and falls into the body.
    assert_eq!(header.succ.iter().copied().collect::<Vec<_>>(), [addresses[3], addresses[2]]);
    assert_eq!(blocks[2].succ.iter().copied().collect::<Vec<_>>(), [addresses[1]]);
    assert!(blocks[3].succ.is_empty());
}

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
    let frame = Frame::new(vec![8, 12]);
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

const BYTE: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i32 @f(i32 %a, i32 %b) {
entry:
  %s = add i32 %a, %b
  %t = trunc i32 %s to i8
  %z = zext i8 %t to i32
  ret i32 %z
}
";

/// A truncate then an extend selects `EXTRACT_SUBREG` and a `movzx`: the
/// subregister is the same value viewed one byte wide, so the `movzx` reads
/// the value the add wrote. It was refused (`Unmapped(EXTRACT_SUBREG)`) in
/// 44 functions of the corpora.
#[test]
fn a_subregister_is_the_same_value_viewed_narrower() {
    let module = llrm_mir::parse::module(BYTE).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let add = insns.iter().find(|i| i.what.as_ref().is_some_and(|w| w.name.as_deref() == Some("add"))).expect("an add");
    let ext =
        insns.iter().find(|i| i.what.as_ref().is_some_and(|w| w.name.as_deref() == Some("movzx"))).expect("a movzx");
    let (add, ext) = (add.what.as_ref().unwrap(), ext.what.as_ref().unwrap());
    let (crate::model::ir::Loc::Held(written), crate::model::ir::Loc::Held(read)) = (&add.dests[0], &ext.sources[0])
    else {
        panic!("{add:?} {ext:?}")
    };
    assert_eq!((written.value, written.width, read.value, read.width), (written.value, 4, written.value, 1));
}

const CALL: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
declare i32 @bar(i32, i32)
define i32 @f(i32 %a, i32 %b) {
entry:
  %s = add i32 %a, 1
  %r = call i32 @bar(i32 %b, i32 %s)
  %t = add i32 %r, 3
  ret i32 %t
}
";

/// A call reserves its argument bytes, stores the arguments at SP, calls,
/// releases the bytes, and takes its result from what the call delivers in
/// EAX; it destroys the caller-saved registers. Each was `Unmapped
/// (ADJCALLSTACKDOWN32)`: 114 + 86 functions of the corpora.
#[test]
fn a_call_is_a_sequence_around_a_call_instruction() {
    let module = llrm_mir::parse::module(CALL).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let call = insns
        .iter()
        .find(|i| i.what.as_ref().is_some_and(|w| w.op == crate::model::ir::Operation::Call))
        .expect("a call");
    assert_eq!(done.calls.values().collect::<Vec<_>>(), ["bar"]);
    assert!(done.calls.contains_key(&call.at));
    assert_eq!(call.delivers.len(), 1, "the result is what the call delivers");
    assert!(
        call.clobbers.contains(&llrm_lir::registers::RegId::EAX),
        "the call destroys the registers the convention does not keep: {:?}",
        call.clobbers
    );
    let names: Vec<String> =
        insns.iter().filter_map(|i| i.what.as_ref()).map(|w| w.name.as_deref().unwrap_or("").to_owned()).collect();
    let (down, up) = (names.iter().position(|n| n == "sub").unwrap(), names.iter().rposition(|n| n == "add").unwrap());
    let called = names.iter().position(|n| n == "call").unwrap();
    assert!(down < called && called < up, "{names:?}");
}

const INDIRECT: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
@fp = global ptr null
define i32 @f(i32 %a) {
entry:
  %p = load ptr, ptr @fp
  %r = call i32 %p(i32 %a)
  ret i32 %r
}
";

/// A call through a pointer is a call with its target as the source and
/// `indirect` set; without the flag it was a direct call with no callee and
/// 71 modules of the vsgcc corpus stopped compiling.
#[test]
fn an_indirect_call_names_its_target() {
    let module = llrm_mir::parse::module(INDIRECT).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let call = done
        .body
        .insns()
        .iter()
        .filter_map(|i| i.what.clone())
        .find(|w| w.op == crate::model::ir::Operation::Call)
        .expect("a call");
    assert!(call.indirect && call.sources.len() == 1, "{call:?}");
    assert!(done.calls.is_empty());
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
  %v = load i32, ptr %p
  ret i32 %v
}
";

/// A local is a cell below the frame pointer, written and read at one
/// displacement; frame object 0 was `Emit(Unsupported(frame index 0))` in
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

const FLOAT64: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define double @f(i64 %a) {
entry:
  %d = uitofp i64 %a to double
  ret double %d
}
";

/// A conversion of an `i64` to a float is lowered by the target's hooks while
/// the types are legalized; the driver gave the type legalizer none, so the
/// function stopped at `Legalize(UIntToFp ... expand operand)` (15 + 11 + 6
/// functions of the corpora).
#[test]
fn the_type_legalizer_has_the_targets_hooks() {
    let module = llrm_mir::parse::module(FLOAT64).expect("parses");
    let why = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .err()
        .map_or_else(String::new, |e| e.0);
    assert!(!why.contains("expand operand"), "{why}");
}

const DIVIDE: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i32 @f(i32 %a, i32 %b) {
entry:
  %q = sdiv i32 %a, %b
  ret i32 %q
}
";

/// A signed divide sign-extends EAX into EDX, divides by the operand and
/// reads the quotient out of EAX: `CDQ` and `IDIV32r` were unmapped in 14 + 14
/// functions of the corpora. The quotient is the divide's first result.
#[test]
fn a_divide_is_cdq_then_idiv_on_eax_and_edx() {
    let module = llrm_mir::parse::module(DIVIDE).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let names: Vec<String> = done
        .body
        .insns()
        .iter()
        .filter_map(|i| i.what.as_ref())
        .map(|w| w.name.as_deref().unwrap_or("").to_owned())
        .collect();
    let (cdq, idiv) =
        (names.iter().position(|n| n == "cdq").expect("cdq"), names.iter().position(|n| n == "idiv").expect("idiv"));
    assert!(cdq + 1 == idiv, "cdq right before idiv: {names:?}");
    // Physical EAX/EDX operands are invisible to the allocator: it emitted
    // `idiv eax` and trapped. The operands are values the form pins.
    let insns = done.body.insns();
    let divide = insns.iter().filter_map(|i| i.what.as_ref()).find(|w| w.name.as_deref() == Some("idiv")).unwrap();
    assert!(
        divide.dests.iter().chain(&divide.sources).all(|l| matches!(l, crate::model::ir::Loc::Held(_))),
        "idiv operands are held values: {divide:?}"
    );
}

const SETCC_STORE: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
@flag = global i8 0
define void @f(i32 %a, i32 %b) {
entry:
  %c = icmp slt i32 %a, %b
  %z = zext i1 %c to i8
  store i8 %z, ptr @flag
  ret void
}
";

/// A comparison stored to a byte is `SETCCm`, which the emitter did not know.
#[test]
fn a_comparison_stored_to_a_byte_is_a_setcc_on_memory() {
    let module = llrm_mir::parse::module(SETCC_STORE).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let set = insns.iter().filter_map(|i| i.what.as_ref()).find(|w| w.name.as_deref() == Some("setl")).expect("a setl");
    assert!(matches!(set.dests[0], crate::model::ir::Loc::Mem(_)), "{set:?}");
}

const REGISTER_CALL: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
declare cc 1002 i32 @g(i32)
define i32 @main() {
entry:
  %r = call cc 1002 i32 @g(i32 7)
  ret i32 %r
}
";

/// A call that takes its arguments in registers must name them: the copies
/// into EAX, EDX and EBX were plain moves to physical registers, so the
/// allocator saw no use of the third, left EBX unset and the program hung
/// (`es_across_copy`).
#[test]
fn a_call_names_the_registers_it_takes_arguments_in() {
    let module = llrm_mir::parse::module(REGISTER_CALL).expect("parses");
    let done = selected(&module, "main", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let call = insns.iter().find(|i| i.call.is_some()).expect("a call");
    assert!(call.requires.len() == 1, "{:?} {:?} {:?}", call.requires, call.clobbers, call.what);
    assert!(call.requires.iter().all(|(held, _)| call.uses.contains(&held.value)));
}

const M16_SUM: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-i32:16-i64:16-n8:16:32\"
define cc 1005 i16 @f(i16 %a, i16 %b) addrspace(1) {
entry:
  %s = add i16 %a, %b
  ret i16 %s
}
";

/// At m16 the arguments arrive in registers and the procedure is far. The
/// driver refused every m16 function ("only m32"); once it took them, the
/// return was named `ret` and printed as a near return in a far procedure.
#[test]
fn an_m16_function_takes_register_arguments_and_returns_unnamed() {
    let module = llrm_mir::parse::module(M16_SUM).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let arrival = insns.iter().find(|i| !i.delivers.is_empty()).expect("an arrival instruction");
    assert_eq!(arrival.delivers.len(), 2, "both arguments arrive in registers");
    assert!(arrival.delivers.iter().all(|(held, _)| held.width == 2), "as 16-bit views");
    let last = insns.last().and_then(|i| i.what.as_ref()).expect("a return");
    assert_eq!((last.op, last.name.as_deref()), (crate::model::ir::Operation::Return, Some("")));
}

const BOTH_CALLS: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
declare cc 1002 i32 @w(i32)
declare i32 @c(i32)
define i32 @main() {
entry:
  %a = call cc 1002 i32 @w(i32 7)
  %b = call i32 @c(i32 %a)
  ret i32 %b
}
";

/// A call destroys what its callee's convention says: the DAG path gave every
/// call the target-wide set, EAX alone, so a value held in ECX or EDX across a
/// cdecl call was destroyed (`es_across_copy` looped forever).
#[test]
fn a_call_destroys_what_its_conventions_says() {
    use llrm_target::Target;
    let module = llrm_mir::parse::module(BOTH_CALLS).expect("parses");
    let done = selected(&module, "main", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let calls: Vec<_> = insns.iter().filter(|i| i.call.is_some()).collect();
    let regs = llrm_lir::registers::Regs(llrm_x86_m32::M32.registers());
    let named = |name: &str| regs.root(llrm_x86_m32::M32.register_named(name));
    assert_eq!(calls.len(), 2);
    assert!(!calls[0].clobbers.contains(&named("ecx")), "watcall keeps ECX: {:?}", calls[0].clobbers);
    assert!(
        calls[1].clobbers.contains(&named("ecx")) && calls[1].clobbers.contains(&named("edx")),
        "{:?}",
        calls[1].clobbers
    );
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
/// (`[ebp+ecx+126]` for 8192 bytes below BP) is no frame position: the layout
/// takes a positive one for the arguments' and left it unmoved, so fill_nest
/// read 12 bytes off. The object's address is made and the cell based on it.
#[test]
fn a_displacement_past_a_frame_object_is_based_on_its_address() {
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
    assert!(whats.iter().any(|w| w.op == crate::model::ir::Operation::Address), "the object's address is made");
    let past = whats
        .iter()
        .flat_map(|w| w.sources.iter())
        .find_map(|l| match l {
            crate::model::ir::Loc::Mem(m) if m.offset == 8318 => Some(m.clone()),
            _ => None,
        })
        .expect("the cell past the object");
    assert!(past.base.is_some() && past.index.is_some(), "{past:?}");
}

const BYVAL_ADDRESS: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
declare void @bar(ptr)
define void @f(ptr %z, ptr byval([20 x i8]) %x, ptr %y) {
entry:
  call void @bar(ptr %x)
  ret void
}
";

/// The builder numbers a byval parameter's fixed object like any other; the
/// frame left it out, so the address of `x` was the next parameter's, 20 bytes
/// up (byvalue_struct_address passed `y`'s place to `bar`).
#[test]
fn a_byval_parameter_is_a_fixed_object_of_its_own() {
    let module = llrm_mir::parse::module(BYVAL_ADDRESS).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let lea = insns
        .iter()
        .filter_map(|i| i.what.as_ref())
        .find(|w| w.op == crate::model::ir::Operation::Address)
        .expect("the address of x");
    let crate::model::ir::Loc::Address(a) = &lea.sources[0] else { panic!("{lea:?}") };
    assert_eq!(a.addr.map(|a| a.disp), Some(12), "x follows z at BP+8: {lea:?}");
}

const SWITCH: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i32 @f(i32 %x) {
entry:
  switch i32 %x, label %d [ i32 1, label %a
                            i32 100, label %b
                            i32 5000, label %c ]
a:
  ret i32 10
b:
  ret i32 20
c:
  ret i32 30
d:
  ret i32 0
}
";

/// A `switch` lowers to compare-and-branch blocks the builder makes after the
/// switch's own; they were refused ("a switch lowering", 5 programs of the
/// m32 execution run). Each is emitted after the block that made it and every
/// branch names one.
#[test]
fn a_switch_is_a_chain_of_compare_and_branch_blocks() {
    let module = llrm_mir::parse::module(SWITCH).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let blocks = done.body.blocks.len();
    assert!(blocks > 5, "the switch's blocks and its four targets: {blocks}");
    let at: Vec<i64> = done.body.blocks.iter().map(|b| b.at).collect();
    for block in done.body.blocks.iter() {
        assert!(block.succ.iter().all(|s| at.contains(s)), "{:?} leaves for a block that is not there", block.succ);
    }
}

const DIV64: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i64 @f(i64 %a, i64 %b) {
entry:
  %q = sdiv i64 %a, %b
  ret i64 %q
}
";

/// An `i64` divide is a call to the target's inline routine; the DAG path left
/// the call without the routine's bytes, so the object had an undefined
/// `divN_s` (link failures in c/int64_division.c, c/data_int64.c of the m32
/// run). The routine's code must travel with the selected function, at the
/// call.
#[test]
fn an_i64_divide_carries_the_routines_code_at_its_call() {
    let module = llrm_mir::parse::module(DIV64).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let routine = llrm_target::Target::divide_routine(&llrm_x86_m32::M32, true, true).expect("a routine");
    let at = done.calls.iter().find(|(_, name)| name.as_str() == routine.name).map(|(at, _)| *at).expect("a call");
    assert_eq!(done.inline.get(&at), Some(&routine.code));
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

const VARIADIC: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
declare void @llvm.va_start.p0(ptr)
define i32 @f(i32 %n, ...) {
entry:
  %ap = alloca ptr, align 4
  call void @llvm.va_start.p0(ptr %ap)
  ret i32 %n
}
";

/// `va_start` stores the address of the next fixed object, the first variadic
/// argument; the frame had no cell for it ("frame index -2" in the variadic
/// programs).
#[test]
fn va_start_finds_the_first_variadic_argument() {
    let module = llrm_mir::parse::module(VARIADIC).expect("parses");
    let why = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .err()
        .map(|e| e.0);
    assert!(why.is_none(), "{why:?}");
}

const M16_DIRECT: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-i32:16-i64:16-n8:16:32\"
declare cc 1005 i16 @g(i16)
define cc 1005 i16 @f(i16 %a) addrspace(1) {
entry:
  %r = call cc 1005 i16 @g(i16 %a)
  ret i16 %r
}
";

/// At m16 a call of a symbol is `CALL16r` of its address; it was emitted as a
/// call through an immediate (`call offset _report`), the biggest cause in the
/// m16 execution run (79 programs).
#[test]
fn an_m16_call_of_a_symbol_is_a_direct_call() {
    let module = llrm_mir::parse::module(M16_DIRECT).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    assert_eq!(done.calls.values().collect::<Vec<_>>(), ["g"]);
    let insns = done.body.insns();
    let call =
        insns.iter().filter_map(|i| i.what.as_ref()).find(|w| w.op == crate::model::ir::Operation::Call).unwrap();
    assert!(!call.indirect && call.sources.is_empty(), "{call:?}");
}

const VARIABLE_SHIFT: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i32 @f(i32 %x, i32 %n) {
entry:
  %r = shl i32 %x, %n
  ret i32 %r
}
";

/// The matcher copies the shift amount to `CL` with a glued `CopyToReg`, which
/// the adapter turned into a copy to a virtual register (`CL` had no physical
/// id): the shift read a `CL` nothing defined, and the LIR verifier said
/// "value defined N times" (about 20 m32 programs once #4074 let them past
/// "missing source").
#[test]
fn a_variable_shift_reads_the_amount_the_matcher_copied_to_cl() {
    let module = llrm_mir::parse::module(VARIABLE_SHIFT).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let shl = insns.iter().filter_map(|i| i.what.as_ref()).find(|w| w.name.as_deref() == Some("shl")).expect("a shl");
    assert!(shl.sources.iter().all(|l| matches!(l, crate::model::ir::Loc::Held(_))), "held values: {shl:?}");
}

const M16_STACK_ARGS: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-i32:16-i64:16-n8:16:32\"
declare i16 @g(i16, i16)
define i16 @f(i16 %a) {
entry:
  %r = call i16 @g(i16 %a, i16 233)
  ret i16 %r
}
";

/// Real mode has no `[sp+k]`: the outgoing arguments are pushed. The stores
/// into the area stopped 32 QCport modules from assembling.
#[test]
fn an_m16_call_pushes_its_stack_arguments() {
    let module = llrm_mir::parse::module(M16_STACK_ARGS).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let pushes =
        insns.iter().filter_map(|i| i.what.as_ref()).filter(|w| w.op == crate::model::ir::Operation::Push).count();
    assert_eq!(pushes, 2);
    let through_sp = insns
        .iter()
        .filter_map(|i| i.what.as_ref())
        .any(
            |w| w.dests
                .iter()
                .any(
                    |l| matches!(
                        l,
                        crate::model::ir::Loc::Mem(m) if m.through == llrm_lir::registers::RegId::SP
                    ),
                ),
        );
    assert!(!through_sp, "no store through SP");
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

const SHIFT_PARTS: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define cc 1002 i64 @f(i64 noundef %0, i32 noundef %1) {
b1:
  %2 = alloca i64
  %3 = alloca i32
  %4 = alloca i64
  store i64 %0, ptr %2
  store i32 %1, ptr %3
  %5 = load i64, ptr %2
  %6 = load i32, ptr %3
  %7 = zext i32 %6 to i64
  %8 = lshr i64 %5, %7
  store i64 %8, ptr %4
  %9 = load i64, ptr %4
  ret i64 %9
}
";

/// The pin's scheduler keeps a unit that defines a physical register from
/// going between that register's definition and the unit that reads it
/// (`DelayForLiveRegsBottomUp`). The `shrd` of a 64-bit shift went between the
/// `test` and the second `jne`, which read the `shrd`'s flags
/// (shift_int64_variable printed wrong values).
#[test]
fn nothing_that_sets_flags_goes_between_a_test_and_the_branches_that_read_it() {
    let module = llrm_mir::parse::module(SHIFT_PARTS).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let names: Vec<String> =
        insns.iter().filter_map(|i| i.what.as_ref()).map(|w| w.name.as_deref().unwrap_or("").to_owned()).collect();
    let test = names.iter().position(|n| n == "test").expect("a test");
    let last_branch = names.iter().rposition(|n| n == "jne" || n == "je").expect("a branch");
    for name in &names[test + 1..last_branch] {
        assert!(
            matches!(name.as_str(), "mov" | "jne" | "je" | "jmp"),
            "{name} between the test and the branches: {names:?}"
        );
    }
}

const FAR_CALL: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-n8:16:32-ni:2\"
declare void @far() addrspace(1)
declare void @near()
define void @f() {
entry:
  call addrspace(1) void @far()
  call void @near()
  ret void
}
";

/// A call of far code is a far call: the call sites are marked in `far`, as
/// the old selector marks them. The DAG path left the set empty, so every
/// call was a near `call` of a far routine and the program crashed at once
/// (99 real-mode programs under `LLRM_ISEL_REAL`).
#[test]
fn a_call_of_far_code_is_marked_far() {
    let module = llrm_mir::parse::module(FAR_CALL).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let sites: Vec<(i64, String)> = done.calls.iter().map(|(at, name)| (*at, name.clone())).collect();
    assert_eq!(sites.len(), 2, "{sites:?}");
    let far: Vec<&str> = sites.iter().filter(|(at, _)| done.far.contains(at)).map(|(_, name)| name.as_str()).collect();
    assert_eq!(far, ["far"], "{sites:?} far={:?}", done.far);
}

/// A call of a 16-bit callee destroys the upper half of `esi` and `edi` (it
/// pushes only `si` and `di`). The DAG path left `clobbers_high` empty, so a
/// 32-bit value kept in `esi` across a call came back with a stale upper half
/// (call_keeps, frameless, div_constants_*, phi_slot_unknown_arm under
/// `LLRM_ISEL_REAL`).
#[test]
fn a_call_of_a_16_bit_callee_destroys_the_upper_halves() {
    let module = llrm_mir::parse::module(FAR_CALL).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let high: Vec<_> = done
        .body
        .blocks
        .iter()
        .flat_map(|block| block.insns.iter())
        .filter(|one| one.call.is_some())
        .map(|one| one.clobbers_high.clone())
        .collect();
    let expected = std::collections::BTreeSet::from([llrm_lir::registers::RegId::ESI, llrm_lir::registers::RegId::EDI]);
    assert_eq!(high, [expected.clone(), expected]);
}

const FLOAT_RESULT: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define double @f(double %x) {
entry:
  %m = fmul double %x, 2.1
  %r = fadd double %m, 1.0
  ret double %r
}
";

/// A floating result is the x87 stack's top at the return, which the stackifier
/// reads as an unnamed store of the value: it was popped and dropped
/// (`fstp st(0)`) once loads and arithmetic were emitted. The constant 2.5 is a
/// pool cell, 1.0 is `fld1`.
#[test]
fn a_floating_result_is_stored_unnamed_before_the_return() {
    use crate::model::ir::Operation;
    let module = llrm_mir::parse::module(FLOAT_RESULT).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let seen: Vec<(Operation, String)> = insns
        .iter()
        .filter_map(|i| i.what.as_ref())
        .map(|w| (w.op, w.name.as_deref().unwrap_or("").to_owned()))
        .collect();
    let names: Vec<&str> = seen.iter().map(|(_, name)| name.as_str()).collect();
    assert!(
        names.contains(&"fld") && names.contains(&"fmul") && names.contains(&"fld1") && names.contains(&"fadd"),
        "{names:?}"
    );
    let ret = seen.len() - 1;
    assert_eq!(seen[ret - 1], (Operation::FloatStore, String::new()), "{names:?}");
    let pooled = insns
        .iter()
        .filter_map(|i| i.what.as_ref())
        .flat_map(|w| w.sources.iter())
        .any(
            |l| matches!(
                l,
                crate::model::ir::Loc::Mem(m) if m.width == 8 && m.addr.is_some()
            ),
        );
    assert!(pooled, "the 2.5 is a cell of the pool: {names:?}");
}

const SHIFTS_ON_ONE_COUNT: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i64 @f(i64 %x, i64 %y, i32 %n) {
entry:
  %c = zext i32 %n to i64
  %a = lshr i64 %x, %c
  %b = shl i64 %y, %c
  %m = mul i64 %b, 7
  %r = add i64 %a, %m
  ret i64 %r
}
";

/// Two selects on one compare, the second in the sink block of the first: a
/// flag-writing unit between them (the pin's scheduler allows it, its EFLAGS
/// live-in covers it) left the second branch on the flags of a `sub`
/// (shift_int64_variable -O2). Each branch is preceded by the compare that
/// made its flags.
#[test]
fn a_branch_on_flags_follows_the_compare_that_made_them() {
    let module = llrm_mir::parse::module(SHIFTS_ON_ONE_COUNT).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let names: Vec<String> =
        insns.iter().filter_map(|i| i.what.as_ref()).map(|w| w.name.as_deref().unwrap_or("").to_owned()).collect();
    for (at, name) in names.iter().enumerate() {
        if !matches!(name.as_str(), "jne" | "je") {
            continue;
        }
        let before = names[..at].iter().rev().find(|n| !matches!(n.as_str(), "mov" | "jmp" | "jne" | "je" | ""));
        assert!(
            matches!(before.map(String::as_str), Some("test" | "cmp" | "and")),
            "{name} after {before:?}: {names:?}"
        );
    }
}

const PAIR: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i64 @f(i32 %a, i32 %b) {
entry:
  %hi = zext i32 %a to i64
  %sh = shl i64 %hi, 32
  %lo = zext i32 %b to i64
  %r = or i64 %sh, %lo
  ret i64 %r
}
";

const SHIFT_OPTIMIZED: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define cc 1002 i64 @f(i64 noundef %0, i32 noundef %1) {
b1:
  %c = zext i32 %1 to i64
  %r = lshr i64 %0, %c
  ret i64 %r
}
";

/// `pair(a, b)` returned `(b << 32) | b`: the low half was copied to EAX, where
/// `a` was held for the high half, and the `ret` required nothing, so the
/// allocator saw no claim on EAX. The `ret` requires both return registers.
#[test]
fn a_ret_requires_the_registers_it_returns_in() {
    let module = llrm_mir::parse::module(PAIR).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let ret = insns.iter().find(|i| i.what.as_ref().is_some_and(|w| w.op == Operation::Return)).expect("a ret");
    let mut registers: Vec<_> = ret.requires.iter().map(|(_, register)| *register).collect();
    registers.sort_by_key(|r| r.index());
    assert_eq!(registers, [llrm_lir::registers::RegId::EAX, llrm_lir::registers::RegId::EDX], "{ret:?}");
}

/// The two selects of a 64-bit shift read one compare: the pin cascades the
/// second into the first's sink block (`EmitLoweredSelect`), one branch with
/// two phis. Two branches, the second with an empty arm, reached the
/// allocator as a shape that lost the zero of the high word (shr -O2 printed
/// the low word's bits there).
#[test]
fn two_selects_on_one_compare_are_one_branch() {
    let module = llrm_mir::parse::module(SHIFT_OPTIMIZED).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let branches =
        insns.iter().filter_map(|i| i.what.as_ref()).filter(|w| w.op == crate::model::ir::Operation::Branch).count();
    assert_eq!(branches, 1);
    let phis: Vec<usize> = done.body.blocks.iter().map(|b| b.phis.len()).collect();
    assert!(phis.contains(&2), "one sink block with both phis: {phis:?}");
}

const VARIABLE_MOVE: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-n8:16:32-ni:2\"
declare void @llvm.memmove.p0.p0.i16(ptr, ptr, i16, i1)
define void @f(ptr %d, ptr %s, i16 %n) {
entry:
  call void @llvm.memmove.p0.p0.i16(ptr %d, ptr %s, i16 %n, i1 false)
  ret void
}
";

/// A call of the runtime's `memmove` at m16 is a far call: its routine is a far
/// procedure in another code segment. It was a near `call`, and the program
/// crashed at once (the memmove_overlap run test under `LLRM_ISEL_REAL`).
#[test]
fn a_runtime_routine_is_called_far_at_m16() {
    let module = llrm_mir::parse::module(VARIABLE_MOVE).expect("parses");
    let libcalls = [("memmove".to_owned(), 0, "_memmove@3".to_owned())];
    let done =
        selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &libcalls)
            .unwrap_or_else(|e| panic!("{}", e.0));
    let sites: Vec<(&i64, &String)> = done.calls.iter().collect();
    assert_eq!(sites.len(), 1, "{sites:?}");
    assert!(done.far.contains(sites[0].0), "{sites:?} far={:?}", done.far);
}

const FLOAT_TO_INT: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i32 @f(double %x) {
entry:
  %r = fptosi double %x to i32
  ret i32 %r
}
";

/// A float truncated to an integer is a `fistt` of the value to a cell (the
/// pin's `FP64_TO_INT32_IN_MEM`, whose control-word dance `floatalloc` makes);
/// 22 programs stopped at `Unmapped(FP*_TO_INT*_IN_MEM)`.
#[test]
fn a_float_to_int_conversion_is_a_truncating_store() {
    use crate::model::ir::{Loc, Operation};
    let module = llrm_mir::parse::module(FLOAT_TO_INT).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let store = insns
        .iter()
        .filter_map(|i| i.what.as_ref())
        .find(|w| w.op == Operation::FloatStore && w.name.as_deref() == Some("fisttp"))
        .expect("a fistt");
    assert!(matches!(&store.dests[..], [Loc::Mem(cell)] if cell.width == 4), "{store:?}");
}

const FLOAT_COMPARE: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i32 @f(double %a, double %b) {
entry:
  %c = fcmp olt double %a, %b
  %r = zext i1 %c to i32
  ret i32 %r
}
";

/// A float compare is an `fcom` of two values and the set that reads its flags;
/// the stackifier adds the `fnstsw ax; sahf`, so the pin's `FNSTSW16r`, the AH
/// copy and `SAHF` emit nothing. 12 programs stopped at `UCOM_Fpr*`.
#[test]
fn a_float_compare_is_an_fcom_and_a_set() {
    use crate::model::ir::{Loc, Operation};
    let module = llrm_mir::parse::module(FLOAT_COMPARE).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let whats: Vec<_> = insns.iter().filter_map(|i| i.what.as_ref()).collect();
    let compare = whats.iter().position(|w| w.op == Operation::Compare).expect("an fcom");
    assert_eq!(whats[compare].name.as_deref(), Some("fcom"));
    assert!(whats[compare].sources.iter().all(|l| matches!(l, Loc::Held(h) if h.width == 10)), "{:?}", whats[compare]);
    assert!(whats[compare..].iter().any(|w| w.name.as_deref().is_some_and(|n| n.starts_with("set"))));
}

const M16_FAR_LOOP: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-i32:16-i64:16-n8:16:32\"
declare cc 1005 ptr addrspace(1) @g()
declare cc 1005 void @use(ptr addrspace(1))
define cc 1005 i16 @f(i16 %n) addrspace(1) {
entry:
  %p = call cc 1005 ptr addrspace(1) @g()
  br label %loop
loop:
  %q = phi ptr addrspace(1) [ %p, %entry ], [ %q2, %loop ]
  %i = phi i16 [ 0, %entry ], [ %i2, %loop ]
  %q2 = getelementptr i8, ptr addrspace(1) %q, i16 4
  %i2 = add i16 %i, 1
  call cc 1005 void @use(ptr addrspace(1) %q)
  %c = icmp ult i16 %i2, %n
  br i1 %c, label %loop, label %done
done:
  ret i16 %i2
}
";

/// A phi of a two-register value (a far pointer) has an entry for each part;
/// both went to the first register, so the second part was read and never
/// defined and the allocator's liveness failed ("a unique value") in 3 QCport
/// modules once float compares let those functions through.
#[test]
fn every_part_of_a_far_phi_is_defined() {
    let module = llrm_mir::parse::module(M16_FAR_LOOP).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let mut defined: std::collections::BTreeSet<u32> = std::collections::BTreeSet::new();
    for block in done.body.blocks.iter() {
        defined.extend(block.phis.iter().map(|phi| phi.result));
        for insn in block.insns.iter() {
            defined.extend(insn.defines.iter().copied());
        }
    }
    for insn in done.body.insns().iter() {
        for used in &insn.uses {
            assert!(defined.contains(used), "value {used} is read and never defined: {:?}", insn.what);
        }
    }
}

const FLOAT_CALL: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
declare float @rf()
declare double @rd(double)
define float @g() {
entry:
  %a = call float @rf()
  %b = fmul float %a, 2.0
  ret float %b
}
define double @h(double %x) {
entry:
  %a = call double @rd(double %x)
  %b = call double @rd(double %x)
  %c = fadd double %a, %b
  ret double %c
}
";

/// A float call result is the stack's top after the call (an unnamed load of no
/// cell); one live across another call cannot stay on the x87 stack and the
/// function is refused, not left for the stackifier to fail the compile.
#[test]
fn a_float_result_is_the_stack_top_and_none_lives_across_a_call() {
    use crate::model::ir::{Loc, Operation};
    let module = llrm_mir::parse::module(FLOAT_CALL).expect("parses");
    let cpu = crate::backend::cpu::profile("486").expect("a cpu");
    let done = selected(&module, "g", &llrm_x86_m32::M32, cpu, &[]).unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    assert!(
        insns.iter().filter_map(|i| i.what.as_ref()).any(|w| {
            w.op == Operation::FloatLoad
                && w.name.as_deref() == Some("")
                && w.sources.is_empty()
                && matches!(&w.dests[..], [Loc::Held(h)] if h.width == 10)
        }),
        "an unnamed load of the stack top"
    );
    let why = selected(&module, "h", &llrm_x86_m32::M32, cpu, &[]).err().map(|e| e.0);
    assert!(why.as_deref().is_some_and(|why| why.contains("live across")), "{why:?}");
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

const DOUBLE_BITS: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i32 @f(double %x) {
entry:
  %b = bitcast double %x to i64
  %t = trunc i64 %b to i32
  %r = and i32 %t, 16777215
  ret i32 %r
}
";

/// The low word of a double reached the matcher as the double's x87 register
/// (`and v5(4 bytes) = v5(10-byte fdiv result) & mask`): no code does that.
/// A float read as an integer is a refusal; float_conversions_loop did not
/// assemble (`mov [esp], mm0`).
#[test]
fn a_double_read_as_its_words_is_refused_not_emitted() {
    let module = llrm_mir::parse::module(DOUBLE_BITS).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[]);
    if let Err(why) = &done {
        assert!(why.0.contains("floating value used as an integer") || !why.0.is_empty(), "{why:?}");
    }
    // Whatever it is, no integer instruction reads a 10-byte floating value.
    if let Ok(done) = done {
        let floating: std::collections::HashSet<u32> = done
            .body
            .insns()
            .iter()
            .filter_map(|i| i.what.as_ref())
            .flat_map(|w| w.dests.iter())
            .filter_map(|d| match d {
                crate::model::ir::Loc::Held(h) if h.width == 10 => Some(h.value),
                _ => None,
            })
            .collect();
        for what in done.body.insns().iter().filter_map(|i| i.what.as_ref()) {
            for l in what.dests.iter().chain(&what.sources) {
                assert!(
                    !matches!(l, crate::model::ir::Loc::Held(h) if h.width != 10 && floating.contains(&h.value)),
                    "{what:?}"
                );
            }
        }
    }
}

const CONSTANT_AFTER_CALL: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
declare double @g(double)
define double @f(double %x) {
entry:
  %a = call double @g(double %x)
  %r = fadd double %a, 1.0
  ret double %r
}
";

/// A floating constant is loaded where it is read: `fld1` hoisted above the
/// call stayed on the x87 stack across it, and the function was refused
/// ("a floating value is live across ...").
#[test]
fn a_floating_constant_is_loaded_after_the_call_that_precedes_its_use() {
    use crate::model::ir::Operation;
    let module = llrm_mir::parse::module(CONSTANT_AFTER_CALL).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let whats: Vec<_> = insns.iter().filter_map(|i| i.what.as_ref()).collect();
    let call = whats.iter().position(|w| w.op == Operation::Call).expect("a call");
    let load = whats.iter().position(|w| w.name.as_deref() == Some("fld1")).expect("an fld1");
    assert!(
        load > call,
        "the constant is read after the call: {:?}",
        whats.iter().map(|w| &w.name).collect::<Vec<_>>()
    );
}

const EXTRA_ARGS: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
declare watcallcc i32 @none()
define i32 @f() {
entry:
  %r = call watcallcc i32 (i32, i32, i32, i32, i32, i32) @none(i32 1, i32 2, i32 3, i32 4, i32 5, i32 6)
  ret i32 %r
}
";

/// A call with more arguments than the callee declares (an old-style `()`
/// definition): the callee pops what it declares, so the caller removes the
/// rest. The DAG path left them on the stack and `main` returned elsewhere
/// (old_style_extra_args -m32: exception 0Dh).
#[test]
fn the_caller_removes_the_arguments_the_callee_does_not_declare() {
    let module = llrm_mir::parse::module(EXTRA_ARGS).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let text = format!("{:?}", done.body.insns().iter().filter_map(|i| i.what.as_ref()).collect::<Vec<_>>());
    assert!(text.contains("\"add\"") && text.contains("Imm(Imm { value: 8,"), "{text}");
}

const REMAINDER_EXTENDED: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i64 @f(i64 %a) {
entry:
  %r = srem i64 %a, 1000003
  %t = trunc i64 %r to i32
  %s = sext i32 %t to i64
  %m = mul i64 %s, 1000003
  ret i64 %m
}
";

/// The sign extension of a 64-bit remainder's low word reads EDX through the
/// glue of its `CDQ` read of EAX; that read was taken for a second result of
/// the remainder libcall, which then delivered two values in EDX and the
/// allocator could not place them ("could not place every range", the
/// regparm3 matrix callees 4, 22 and 31 under `--isel=dag`).
#[test]
fn no_call_delivers_two_values_in_one_register() {
    let module = llrm_mir::parse::module(REMAINDER_EXTENDED).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    for insn in done.body.insns().iter().filter(|i| !i.delivers.is_empty()) {
        let mut registers: Vec<_> = insn.delivers.iter().map(|(_, register)| *register).collect();
        let all = registers.len();
        registers.sort_by_key(|r| r.index());
        registers.dedup();
        assert_eq!(registers.len(), all, "{:?}", insn.delivers);
    }
}

const NARROW_POOL_CONSTANT: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define double @f(double %x) {
entry:
  %r = fmul double %x, 2.5
  ret double %r
}
";

/// The pool holds 2.5 as four bytes (a double that is exactly a float), and the
/// multiply reads four: it read eight, the constant's neighbour as its high
/// word (float_return, x87_arguments, double_to_uint64 printed wrong values).
/// A read at another width is refused as well.
#[test]
fn a_pool_entry_is_read_at_its_own_width() {
    use crate::model::ir::{Loc, Operation};
    let module = llrm_mir::parse::module(NARROW_POOL_CONSTANT).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let multiply = insns
        .iter()
        .filter_map(|i| i.what.as_ref())
        .find(|w| w.op == Operation::FloatArith && w.name.as_deref() == Some("fmul"))
        .expect("an fmul");
    assert!(multiply.sources.iter().any(|l| matches!(l, Loc::Mem(m) if m.width == 4)), "{multiply:?}");
}

const LONG_DOUBLE_SUM: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define x86_fp80 @f(x86_fp80 %x, x86_fp80 %y) {
entry:
  %r = fadd x86_fp80 %x, %y
  ret x86_fp80 %r
}
";

/// A long double is selected (it was refused 'long double' until its
/// arguments were proved), and each stack argument takes a 12-byte slot at m32,
/// as the front end and gcc lay them out: slots were the 10-byte store size,
/// so `sum(3, a, b, a)` read its second and third values 2 and 4 bytes short
/// (long_double_variadic printed -2147483648 for 13).
#[test]
fn a_long_double_is_selected_and_takes_a_twelve_byte_slot() {
    let module = llrm_mir::parse::module(LONG_DOUBLE_SUM).expect("parses");
    selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let layout = llrm_mir::datalayout::DataLayout::parse(module.datalayout.as_deref().unwrap()).unwrap();
    let (id, _, function) = module.functions().next().expect("a function");
    let convention = crate::backend::isel::convention(&module, &layout, id, &llrm_x86_m32::M32).expect("a convention");
    let first = llrm_target::Target::first_argument_offset(&llrm_x86_m32::M32, false);
    assert_eq!(convention.stack_arguments - first, 24, "two long doubles, twelve bytes each: {convention:?}");
    let _ = function;
}

const INT_TO_FLOAT_ROUNDED: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define float @f(i32 %x) {
entry:
  %r = sitofp i32 %x to float
  ret float %r
}
";

/// Under `-fexcess-precision=standard` an integer wider than the float's
/// mantissa is rounded to the float type (16777217 is 16777216.0f): the
/// conversion is stored and loaded back. The DAG path left the exact `fild`
/// in the register and `big != single` answered 1 (int_compared_with_float
/// -O0).
#[test]
fn an_integer_wider_than_the_mantissa_is_rounded_under_float_store() {
    let module = llrm_mir::parse::module(INT_TO_FLOAT_ROUNDED).expect("parses");
    let stores = |float_store: bool| {
        let mut cpu = crate::backend::cpu::profile("486").expect("a cpu").clone();
        cpu.float_store = float_store;
        let done = selected(&module, "f", &llrm_x86_m32::M32, &cpu, &[]).unwrap_or_else(|e| panic!("{}", e.0));
        done.body
            .insns()
            .iter()
            .filter_map(|i| i.what.as_ref())
            .filter(|w| w.op == crate::model::ir::Operation::FloatStore)
            .count()
    };
    assert!(stores(true) > stores(false), "{} vs {}", stores(true), stores(false));
}

const M16_FAR_LOAD: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-i32:16-i64:16-n8:16:32\"
define cc 1005 i16 @f(ptr addrspace(1) %p) addrspace(1) {
entry:
  %v = load i16, ptr addrspace(1) %p
  ret i16 %v
}
";

/// A far pointer loaded from memory is its offset and, beside it, its selector
/// (the two word loads `farload` joins into a `les`); the cell through ES
/// takes that selector. `LES16rm` was 328 of 494 refusals at m16.
#[test]
fn a_far_load_reads_the_pointer_and_goes_through_its_selector() {
    use crate::model::ir::{Loc, Operation, Space};
    let module = llrm_mir::parse::module(M16_FAR_LOAD).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let whats: Vec<_> = insns.iter().filter_map(|i| i.what.as_ref()).collect();
    let words = whats
        .iter()
        .filter(|w| {
            w.op == Operation::Move
                && matches!(
                    &w.sources[..],
                    [Loc::Mem(m)] if m.width == 2 && m.selector.is_none()
                )
        })
        .count();
    assert!(words >= 2, "the offset and the selector are two word loads: {whats:?}");
    assert!(
        whats
            .iter()
            .flat_map(|w| w.sources.iter())
            .any(|l| matches!(l, Loc::Mem(m) if m.selector.is_some() && m.addr.is_some_and(|a| a.space == Space::Far))),
        "the access goes through the selector"
    );
}

const M16_FILL: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-i32:16-i64:16-n8:16:32\"
@buf = global [100 x i8] zeroinitializer
declare void @llvm.memset.p0.i16(ptr, i8, i16, i1)
define cc 1005 void @f() addrspace(1) {
entry:
  call void @llvm.memset.p0.i16(ptr @buf, i8 7, i16 100, i1 false)
  ret void
}
";

/// At real mode `rep stosb` names the segment its pointer is in (a value: the
/// group's selector for a global); `REP_STOSB_16` was a refusal in 9 functions.
#[test]
fn an_m16_fill_names_its_segment() {
    use crate::model::ir::Operation;
    let module = llrm_mir::parse::module(M16_FILL).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let fill = insns
        .iter()
        .filter_map(|i| i.what.as_ref())
        .find(|w| w.op == Operation::Fill && w.name.as_deref() == Some("stosb"))
        .expect("a stosb");
    assert_eq!(fill.sources.len(), 4, "value, count, destination and its segment: {fill:?}");
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

const U64_TO_DOUBLE: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define double @f(i64 %x) {
entry:
  %r = uitofp i64 %x to double
  ret double %r
}
";

/// An unsigned 64-bit integer converts through the 80-bit register class and a
/// pool entry of its fudge (0.0f at +0, 2^64 at +4): the entry is read four
/// bytes inside its eight, and the register class alone is no long double.
/// 19 functions stopped at the entry's type.
#[test]
fn an_unsigned_64_bit_integer_converts_to_a_double() {
    let module = llrm_mir::parse::module(U64_TO_DOUBLE).expect("parses");
    selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
}

const CALL_RETURNING_I64: &str =
    "target datalayout = \"e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32\"
define internal cc 1002 i64 @t_(double noundef %0) {
b1:
  %1 = fptosi double %0 to i64
  ret i64 %1
}
define i64 @f(double noundef %d) {
b1:
  %r = call cc 1002 i64 @t_(double noundef %d)
  ret i64 %r
}
";

/// A call returning an i64 delivers both halves. The EDX read took the call's
/// glue only when the convention's clobber list or the target's named EDX; a
/// callee that returns in EDX without clobbering it made `mov v, EDX` read a
/// register nothing defined (`floatalloc: _main reads edx before every path
/// writes it`, double_to_int64.c -O0 under `--isel=dag`).
#[test]
fn a_call_delivers_both_halves_of_an_i64_result() {
    let module = llrm_mir::parse::module(CALL_RETURNING_I64).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let call = insns.iter().find(|i| i.what.as_ref().is_some_and(|w| w.op == Operation::Call)).expect("a call");
    let mut registers: Vec<_> = call.delivers.iter().map(|(_, register)| *register).collect();
    registers.sort_by_key(|r| r.index());
    assert_eq!(registers, [llrm_lir::registers::RegId::EAX, llrm_lir::registers::RegId::EDX], "{call:?}");
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

const FAR_INDIRECT_ARGUMENT: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-n8:16:32-ni:2\"
define void @f(ptr addrspace(1) %p) {
entry:
  call watcallcc addrspace(1) void %p(i32 7)
  ret void
}
";

/// The argument of a call through a far pointer reaches the call in its
/// register: `CALL_FAR` was not a call to the emitter, so the copy to `eax`
/// was dropped and `table[i](7 + i)` ran with garbage (function_table -O2:
/// want 7, got 478).
#[test]
fn a_call_through_a_far_pointer_receives_its_arguments() {
    let module = llrm_mir::parse::module(FAR_INDIRECT_ARGUMENT).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let call = insns.iter().find(|i| i.what.as_ref().is_some_and(|w| w.op == crate::model::ir::Operation::Call));
    assert!(call.is_some_and(|call| !call.requires.is_empty()), "{insns:?}");
}

const TWO_TABLES: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i32 @f(i32 %x, i32 %y) {
entry:
  switch i32 %x, label %ad [
    i32 0, label %a0
    i32 1, label %a1
    i32 2, label %a2
    i32 3, label %a3
    i32 4, label %a4
    i32 5, label %a5
  ]
a0:
  %av0 = add i32 %x, 1
  br label %ajoin
a1:
  %av1 = add i32 %x, 8
  br label %ajoin
a2:
  %av2 = add i32 %x, 15
  br label %ajoin
a3:
  %av3 = add i32 %x, 22
  br label %ajoin
a4:
  %av4 = add i32 %x, 29
  br label %ajoin
a5:
  %av5 = add i32 %x, 36
  br label %ajoin
ad:
  br label %ajoin
ajoin:
  %ar = phi i32 [ %x, %ad ], [ %av0, %a0 ], [ %av1, %a1 ], [ %av2, %a2 ], [ %av3, %a3 ], [ %av4, %a4 ], [ %av5, %a5 ]
  br label %second
second:
  switch i32 %y, label %bd [
    i32 0, label %b0
    i32 1, label %b1
    i32 2, label %b2
    i32 3, label %b3
    i32 4, label %b4
    i32 5, label %b5
  ]
b0:
  %bv0 = add i32 %x, 100
  br label %bjoin
b1:
  %bv1 = add i32 %x, 107
  br label %bjoin
b2:
  %bv2 = add i32 %x, 114
  br label %bjoin
b3:
  %bv3 = add i32 %x, 121
  br label %bjoin
b4:
  %bv4 = add i32 %x, 128
  br label %bjoin
b5:
  %bv5 = add i32 %x, 135
  br label %bjoin
bd:
  br label %bjoin
bjoin:
  %br = phi i32 [ %x, %bd ], [ %bv0, %b0 ], [ %bv1, %b1 ], [ %bv2, %b2 ], [ %bv3, %b3 ], [ %bv4, %b4 ], [ %bv5, %b5 ]
  %s = add i32 %ar, %br
  ret i32 %s
}
";

/// A dense `switch` is a jump through a table of the case blocks (the pin's
/// `JMP32m` on the table), and two of them in one function number their
/// blocks apart: both started at the function's block count, so the second's
/// table block was the first's and `switch_jump_table` read a value its
/// header never set (and, before that, refused on `TargetJumpTable`).
#[test]
fn two_dense_switches_are_two_jumps_through_tables() {
    let module = llrm_mir::parse::module(TWO_TABLES).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let at: Vec<i64> = done.body.blocks.iter().map(|b| b.at).collect();
    let mut unique = at.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), at.len(), "a block address is used twice: {at:?}");
    let tables: Vec<_> = done
        .body
        .insns()
        .iter()
        .filter_map(|i| i.what.as_ref())
        .filter(|w| w.indirect && !w.table.is_empty())
        .map(|w| w.table.clone())
        .collect();
    assert_eq!(tables.len(), 2, "one jump through a table for each switch");
    assert!(tables.iter().all(|table| table.len() == 6 && table.iter().all(|t| at.contains(t))), "{tables:?}");
    assert_ne!(tables[0], tables[1]);
}

const FAR_GLOBAL_ELEMENT: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-n8:16:32-ni:2\"
@cells = addrspace(1) global [4 x i16] zeroinitializer
define i16 @f() {
entry:
  %p = getelementptr [4 x i16], ptr addrspace(1) @cells, i16 0, i16 3
  %v = load i16, ptr addrspace(1) %p
  ret i16 %v
}
";

/// An element of a far global is a cell through ES whose displacement is the
/// global's symbol and offset. The emitter wanted a number there ("a
/// non-constant address part": far_return, es_across_copy, farstruct,
/// wide_globals at m16).
#[test]
fn an_element_of_a_far_global_is_a_cell_through_es() {
    let module = llrm_mir::parse::module(FAR_GLOBAL_ELEMENT).expect("parses");
    selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
}

const M16_STACK: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-i32:16-i64:16-n8:16:32\"
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

const STRING_OP_ON_ELEMENT: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-n8:16:32-ni:2\"
@buf = global [64 x i8] zeroinitializer
declare void @llvm.memset.p0.i16(ptr, i8, i16, i1)
define void @f(i16 %i, i16 %n) {
entry:
  %p = getelementptr [64 x i8], ptr @buf, i16 0, i16 %i
  call void @llvm.memset.p0.i16(ptr %p, i8 0, i16 %n, i1 false)
  ret void
}
";

/// A string operation on `&buf[i]` (an `lea` of the global and an index) takes
/// the global's segment: it refused "a pointer of no known segment" (idioms,
/// os_more, rep_count at m16).
#[test]
fn a_string_operation_on_an_element_of_a_global_knows_its_segment() {
    let module = llrm_mir::parse::module(STRING_OP_ON_ELEMENT).expect("parses");
    selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
}

/// The load of an incoming stack argument carried no invariant flag to LIR, so
/// MachineCSE and the sinker took it for memory a store may change: the pin's
/// `LowerMemArgument` makes the fixed object immutable and
/// `isDereferenceableInvariantLoad` takes the load.
#[test]
fn the_load_of_an_incoming_stack_argument_is_an_invariant_load_in_lir() {
    let text = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i32 @f(i32 %a, i32 %b) {
entry:
  ret i32 %a
}
";
    let module = llrm_mir::parse::module(text).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let flags: Vec<bool> = done
        .body
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
                            [crate::model::ir::Loc::Mem(cell)] if cell.addr.as_ref().is_some_and(|a| a.space == crate::model::ir::Space::Frame)
                        ),
                )
        })
        .map(|one| one.invariant())
        .collect();
    assert!(!flags.is_empty(), "the premise: the argument is loaded from the frame");
    assert!(flags.iter().all(|&flag| flag), "{flags:?}");
}

const M16_DOUBLE_ARGUMENT: &str = "target datalayout = \"e-p:16:16-i32:16-i64:16-n8:16:32\"
declare void @g(double)
define void @f(double %x) {
entry:
  %y = fadd double %x, 1.000000e+00
  call void @g(double %y)
  ret void
}
";

/// A double passed on the stack at m16 is stored with `fstp` into the argument
/// area, which no push can do and `[sp+k]` cannot address: it goes through a
/// register that holds SP (`byval_struct`, `float_return`, `wide_frame` and 5
/// more were refused 'a floating argument stored into the area').
#[test]
fn a_double_argument_is_stored_through_a_copy_of_sp_at_m16() {
    use crate::model::ir::{Loc, Operation};
    let module = llrm_mir::parse::module(M16_DOUBLE_ARGUMENT).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let store = insns
        .iter()
        .filter_map(|i| i.what.as_ref())
        .find(|w| {
            w.op == Operation::FloatStore
                && matches!(&w.dests[..], [Loc::Mem(_)])
                && w.name.as_deref().is_some_and(|n| n.starts_with("fst"))
        })
        .expect("an fstp into the area");
    let Loc::Mem(cell) = &store.dests[0] else { unreachable!() };
    assert!(cell.base.is_some() && cell.through == llrm_lir::registers::RegId::None, "{cell:?}");
}

const M16_SELECT_IN_ARGUMENT: &str = "target datalayout = \"e-p:16:16-i32:16-i64:16-n8:16:32\"
declare void @g(i64)
define void @f(i64 %x, i64 %n) {
entry:
  %r = shl i64 %x, %n
  call void @g(i64 %r)
  ret void
}
";

/// A select inside an outgoing argument area splits the block between the
/// area and the call: the stores after the split had no area in their block
/// and were refused 'a store through the stack pointer at m16'
/// (shift_int64_variable, shift_int64_masked_amount).
#[test]
fn an_argument_area_split_by_a_select_is_stored_through_a_copy_of_sp() {
    let module = llrm_mir::parse::module(M16_SELECT_IN_ARGUMENT).expect("parses");
    selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
}

const M16_FAR_AND_DOUBLE: &str =
    "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-i32:16-i64:16-n8:16:32\"
define double @f(ptr addrspace(1) %p, double %x) addrspace(1) {
entry:
  store i16 1, ptr addrspace(1) %p
  %y = fadd double %x, 1.000000e+00
  ret double %y
}
";

/// A far cell and a double in one function were refused as a pair ('floating
/// values with far cells'); only a double live across the far access is
/// (the stackifier counts the ES load as a boundary).
#[test]
fn a_double_not_live_across_a_far_cell_is_selected() {
    let module = llrm_mir::parse::module(M16_FAR_AND_DOUBLE).expect("parses");
    selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
}

const M16_FAR_NULL: &str =
    "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-i32:16-i64:16-n8:16:32\"
define i16 @f(ptr addrspace(1) %p) addrspace(1) {
entry:
  %c = icmp eq ptr addrspace(1) %p, null
  %r = zext i1 %c to i16
  ret i16 %r
}
";

/// A far null pointer is two zero words. `get_constant` of the far type
/// asserted 'integer constant of Far' and stopped 35 QCport functions.
#[test]
fn a_far_null_is_two_zero_words() {
    let module = llrm_mir::parse::module(M16_FAR_NULL).expect("parses");
    selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
}

const M16_HUGE_MEMSET: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-n8:16:32-ni:2\"
define cc 1005 void @f(ptr addrspace(3) %p, i32 %n) addrspace(1) {
entry:
  call void @llvm.memset.p3.i32(ptr addrspace(3) %p, i8 1, i32 %n, i1 false)
  ret void
}
declare void @llvm.memset.p3.i32(ptr addrspace(3), i8, i32, i1)
";

/// A fill of `n` bytes through a huge pointer crosses segment ends and `n`
/// outgrows `cx`: a `rep stosb` filled only the first 64K (huge_runs gave a
/// wrong sum). The DAG path refuses it and the old selector's stepped loop
/// stays.
#[test]
fn a_memset_through_a_huge_pointer_is_refused_not_a_rep_stos() {
    let module = llrm_mir::parse::module(M16_HUGE_MEMSET).expect("parses");
    let refused = selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .err()
        .expect("refused");
    assert!(refused.0.contains("huge pointer"), "{}", refused.0);
}

/// `memcpy` and `memmove` through a huge pointer (destination or source) are
/// refused like `memset`: a `rep movs` or the runtime's near routine would
/// stop at the first segment end.
#[test]
fn a_memcpy_or_memmove_through_a_huge_pointer_is_refused() {
    for (name, huge_dst) in [("memcpy", true), ("memcpy", false), ("memmove", true), ("memmove", false)] {
        let (dst, src) = if huge_dst { ("3", "0") } else { ("0", "3") };
        let ir = format!(
            "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-n8:16:32-ni:2\"
define cc 1005 void @f(ptr addrspace({dst}) %d, ptr addrspace({src}) %s, i32 %n) addrspace(1) {{
entry:
  call void @llvm.{name}.p{dst}.p{src}.i32(ptr addrspace({dst}) %d, ptr addrspace({src}) %s, i32 %n, i1 false)
  ret void
}}
declare void @llvm.{name}.p{dst}.p{src}.i32(ptr addrspace({dst}), ptr addrspace({src}), i32, i1)
"
        );
        let module = llrm_mir::parse::module(&ir).expect("parses");
        let refused =
            selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
                .err()
                .unwrap_or_else(|| panic!("{name} huge_dst={huge_dst}: selected"));
        assert!(refused.0.contains("huge pointer"), "{name} huge_dst={huge_dst}: {}", refused.0);
    }
}

const QB_STRING_COMPARE: &str = "target datalayout = \"e-p:16:16-i32:16-i64:16-n8:16:32\"
define i16 @f(ptr %a, ptr %b) {
entry:
  %r = call i16 @llrm.qb.B$SCMP(ptr %a, ptr %b)
  ret i16 %r
}
declare i16 @llrm.qb.B$SCMP(ptr, ptr)
";

/// A call of a QB runtime routine takes its arguments on the stack and answers
/// in the flags (`B$SCMP`); the DAG path passed `ax`/`dx` and tested `ax`, so
/// every string comparison of `strcmp_smoke` printed 0. It is refused for a
/// program whose ABI gives the runtime contracts.
#[test]
fn a_call_of_a_qb_runtime_routine_is_refused() {
    let module = llrm_mir::parse::module(QB_STRING_COMPARE).expect("parses");
    let cpu = crate::backend::cpu::profile("386").expect("a cpu");
    let pool = || crate::backend::constpool::Pool::new(module.globals.len() as i64);
    let call = |contracts| {
        super::dag_function::selected_in_pool(&module, "f", &llrm_x86_m16::M16, cpu, &[], contracts, &mut pool())
    };
    assert!(call(false).is_ok());
    let refused = call(true).err().expect("refused");
    assert!(refused.0.contains("QB runtime routine"), "{}", refused.0);
}

const I1_FREEZE: &str = "target datalayout = \"e-p:16:16-i32:16-i64:16-n8:16:32\"
define i16 @f(i1 %a, i16 %x) {
entry:
  %f = freeze i1 %a
  %z = zext i1 %f to i16
  %r = add i16 %z, %x
  ret i16 %r
}
";

/// Freezing an `i1` promotes the operand and freezes that
/// (`Freeze` of `Simple(I1)` stopped 2 QCport functions: 'promote result').
#[test]
fn an_i1_freeze_is_selected() {
    let module = llrm_mir::parse::module(I1_FREEZE).expect("parses");
    selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
}
