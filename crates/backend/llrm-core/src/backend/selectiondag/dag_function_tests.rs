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
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu")).map_err(|e| e.0)?;
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
    assert_eq!(names(SUM).unwrap(), ["mov", "add", "mov", "ret"]);
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
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu")).unwrap_or_else(|e| panic!("{}", e.0));
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
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu")).unwrap_or_else(|e| panic!("{}", e.0));
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
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu")).unwrap_or_else(|e| panic!("{}", e.0));
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
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"))
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let call = insns.iter().find(|i| i.what.as_ref().is_some_and(|w| w.op == crate::model::ir::Operation::Call)).expect("a call");
    assert_eq!(done.calls.values().collect::<Vec<_>>(), ["bar"]);
    assert!(done.calls.contains_key(&call.at));
    assert_eq!(call.delivers.len(), 1, "the result is what the call delivers");
    assert!(call.clobbers.contains(&llrm_lir::registers::RegId::EAX), "the call destroys the registers the convention does not keep: {:?}", call.clobbers);
    let names: Vec<String> =
        insns.iter().filter_map(|i| i.what.as_ref()).map(|w| w.name.as_deref().unwrap_or("").to_owned()).collect();
    let (down, up) = (names.iter().position(|n| n == "sub").unwrap(), names.iter().rposition(|n| n == "add").unwrap());
    let called = names.iter().position(|n| n == "call").unwrap();
    assert!(down < called && called < up, "{names:?}");
}
