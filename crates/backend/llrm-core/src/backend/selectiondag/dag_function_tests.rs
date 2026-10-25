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
/// cell); one live across another call that returns a float is selected too:
/// the float allocator spills it around the call and the stackifier takes the
/// result under the reload (RFP-5, fpstack #4362).
#[test]
fn a_float_result_is_the_stack_top_and_one_may_live_across_a_call() {
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
    selected(&module, "h", &llrm_x86_m32::M32, cpu, &[]).unwrap_or_else(|e| panic!("{}", e.0));
}

const FLOAT_ACROSS_VOID_CALL: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
declare void @note(i32)
define double @f(double %x, i32 %n) {
entry:
  %y = fadd double %x, 1.000000e+00
  call void @note(i32 %n)
  %z = fadd double %y, %x
  ret double %z
}
";

/// A float live across a call that returns no float is selected too.
#[test]
fn a_float_live_across_a_call_that_returns_no_float_is_selected() {
    let module = llrm_mir::parse::module(FLOAT_ACROSS_VOID_CALL).expect("parses");
    let cpu = crate::backend::cpu::profile("486").expect("a cpu");
    selected(&module, "f", &llrm_x86_m32::M32, cpu, &[]).unwrap_or_else(|e| panic!("{}", e.0));
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
/// values with far cells'); neither refusal is left.
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

fn huge_ir(
    name: &str,
    dst: &str,
    src: &str,
    metadata: &str,
) -> String {
    format!(
        "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-n8:16:32-ni:2\"
define cc 1005 void @f(ptr addrspace({dst}) %d, ptr addrspace({src}) %s, i32 %n) addrspace(1) {{
entry:
  call void @llvm.{name}.p{dst}.p{src}.i32(ptr addrspace({dst}) %d, ptr addrspace({src}) %s, i32 %n, i1 false){metadata}
  ret void
}}
declare void @llvm.{name}.p{dst}.p{src}.i32(ptr addrspace({dst}), ptr addrspace({src}), i32, i1)
"
    )
}

fn select_f(ir: &str) -> Result<crate::backend::isel::Selected, String> {
    let module = llrm_mir::parse::module(ir).expect("parses");
    selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .map_err(|e| e.0)
}

fn mnemonics(done: &crate::backend::isel::Selected) -> Vec<String> {
    done.body.insns().iter().filter_map(|i| i.what.as_ref()?.name.as_deref().map(str::to_owned)).collect()
}

/// A fill of `n` bytes through a huge pointer crosses segment ends and `n`
/// outgrows `cx`: a `rep stosb` filled only the first 64K (huge_runs gave a
/// wrong sum). The target's stepped routine is called inline instead, with
/// its operands in the registers it reads (`huge_runs` was the one c program
/// the DAG path refused).
#[test]
fn a_memset_through_a_huge_pointer_calls_the_stepped_routine() {
    let ir = M16_HUGE_MEMSET.to_owned();
    let done = select_f(&ir).unwrap_or_else(|why| panic!("{why}"));
    let routine = llrm_target::Target::huge_fill_routine(&llrm_x86_m16::M16, 1).expect("a fill routine");
    assert!(done.calls.values().any(|name| name == routine.name), "{:?}", done.calls);
    assert!(done.inline.values().any(|code| *code == routine.code), "its bytes are laid down at the call");
    assert!(!mnemonics(&done).iter().any(|m| m.starts_with("rep")), "{:?}", mnemonics(&done));
}

/// `memcpy` between huge pointers, and a `memmove` a pass proved forward or
/// backward, call the copy routine; a `memmove` of no proven direction, or
/// with a near pointer beside the huge one, is refused.
#[test]
fn a_copy_through_huge_pointers_calls_the_stepped_routine() {
    let forward = llrm_target::Target::huge_copy_routine(&llrm_x86_m16::M16, false).expect("a copy routine");
    let done = select_f(&huge_ir("memcpy", "3", "3", "")).unwrap_or_else(|why| panic!("{why}"));
    assert!(done.calls.values().any(|name| name == forward.name), "{:?}", done.calls);
    let refused = select_f(&huge_ir("memmove", "3", "3", "")).err().expect("no direction proved");
    assert!(refused.contains("direction"), "{refused}");
    let refused = select_f(&huge_ir("memcpy", "3", "0", "")).err().expect("a near source");
    assert!(refused.contains("near"), "{refused}");
}

const QB_STRING_COMPARE: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-i32:16-i64:16-n8:16:32\"
define cc 1000 i16 @f(ptr %a, ptr %b) addrspace(1) {
entry:
  %r = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %a, ptr %b)
  ret i16 %r
}
declare cc 1000 i16 @llrm.qb.B$SCMP(ptr, ptr) addrspace(1)
";

/// An ABI whose runtime routines return their answer in the flags or not.
struct Stub {
    flags: bool,
}

impl crate::backend::assemble::Abi for Stub {
    fn contract(
        &self,
        callee: &str,
        pops: bool,
        pushed: i64,
    ) -> Result<crate::abi::runtime::Contract, String> {
        let cleanup =
            if pops { crate::hir::model::StackCleanup::Callee } else { crate::hir::model::StackCleanup::Caller };
        Ok(crate::abi::runtime::Contract {
            flags_result: self.flags,
            ..crate::abi::qb::ordinary(callee, cleanup, pushed, &Default::default())
        })
    }

    fn linked(
        &self,
        name: &str,
    ) -> String {
        name.to_owned()
    }

    fn runtime_contracts(&self) -> bool {
        true
    }
}

fn verdicts(flags: bool) -> Vec<Result<super::builder::RuntimeAnswer, &'static str>> {
    let module = llrm_mir::parse::module(QB_STRING_COMPARE).expect("parses");
    let function = module.global(module.named("f").expect("f")).function().expect("a body");
    let layout = llrm_mir::datalayout::DataLayout::parse(module.datalayout.as_deref().unwrap()).unwrap();
    let verdicts =
        super::dag_function::runtime_call_verdicts(&module, function, &layout, &llrm_x86_m16::M16, &Stub { flags });
    verdicts.values().copied().collect()
}

