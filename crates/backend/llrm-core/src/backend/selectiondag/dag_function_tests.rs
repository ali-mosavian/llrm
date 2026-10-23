use super::dag_function::selected;

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
    assert_eq!(names(SUM).unwrap(), ["mov", "add", "mov", ""]);
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
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"))
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
