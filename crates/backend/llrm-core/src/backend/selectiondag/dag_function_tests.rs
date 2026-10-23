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
    let done = selected(&module, "f", &llrm_x86_m32::M32).map_err(|e| e.0)?;
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
    let done = selected(&module, "f", &llrm_x86_m32::M32).unwrap_or_else(|e| panic!("{}", e.0));
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
    let done = selected(&module, "f", &llrm_x86_m32::M32).unwrap_or_else(|e| panic!("{}", e.0));
    let first = done.body.insns()[0].what.clone().expect("semantics");
    let crate::model::ir::Loc::Mem(cell) = &first.sources[0] else { panic!("{first:?}") };
    assert!(cell.addr.is_some_and(|a| a.index == 0), "{cell:?}");
}