/// `B$SCMP` answers in the flags under `--runtime qb45`; the DAG path tested
/// `ax` instead, so every string comparison of `strcmp_smoke` printed 0. The
/// call is known to answer in the flags.
#[test]
fn a_runtime_call_answering_in_the_flags_says_so() {
    assert_eq!(verdicts(true), [Ok(super::builder::RuntimeAnswer::Flags)]);
}

#[test]
fn a_runtime_call_answering_in_registers_says_so() {
    assert_eq!(verdicts(false), [Ok(super::builder::RuntimeAnswer::Registers)]);
}

const VARSEG: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-n8:16:32-ni:2\"
@x = internal global i16 0
define i16 @f() {
entry:
  ret i16 ptrtoint (ptr addrspace(2) addrspacecast (ptr @x to ptr addrspace(2)) to i16)
}
";

/// `VARSEG(x)` is a near pointer cast to the segment space: the selector of
/// its segment. The DAG path took the offset (`mov b$seg, offset X%`), so
/// `POKE` after `DEF SEG = VARSEG(x)` wrote 0 bytes from the segment and
/// `far_poke_*` printed garbage.
#[test]
fn a_near_pointer_cast_to_the_segment_space_is_its_segment_selector() {
    let module = llrm_mir::parse::module(VARSEG).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let spaces: Vec<_> = done
        .body
        .insns()
        .iter()
        .filter_map(|i| i.what.clone())
        .flat_map(|w| w.sources)
        .filter_map(|loc| match loc {
            crate::model::ir::Loc::Imm(imm) => imm.address.map(|a| a.space),
            _ => None,
        })
        .collect();
    assert_eq!(spaces, [crate::model::ir::Space::Group], "{spaces:?}");
}

const QB_PAD: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-i32:16-i64:16-n8:16:32\"
define cc 1000 void @f() addrspace(1) personality ptr addrspace(1) @llrm.qb.personality {
entry:
  invoke cc 1000 addrspace(1) void @llrm.qb.B$CEND() to label %next unwind label %landing
next:
  ret void
landing:
  %e = landingpad { ptr, i32 } catch ptr null
  ret void
}
declare cc 1000 void @llrm.qb.B$CEND() addrspace(1)
declare i32 @llrm.qb.personality(...) addrspace(1)
";

fn select_with_contracts(text: &str) -> Result<crate::backend::isel::Selected, String> {
    let module = llrm_mir::parse::module(text).expect("parses");
    let contracts =
        super::dag_function::RuntimeContracts { abi: &Stub { flags: false }, segments: &crate::backend::target::BASIC };
    let pool = &mut crate::backend::constpool::Pool::new(module.globals.len() as i64);
    super::dag_function::selected_in_pool(
        &module,
        "f",
        &llrm_x86_m16::M16,
        crate::backend::cpu::profile("386").expect("a cpu"),
        &[],
        Some(&contracts),
        pool,
    )
    .map_err(|e| e.0)
}

/// The runtime enters a landing pad: no edge reaches it. The DAG path left it
/// a block nothing led to (`block 0x0004 is not reachable from the entry`,
/// 13 qb programs that register `ON ERROR`); it needs the old selector's pad
/// call and a root block that branches to it.
#[test]
fn a_landing_pad_is_marked_and_reachable_from_the_entry() {
    let done = select_with_contracts(QB_PAD).unwrap_or_else(|why| panic!("{why}"));
    let landing = done.landing.expect("the pad's address");
    let blocks = &done.body.blocks;
    let by_at: std::collections::HashMap<i64, &crate::model::lir::Block> = blocks.iter().map(|b| (b.at, b)).collect();
    let mut seen = std::collections::HashSet::new();
    let mut work = vec![done.body.entry];
    while let Some(at) = work.pop() {
        if seen.insert(at) {
            work.extend(by_at[&at].succ.iter().copied());
        }
    }
    assert!(seen.contains(&landing), "{seen:?} {landing}");
    assert_eq!(done.calls.get(&landing).map(String::as_str), Some("__LANDING"));
}

const DEAD_DEFAULT: &str = "target datalayout = \"e-p:16:16-i32:16-i64:16-n8:16:32\"
define i16 @f(i16 %x) {
entry:
  switch i16 %x, label %dead [ i16 1, label %a ]
a:
  ret i16 7
dead:
  unreachable
}
";

const CONSTANT_BRANCH: &str = "target datalayout = \"e-p:16:16-i32:16-i64:16-n8:16:32\"
define i16 @f(i16 %x) {
entry:
  br i1 false, label %a, label %b
a:
  ret i16 7
b:
  ret i16 %x
}
";

/// A branch on a constant leaves one successor; the other block is dropped with
/// no branch left to it (`logical_value.c -O0` stopped on `block goes to
/// 0x001a, which is not in this body` when the MIR block was never built).
#[test]
fn a_branch_on_a_constant_leaves_no_branch_to_a_dropped_block() {
    let done = select_with_contracts(CONSTANT_BRANCH).unwrap_or_else(|why| panic!("{why}"));
    let present: std::collections::HashSet<i64> = done.body.blocks.iter().map(|b| b.at).collect();
    for block in &done.body.blocks {
        assert!(block.succ.iter().all(|next| present.contains(next)), "{:?} leaves for a missing block", block.at);
    }
}

/// A switch whose default is `unreachable` leaves that block with no edge;
/// the machine verifier stops at a block nothing reaches. It is dropped, as
/// `UnreachableMachineBlockElim` does.
#[test]
fn a_block_no_edge_reaches_is_dropped() {
    let done = select_with_contracts(DEAD_DEFAULT).unwrap_or_else(|why| panic!("{why}"));
    let by_at: std::collections::HashMap<i64, &crate::model::lir::Block> =
        done.body.blocks.iter().map(|b| (b.at, b)).collect();
    let mut seen = std::collections::HashSet::new();
    let mut work = vec![done.body.entry];
    while let Some(at) = work.pop() {
        if seen.insert(at) {
            work.extend(by_at[&at].succ.iter().copied());
        }
    }
    assert_eq!(seen.len(), done.body.blocks.len(), "{} blocks, {} reached", done.body.blocks.len(), seen.len());
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

const M16_REMAINDER_TWICE: &str = "target datalayout = \"e-p:16:16-i32:16-i64:16-n8:16:32\"
define i8 @f(ptr %p, i16 %a, i16 %b) {
entry:
  %r = srem i16 %a, %b
  store i16 %r, ptr %p
  %q = getelementptr i8, ptr %p, i16 %r
  %v = load i8, ptr %q
  ret i8 %v
}
";

/// A remainder read by a store and folded into an address: the selector
/// morphed the load without recording it as a user of the SDIVREM's result,
/// so the result's replacement missed it and the emitter met an unselected
/// 'SDivRem' (QCport `ls_animate`).
#[test]
fn a_remainder_used_by_a_store_and_an_address_is_selected() {
    let module = llrm_mir::parse::module(M16_REMAINDER_TWICE).expect("parses");
    selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
}

const FPROUND_F32: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-n8:16:32-ni:2\"
define cc 1000 ptr @h(ptr %0, ptr %1) addrspace(1) {
b1:
  %2 = load float, ptr %0
  %3 = fmul float %2, 5.000000e-01
  %4 = fpround float %3 to float
  store float %4, ptr %1
  ret ptr %1
}
";

/// `fpround float to float` stores to a slot and loads it back; the slot was a
/// frame object the frame did not know (`Emit(Unsupported("frame index 0"))`:
/// `byref2.bas`, a rounded SINGLE).
#[test]
fn a_rounded_float_has_its_slot_in_the_frame() {
    let module = llrm_mir::parse::module(FPROUND_F32).expect("parses");
    selected(&module, "h", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|why| panic!("{}", why.0));
}

const INLINE_CODE: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-n8:16:32-ni:2\"
declare i32 @llrm.ia16.code.89a60000.2p0(ptr)
define i32 @f() {
entry:
  %slot = alloca i16
  %r = call i32 @llrm.ia16.code.89a60000.2p0(ptr %slot)
  ret i32 %r
}
";

/// Inline code (`llrm.ia16.code`) is laid down at its site, the frame place it
/// names patched in as a displacement from BP: the emitter had no arm for the
/// node and refused "opcode 30013" (QCport's `__emit__`, 3 functions).
#[test]
fn inline_code_is_laid_down_at_its_site_with_its_frame_place() {
    let module = llrm_mir::parse::module(INLINE_CODE).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    assert_eq!(done.inline.len(), 1, "{:?}", done.inline);
    let (at, bytes) = done.inline.iter().next().unwrap();
    assert_eq!(done.inline_places.get(at).map(Vec::len), Some(1));
    assert!(!done.far.contains(at), "inline code is no far call");
    let (_, disp, addend) = done.inline_places[at][0];
    assert_eq!(u16::from_le_bytes([bytes[2], bytes[3]]), (disp + addend) as u16);
}

const BYVAL_ODD: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-n8:16:32-ni:2\"
declare void @g(ptr byval([41 x i8]) %p)
define void @f(ptr %q) {
entry:
  call void @g(ptr byval([41 x i8]) %q)
  ret void
}
";

/// A byval struct whose size is not a whole number of words (41 bytes) leaves
/// its last byte of the outgoing area unwritten: the stores that cannot be
/// pushes stay in place, based on a copy of SP. It was refused "an outgoing
/// argument area the stores do not cover at m16" (byval_struct -O0).
#[test]
fn an_odd_sized_byval_argument_is_stored_in_place_at_m16() {
    let module = llrm_mir::parse::module(BYVAL_ODD).expect("parses");
    selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
}

const CARRY_ACROSS_A_TEST: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define cc 1003 range(i32 0, -2147483648) i32 @_bench_x_ll_arith(i32 noundef %0) \"llrm-fp-eval\"=\"x87-extended\" memory(none) nounwind norecurse willreturn {
b1:
  %1 = icmp sgt i32 %0, 0
  br i1 %1, label %22, label %b3

b3:
  %2 = phi i32 [ %21, %20 ], [ 0, %b1 ]
  %3 = and i32 %2, 2147483647
  ret i32 %3

b4:
  %4 = phi i32 [ %19, %b4 ], [ 0, %22 ]
  %5 = phi i64 [ %18, %b4 ], [ 0, %22 ]
  %6 = phi i64 [ %8, %b4 ], [ 81985529216486895, %22 ]
  %7 = mul i64 %6, 6364136223846793005
  %8 = add i64 %7, 1442695040888963407
  %9 = lshr i64 %8, 33
  %10 = or i64 %9, 1
  %11 = mul i64 %10, %8
  %12 = and i32 %4, 31
  %13 = zext nneg i32 %12 to i64
  %14 = lshr i64 %8, %13
  %15 = xor i64 %11, %14
  %16 = shl i64 %8, %13
  %17 = add i64 %16, %5
  %18 = add i64 %17, %15
  %19 = add nuw nsw i32 %4, 1
  %exitcond = icmp ne i32 %0, %19
  br i1 %exitcond, label %b4, label %20

20:
  %21 = trunc i64 %18 to i32
  br label %b3

22:
  br label %b4
}
";

/// A 64-bit add's carry read after a select's `test` came between: the pin
/// glues the `adc` to the `add` and the scheduler here could not keep the
/// pair together ('flags overwritten between their producer and a consumer',
/// vsgcc x_ll_arith). The carry is saved in a byte before the `test` and put
/// back before its reader.
#[test]
fn a_carry_survives_a_flag_writer_scheduled_between_the_add_and_its_reader() {
    let module = llrm_mir::parse::module(CARRY_ACROSS_A_TEST).expect("parses");
    let done = selected(
        &module,
        "_bench_x_ll_arith",
        &llrm_x86_m32::M32,
        crate::backend::cpu::profile("486").expect("a cpu"),
        &[],
    )
    .unwrap_or_else(|e| panic!("{}", e.0));
    let saved = done.body.insns().iter().filter_map(|i| i.what.as_ref()).any(|w| w.name.as_deref() == Some("setb"));
    assert!(saved, "the carry is saved in a byte");
}

const LRINT: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i32 @f(double %x) {
entry:
  %r = call i32 @llvm.lrint.i32.f64(double %x)
  ret i32 %r
}
declare i32 @llvm.lrint.i32.f64(double)
";

/// `lrint` of an x87 value is `fist` into a stack slot and a load (the pin's
/// `IST_Fp32m64`); `CannotSelect(LRint)` refused 7 qb functions, the ones that
/// round a float to an integer (`CINT`).
#[test]
fn an_lrint_of_a_double_is_a_fist_and_a_load() {
    let made = names(LRINT).unwrap_or_else(|why| panic!("{why}"));
    assert!(made.iter().any(|name| name.starts_with("fist")), "{made:?}");
}

const AFTER_UNREACHABLE: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
declare void @abort()
define i32 @f(i32 %x, i1 %c) {
entry:
  br i1 %c, label %a, label %dead
a:
  %v = add i32 %x, 1
  br label %live
dead:
  call void @abort()
  unreachable
live:
  ret i32 %v
}
";

/// A block that ends in `unreachable` leads nowhere. The edge from it into the
/// next block in layout was made, and a value that block's real predecessor
/// defines was "read and not defined on every path" (the nib runtime's
/// `heap.allocate`, `@format.put`: 8 huge programs under `--isel=dag`).
#[test]
fn a_block_ending_in_unreachable_does_not_fall_into_the_next() {
    let module = llrm_mir::parse::module(AFTER_UNREACHABLE).expect("parses");
    selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|why| panic!("{}", why.0));
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
/// `std; rep movsb; cld` from the last byte, where the runtime has no
/// `memmove` (`copyw.nib`'s scroll up and down at m32); one proved neither way
/// is still refused.
#[test]
fn a_memmove_with_a_proved_direction_is_a_string_copy() {
    assert!(select_m32(&memmove_ir(", !llrm.forward !0")).is_ok());
    let backward = select_m32(&memmove_ir(", !llrm.backward !0")).unwrap_or_else(|why| panic!("{why}"));
    assert!(backward.inline.values().any(|bytes| bytes[..] == [0xFD, 0xF3, 0xA4, 0xFC]), "{:?}", backward.inline);
    let refused = select_m32(&memmove_ir("")).err().expect("no direction proved");
    assert!(refused.contains("memmove"), "{refused}");
}

const SCALED_INDEX_AT_M16: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-n8:16:32-ni:2\"
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
    let module = llrm_mir::parse::module(SCALED_INDEX_AT_M16).expect("parses");
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
        scaled += usize::from(scale != 1);
        assert!(scale == 1 || place.is_none(), "{what:?}");
    }
    assert!(scaled > 0, "the function has a scaled lea");
}

const SHIFT_IN_MEMORY: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define void @f(ptr %p, i16 %n) {
entry:
  %v = load i16, ptr %p
  %c = and i16 %n, 15
  %r = shl i16 %v, %c
  store i16 %r, ptr %p
  ret void
}
";

/// A variable shift of a value in memory is `shl [p], cl`: the form pins its
/// amount to `cl`, which the glued copy wrote. Only the register forms read
/// it, and `SHL16mCL` stopped the nib runtime's `N$PR4` with `missing source`.
#[test]
fn a_variable_shift_in_memory_reads_cl() {
    let module = llrm_mir::parse::module(SHIFT_IN_MEMORY).expect("parses");
    selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|why| panic!("{}", why.0));
}

const THREE_WAY_COMPARE: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i8 @f(i32 %a, i32 %b) {
entry:
  %lt = icmp ult i32 %a, %b
  %min = select i1 %lt, i32 %a, i32 %b
  %any = icmp ne i32 %min, 0
  br i1 %any, label %longer, label %done
longer:
  ret i8 7
done:
  %lt2 = icmp ult i32 %a, %b
  %eq = icmp eq i32 %a, %b
  %neg = select i1 %lt2, i8 -1, i8 1
  %r = select i1 %eq, i8 0, i8 %neg
  ret i8 %r
}
";

/// One compare feeds an unsigned less-than, an equality and a branch, with
/// instructions that rewrite the flags between them: the compare is `sub`
/// whose value nothing reads, made again for the next reader (the nib
/// runtime's `N$VCMP` stopped with `flags overwritten between their producer
/// and a consumer`, 3 of 51 nib programs and 25 examples under `--isel=dag`).
#[test]
fn a_compare_read_after_the_flags_changed_is_made_again() {
    let module = llrm_mir::parse::module(THREE_WAY_COMPARE).expect("parses");
    selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|why| panic!("{}", why.0));
}

const FLAGS_OF_A_SUB_AFTER_A_SHIFT: &str = "target datalayout = \"e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32\"
declare fastcc void @_far_get(ptr, ptr)
declare fastcc i16 @_con_gfx_trim(ptr)
declare fastcc i16 @_con_back_load(ptr, ptr)
declare fastcc i16 @_con_chars_load(ptr, ptr)
declare fastcc void @_con_text(ptr, i32, i16, ptr)
declare fastcc void @_con_back_draw(ptr, i32, i16)
declare cc 1003 i32 @_strlen(ptr)
declare cc 1003 ptr @_strcpy(ptr, ptr)
declare i32 @llvm.smin.i32(i32, i32)
define cc 1003 void @_con_draw(ptr noundef nocapture %0, ptr noundef %1, i32 noundef %2, i16 noundef %3, float noundef %4) \"llrm-fp-eval\"=\"x87-extended\" {
b1:
  %5 = alloca [82 x i8]
  %6 = alloca [80 x i8]
  %7 = getelementptr inbounds nuw i8, ptr %0, i32 4
  %8 = load float, ptr %7
  %9 = fcmp ole float %8, 0.000000e+00
  br i1 %9, label %b3, label %b2

b2:
  %10 = tail call fastcc i16 @_con_back_load(ptr noundef nonnull %0, ptr noundef %1)
  %11 = icmp eq i16 %10, 0
  br i1 %11, label %b9, label %b18

b3:
  %12 = getelementptr inbounds nuw i8, ptr %0, i32 10
  %13 = load i16, ptr %12
  %14 = sext i16 %13 to i32
  %15 = trunc i32 %14 to i16
  %16 = icmp slt i32 %14, 4
  %17 = select i1 %16, i16 %15, i16 4
  %18 = sext i16 %17 to i32
  %19 = tail call fastcc i16 @_con_gfx_trim(ptr noundef nonnull %0)
  %20 = icmp eq i16 %19, 0
  br i1 %20, label %b9, label %b7

b7:
  %21 = tail call fastcc i16 @_con_chars_load(ptr noundef nonnull %0, ptr noundef %1)
  %22 = icmp ne i16 %21, 0
  %23 = icmp sgt i32 %18, 0
  %or.cond = select i1 %22, i1 %23, i1 false
  br i1 %or.cond, label %135, label %b9

b9:
  ret void

b14:
  %24 = phi i32 [ %139, %135 ], [ %49, %b17 ]
  %25 = phi i32 [ %138, %135 ], [ %48, %b17 ]
  %26 = phi i16 [ 0, %135 ], [ %47, %b17 ]
  %27 = getelementptr i8, ptr %0, i32 8
  %28 = load i16, ptr %27
  %29 = sext i16 %28 to i32
  %30 = add i32 %24, %29
  %31 = icmp slt i32 %30, 0
  %32 = select i1 %31, i32 15, i32 0
  %33 = sext i16 %28 to i32
  %34 = add i32 %32, %24
  %35 = add i32 %34, %33
  %36 = lshr i32 %35, 4
  %37 = shl nuw i32 %36, 4
  %38 = sub i32 %33, %37
  %39 = add i32 %24, %38
  %40 = mul i32 %39, 4
  %41 = getelementptr i8, ptr %0, i32 %40
  %42 = getelementptr i8, ptr %41, i32 1296
  %43 = load float, ptr %42
  %44 = fcmp ole float %43, 0.000000e+00
  br i1 %44, label %b17, label %b15

b15:
  %45 = getelementptr inbounds [80 x i8], ptr %136, i32 %39
  call fastcc void @_far_get(ptr noundef %5, ptr noundef nonnull %45)
  call fastcc void @_con_text(ptr noundef nonnull %0, i32 noundef %2, i16 noundef %26, ptr noundef %5)
  %46 = add i16 %26, 8
  br label %b17

b17:
  %47 = phi i16 [ %26, %b14 ], [ %46, %b15 ]
  %48 = add i32 %25, -1
  %49 = add i32 %24, 1
  %50 = icmp samesign ugt i32 %48, 1
  br i1 %50, label %b14, label %b9

b18:
  %51 = getelementptr i8, ptr %0, i32 4
  %52 = load float, ptr %51
  %53 = fptosi float %52 to i16
  %54 = zext nneg i16 %53 to i32
  %55 = add i16 %53, -200
  tail call fastcc void @_con_back_draw(ptr noundef nonnull %0, i32 noundef %2, i16 noundef %55)
  %56 = icmp slt i16 %53, 16
  br i1 %56, label %b9, label %b20

b20:
  %57 = tail call fastcc i16 @_con_chars_load(ptr noundef nonnull %0, ptr noundef %1)
  %58 = icmp eq i16 %57, 0
  br i1 %58, label %b9, label %b22

b22:
  %59 = add nsw i32 %54, -16
  %60 = lshr i32 %59, 3
  %61 = getelementptr inbounds nuw i8, ptr %0, i32 10
  %62 = load i16, ptr %61
  %63 = sext i16 %62 to i32
  %64 = getelementptr inbounds nuw i8, ptr %0, i32 12
  %65 = load i16, ptr %64
  %66 = sext i16 %65 to i32
  %67 = sext i16 %62 to i32
  %68 = sext i16 %65 to i32
  %69 = sub nsw i32 %67, %68
  %70 = icmp sgt i32 %60, %69
  %71 = select i1 %70, i32 %69, i32 %60
  %72 = shl i32 %71, 16
  %73 = ashr exact i32 %72, 16
  %74 = icmp sgt i32 %73, 0
  br i1 %74, label %140, label %b27

b27:
  %75 = trunc i32 %54 to i16
  store i8 93, ptr %5
  %76 = getelementptr inbounds nuw i8, ptr %0, i32 1360
  call fastcc void @_far_get(ptr noundef %6, ptr noundef nonnull %76)
  %77 = icmp slt i16 %3, 0
  %78 = select i1 %77, i16 7, i16 0
  %79 = add i16 %3, %78
  %80 = ashr i16 %79, 3
  %81 = call cc 1003 i32 @_strlen(ptr noundef nonnull %6)
  %82 = shl i32 %81, 16
  %83 = ashr exact i32 %82, 16
  %84 = sext i16 %80 to i32
  %85 = add nsw i32 %84, -4
  %86 = icmp sgt i32 %83, %85
  %87 = sub nsw i32 %83, %85
  %88 = shl i32 %87, 16
  %89 = ashr exact i32 %88, 16
  %90 = select i1 %86, i32 %89, i32 0
  %91 = getelementptr inbounds i8, ptr %6, i32 %90
  %92 = getelementptr inbounds nuw i8, ptr %5, i32 1
  %93 = call cc 1003 ptr @_strcpy(ptr noundef nonnull %92, ptr noundef nonnull %91)
  %94 = call cc 1003 i32 @_strlen(ptr noundef nonnull %5)
  %95 = fmul float %4, 4.000000e+00
  %96 = fptosi float %95 to i32
  %97 = trunc i32 %96 to i8
  %98 = and i8 %97, 1
  %99 = or i8 %98, 10
  %100 = shl i32 %94, 16
  %101 = ashr exact i32 %100, 16
  %102 = getelementptr inbounds i8, ptr %5, i32 %101
  store i8 %99, ptr %102
  %103 = add i32 %100, 65536
  %104 = ashr exact i32 %103, 16
  %105 = getelementptr inbounds i8, ptr %5, i32 %104
  store i8 0, ptr %105
  %106 = add nsw i16 %75, -16
  call fastcc void @_con_text(ptr noundef nonnull %0, i32 noundef %2, i16 noundef %106, ptr noundef %5)
  br label %b9

b28:
  %107 = phi i32 [ %154, %140 ], [ %133, %b28 ]
  %108 = phi i32 [ %153, %140 ], [ %132, %b28 ]
  %109 = phi i16 [ %144, %140 ], [ %131, %b28 ]
  %110 = getelementptr i8, ptr %0, i32 8
  %111 = load i16, ptr %110
  %112 = getelementptr i8, ptr %0, i32 12
  %113 = load i16, ptr %112
  %114 = sext i16 %111 to i32
  %115 = sext i16 %113 to i32
  %116 = sub i32 %114, %115
  %117 = add i32 %107, %116
  %118 = icmp slt i32 %117, 0
  %119 = select i1 %118, i32 15, i32 0
  %120 = sext i16 %111 to i32
  %121 = sext i16 %113 to i32
  %122 = sub i32 %120, %121
  %123 = add i32 %119, %107
  %124 = add i32 %123, %120
  %125 = sub i32 %124, %121
  %126 = lshr i32 %125, 4
  %127 = shl nuw i32 %126, 4
  %128 = sub i32 %122, %127
  %129 = add i32 %107, %128
  %130 = getelementptr inbounds [80 x i8], ptr %145, i32 %129
  call fastcc void @_far_get(ptr noundef %5, ptr noundef nonnull %130)
  call fastcc void @_con_text(ptr noundef nonnull %0, i32 noundef %2, i16 noundef %109, ptr noundef %5)
  %131 = add i16 %109, 8
  %132 = add i32 %108, -1
  %133 = add i32 %107, 1
  %134 = icmp samesign ugt i32 %132, 1
  br i1 %134, label %b28, label %b27

135:
  %136 = getelementptr inbounds nuw i8, ptr %0, i32 14
  %137 = call i32 @llvm.smin.i32(i32 %14, i32 4)
  %138 = add nsw i32 %137, 1
  %139 = sub i32 16, %137
  br label %b14

140:
  %141 = trunc i32 %54 to i16
  %142 = lshr exact i32 %72, 13
  %143 = sub nsw i32 %59, %142
  %144 = trunc i32 %143 to i16
  %145 = getelementptr inbounds nuw i8, ptr %0, i32 14
  %146 = zext i16 %141 to i32
  %147 = add nsw i32 %146, -16
  %148 = lshr i32 %147, 3
  %149 = sub i32 %63, %66
  %150 = call i32 @llvm.smin.i32(i32 %148, i32 %149)
  %151 = trunc i32 %150 to i16
  %152 = sext i16 %151 to i32
  %153 = add nsw i32 %152, 1
  %154 = sub i32 16, %152
  br label %b28
}
";

/// A `sub`'s flags read by a branch after a shift came between: the flags are
/// made again as a `cmp` of the same operands, as a `cmp`'s own were
/// (QCport console `_con_draw` at m32: 'flags overwritten between their
/// producer and a consumer'). A `MOV32r0` is a `mov` here and writes none.
#[test]
fn the_flags_of_a_sub_are_made_again_after_another_unit_wrote_them() {
    let module = llrm_mir::parse::module(FLAGS_OF_A_SUB_AFTER_A_SHIFT).expect("parses");
    selected(&module, "_con_draw", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
}

const PAD_READS_THE_ENTRY: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-i32:16-i64:16-n8:16:32\"
@g = internal global i16 0
define cc 1000 void @f(i16 %x) addrspace(1) personality ptr addrspace(1) @llrm.qb.personality {
entry:
  %y = add i16 %x, 1
  invoke cc 1000 addrspace(1) void @llrm.qb.B$CEND() to label %next unwind label %landing
next:
  ret void
landing:
  %e = landingpad { ptr, i32 } catch ptr null
  store i16 %y, ptr @g
  ret void
}
declare cc 1000 void @llrm.qb.B$CEND() addrspace(1)
declare i32 @llrm.qb.personality(...) addrspace(1)
";

/// The runtime enters a landing pad with no register intact, so a value the
/// entry block defined is not there. The DAG path took such a function and the
/// verifier stopped the compile (`value is read but never defined`, qb
/// `onerr` and `erlnum`): it is refused and the old selector takes it.
#[test]
fn a_landing_pad_reading_an_entry_value_is_refused() {
    let refused = select_with_contracts(PAD_READS_THE_ENTRY).err().expect("refused");
    assert!(refused.contains("read and not defined"), "{refused}");
}

const RUNTIME_CALL_MORE_ARGS_THAN_DECLARED: &str =
    "target datalayout = \"e-p:16:16-p1:32:16:16:16-i32:16-i64:16-n8:16:32\"
define cc 1000 void @f(i16 %a) addrspace(1) {
entry:
  call cc 1000 addrspace(1) void (i16, i16, i16) @llrm.qb.B$RDIM(i16 %a, i16 1, i16 2)
  ret void
}
declare cc 1000 void @llrm.qb.B$RDIM() addrspace(1)
";

/// A runtime routine's declaration names no parameters (`B$RDIM()`), but its
/// callers push their arguments and it pops them all. The builder took the
/// declared count as what the callee pops and left 4 bytes to the caller:
/// `add sp, 4` after the call, and qb `bounds.bas` (`LOCALS`) crashed once
/// pads were selected.
#[test]
fn a_runtime_call_pops_every_argument_it_is_given() {
    let module = llrm_mir::parse::module(RUNTIME_CALL_MORE_ARGS_THAN_DECLARED).expect("parses");
    let contracts =
        super::dag_function::RuntimeContracts { abi: &Stub { flags: false }, segments: &crate::backend::target::BASIC };
    let pool = &mut crate::backend::constpool::Pool::new(module.globals.len() as i64);
    let done = super::dag_function::selected_in_pool(
        &module,
        "f",
        &llrm_x86_m16::M16,
        crate::backend::cpu::profile("386").expect("a cpu"),
        &[],
        Some(&contracts),
        pool,
    )
    .unwrap_or_else(|why| panic!("{}", why.0));
    let adjusts = done
        .body
        .insns()
        .iter()
        .filter(|i| {
            i.what
                .as_ref()
                .is_some_and(
                    |w| w.name.as_deref() == Some("add")
                        && w.dests.iter().any(|d| {
                            matches!(
                                d,
                                crate::model::ir::Loc::Reg(r) if r.register == crate::model::ir::STACK
                            )
                        }),
                )
        })
        .count();
    assert_eq!(adjusts, 0, "the caller removes nothing");
}

const SMUL_FIX: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i32 @f(i32 %a, i32 %b) {
entry:
  %r = call i32 @llvm.smul.fix.i32(i32 %a, i32 %b, i32 12)
  ret i32 %r
}
declare i32 @llvm.smul.fix.i32(i32, i32, i32 immarg)
";

/// `smul.fix` is the product's two halves and a funnel shift by the scale
/// (the pin's `IMUL32r` and `SHRD32rri8`); `CannotSelect("SMulFix")` stopped
/// the `mandel` example, which does its arithmetic in 20.12 fixed point.
#[test]
fn a_fixed_point_multiply_is_a_widening_multiply_and_a_funnel_shift() {
    let names = names(SMUL_FIX).unwrap_or_else(|why| panic!("{why}"));
    assert!(names.iter().any(|n| n == "imul" || n == "mul"), "{names:?}");
    assert!(names.iter().any(|n| n == "shrd"), "{names:?}");
}

const MERGED_WITH_PHI: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i32 @f(i32 %a, i32 %b, ptr %p, ptr %q) {
entry:
  %x = icmp slt i32 %a, %b
  %l = load i32, ptr %p
  %m = load i32, ptr %q
  %s = mul i32 %l, %m
  %k = add i32 %a, 1
  %y = icmp sgt i32 %s, 7
  %o = or i1 %x, %y
  br i1 %o, label %t, label %e
t:
  br label %j
e:
  br label %j
j:
  %r = phi i32 [ %k, %t ], [ %b, %e ]
  %u = add i32 %r, %k
  ret i32 %u
}
";

/// A condition split into branches whose right side uses a value the join
/// block also reads: the value is exported from the first block and read in
/// the second, the phi takes its inputs from every block that branches to it.
#[test]
fn a_split_condition_exports_what_the_later_blocks_read() {
    let module = llrm_mir::parse::module(MERGED_WITH_PHI).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    assert_eq!(done.body.blocks.len(), 5, "entry, the second compare, then, else, join");
}

const PORTS: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-n8:16:32-ni:2\"
declare i8 @llrm.ia16.in.i8(i16)
declare void @llrm.ia16.out.i8(i16, i8)
define i8 @f(i16 %p) {
entry:
  call void @llrm.ia16.out.i8(i16 968, i8 7)
  %v = call i8 @llrm.ia16.in.i8(i16 %p)
  ret i8 %v
}
";

/// Port I/O (`OUT`, `INP` of QB): a write to an immediate port, then a read
/// through `dx`, in that order, as `out` and `in` barriers. The builder
/// refused them ("intrinsic call llrm.ia16.out.i8": palette_using_pds71).
#[test]
fn port_writes_and_reads_stay_in_order() {
    let module = llrm_mir::parse::module(PORTS).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let names: Vec<&str> = insns
        .iter()
        .filter_map(|i| i.what.as_ref())
        .filter(|w| w.op == crate::model::ir::Operation::Barrier)
        .filter_map(|w| w.name.as_deref())
        .collect();
    assert_eq!(names, ["out", "in"]);
}

const ASM_TWO_OUTPUTS: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-i32:16-i64:16-n8:16:32\"
define i32 @f() addrspace(1) {
entry:
  %r = call { i16, i16 } @llrm.ia16.asm.cd1a.-.cx_dx.ax.n()
  %hi = extractvalue { i16, i16 } %r, 0
  %lo = extractvalue { i16, i16 } %r, 1
  %h = zext i16 %hi to i32
  %l = zext i16 %lo to i32
  %s = shl i32 %h, 16
  %v = or i32 %s, %l
  ret i32 %v
}
declare { i16, i16 } @llrm.ia16.asm.cd1a.-.cx_dx.ax.n()
";

/// An inline block with two outputs (`speaker.nib`'s BIOS tick count in
/// `cx:dx`) comes back as the fields of one aggregate; the builder refused
/// every block with more than one output.
#[test]
fn an_inline_block_with_two_outputs_is_selected() {
    let module = llrm_mir::parse::module(ASM_TWO_OUTPUTS).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|why| panic!("{}", why.0));
    assert!(done.inline.values().any(|bytes| bytes[..] == [0xCD, 0x1A]), "{:?}", done.inline);
}

const SWITCH_TO_PHI: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i32 @f(i32 %x, i32 %a, i32 %b) {
entry:
  switch i32 %x, label %d [ i32 0, label %j
    i32 1, label %k
    i32 2, label %j
    i32 3, label %k
    i32 4, label %j
    i32 5, label %k ]
d:
  ret i32 0
j:
  %p = phi i32 [ %a, %entry ]
  ret i32 %p
k:
  %q = phi i32 [ %b, %entry ]
  ret i32 %q
}
";

/// A switch whose targets have phis: the incoming values are copied once in
/// the switch's block and every block of the lowering that branches there (the
/// header, the compare blocks, the table) is an edge of the phi. The builder
/// refused it ("switch to a block with phis": 29 functions of the corpora with
/// the MIR switch lowering off).
#[test]
fn a_switch_to_blocks_with_phis_is_lowered() {
    use crate::model::ir::Operation;
    let module = llrm_mir::parse::module(SWITCH_TO_PHI).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let branches = done
        .body
        .blocks
        .iter()
        .flat_map(|block| block.insns.iter())
        .filter(|i| i.what.as_ref().is_some_and(|w| matches!(w.op, Operation::Branch | Operation::Jump)))
        .count();
    assert!(branches >= 2, "the switch became branches");
    let edges: usize =
        done.body.blocks.iter().map(|block| block.phis.iter().map(|phi| phi.incoming.len()).sum::<usize>()).sum();
    assert!(edges >= 2, "each phi has an edge from the lowering's blocks");
}

const PEELED_SWITCH: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
declare void @a()
declare void @b()
declare void @c()
define void @f(i32 %x) {
entry:
  switch i32 %x, label %d [ i32 1, label %hot
    i32 20, label %b
    i32 40, label %c ], !prof !0
d:
  ret void
hot:
  call void @a()
  ret void
b:
  call void @b()
  ret void
c:
  call void @c()
  ret void
}
!0 = !{!\"branch_weights\", i32 1, i32 1000, i32 1, i32 1}
";

/// A case that takes most of the probability (`peelDominantCaseCluster`) is
/// tested first, in the switch's block, and falls through to a block of the
/// rest. The builder refused it ("switch with a peeled case": bounds, onerr).
#[test]
fn a_dominant_case_is_peeled_and_the_rest_follows() {
    let module = llrm_mir::parse::module(PEELED_SWITCH).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    assert!(
        done.body.blocks.len() >= 7,
        "the peeled test and the rest are blocks of their own: {}",
        done.body.blocks.len()
    );
}

const M16_FAR_COPY: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-i32:16-i64:16-n8:16:32\"
@far = addrspace(1) global [100 x i16] zeroinitializer
@near = global [100 x i16] zeroinitializer
declare void @llvm.memcpy.p0.p1.i16(ptr, ptr addrspace(1), i16, i1)
define void @f(i16 %n) {
entry:
  %bytes = shl i16 %n, 1
  call void @llvm.memcpy.p0.p1.i16(ptr @near, ptr addrspace(1) @far, i16 %bytes, i1 false)
  ret void
}
";

/// A copy from a far global took DGROUP as the source's segment, because the
/// far pointer was built with `add` and only `or` was recognised:
/// `idiom_edges.c` at m16 summed the near array's old contents.
#[test]
fn a_copy_from_a_far_global_loads_the_globals_segment() {
    let module = llrm_mir::parse::module(M16_FAR_COPY).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let copy = insns.iter().filter_map(|i| i.what.as_ref()).find(|w| w.op == Operation::Copy).expect("a copy");
    let crate::model::ir::Loc::Held(from) = copy.sources[3] else { panic!("{copy:?}") };
    let loaded = insns
        .iter()
        .filter_map(|i| i.what.as_ref())
        .find(|w| w.dests.iter().any(|d| matches!(d, crate::model::ir::Loc::Held(h) if h.value == from.value)))
        .expect("the source segment is loaded");
    let crate::model::ir::Loc::Imm(imm) = &loaded.sources[0] else { panic!("{loaded:?}") };
    let group = imm.address.as_ref().map(|address| address.index);
    assert_ne!(group, Some(0), "DGROUP, not the global's own segment: {loaded:?}");
}

const INDIRECT_TAIL: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
@fp = global ptr null
define i32 @f() {
entry:
  %p = load ptr, ptr @fp
  %r = tail call i32 %p()
  ret i32 %r
}
";

/// A tail call through a pointer was refused (`Unmapped("TCRETURNri")`): it
/// is the epilogue and a `jmp` through a register the epilogue leaves alone.
#[test]
fn a_sibling_call_through_a_pointer_jumps_through_a_register() {
    let module = llrm_mir::parse::module(INDIRECT_TAIL).expect("parses");
    super::dag_function::TAIL_CALLS.set(true);
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[]);
    super::dag_function::TAIL_CALLS.set(false);
    let done = done.unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let jump = insns
        .iter()
        .filter_map(|i| i.what.as_ref())
        .find(|w| w.op == Operation::Return && w.name.as_deref() == Some("jmp"))
        .expect("a jmp that returns");
    assert!(jump.indirect && matches!(jump.sources.as_slice(), [crate::model::ir::Loc::Held(_)]), "{jump:?}");
    assert!(insns.iter().filter_map(|i| i.what.as_ref()).all(|w| w.op != Operation::Call), "no call remains");
}

const M16_TAIL: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-i32:16-i64:16-n8:16:32\"
declare cc 1005 i16 @g(i16)
define cc 1005 i16 @f(i16 %a) addrspace(1) {
entry:
  %r = tail call cc 1005 i16 @g(i16 %a)
  ret i16 %r
}
";

/// At m16 a call in tail position was always a call and a far return: the
/// sibling-call path was m32 only. It is the epilogue and a jump to the far
/// callee, which returns to the caller's caller.
#[test]
fn an_m16_tail_call_jumps_to_the_callee() {
    let module = llrm_mir::parse::module(M16_TAIL).expect("parses");
    super::dag_function::TAIL_CALLS.set(true);
    let done = selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[]);
    super::dag_function::TAIL_CALLS.set(false);
    let done = done.unwrap_or_else(|e| panic!("{}", e.0));
    assert_eq!(done.calls.values().collect::<Vec<_>>(), ["g"]);
    let insns = done.body.insns();
    let whats: Vec<_> = insns.iter().filter_map(|i| i.what.as_ref()).collect();
    assert!(whats.iter().any(|w| w.op == Operation::Return && w.name.as_deref() == Some("jmp")), "a jmp that returns");
    assert!(whats.iter().all(|w| w.op != Operation::Call), "no call remains");
}
