use crate::module::{BlockId, Function, InstId, Module, Operand, ValueId};
use crate::touched::Edge;
use crate::{Position, parse};

const TEXT: &str = "define i16 @f(i16 %a, i1 %c, ptr %p) {
entry:
  %x = add i16 %a, 1
  %l = load i16, ptr %p
  store i16 %x, ptr %p
  br i1 %c, label %yes, label %no
yes:
  %y = mul i16 %l, 2
  br label %join
no:
  br label %join
join:
  %r = phi i16 [ %y, %yes ], [ 0, %no ]
  ret i16 %r
}
";

fn module() -> Module {
    parse::module(TEXT).unwrap_or_else(|error| panic!("{error}"))
}

fn named(
    function: &Function,
    name: &str,
) -> (InstId, ValueId) {
    function
        .walk()
        .find_map(|(_, inst)| {
            let result = function.instruction(inst).result?;
            (function.value(result).name.as_deref() == Some(name)).then_some((inst, result))
        })
        .unwrap_or_else(|| panic!("%{name}"))
}

fn block(
    function: &Function,
    name: &str,
) -> BlockId {
    function.layout().iter().copied().find(|&one| function.block(one).name.as_deref() == Some(name)).expect(name)
}

fn store(function: &Function) -> InstId {
    function
        .walk()
        .map(|(_, inst)| inst)
        .find(|&inst| {
            function.instruction(inst).result.is_none()
                && function.instruction(inst).operands.len() == 2
                && !function.instruction(inst).opcode.is_terminator()
        })
        .expect("a store")
}

/// An integer edit was seen as an edit of memory: every consumer of the
/// digest then recomputed what a rewritten `add` cannot change.
#[test]
fn an_integer_rewrite_touches_no_memory_and_no_edge() {
    let mut module = module();
    let f = module.function_mut("f").expect("@f").1;
    let mark = f.mark();
    let before = f.epochs();
    let (x, _) = named(f, "x");
    f.set_operand(x, 1, Operand::Value(f.parameters()[0]));
    let touched = f.touched_since(mark).expect("history");
    assert!(!touched.memory(), "{touched:?}");
    assert!(touched.cfg.is_empty());
    assert_eq!(touched.operands_changed.iter().collect::<Vec<_>>(), [named(f, "x").1]);
    assert_eq!(touched.blocks.iter().collect::<Vec<_>>(), [f.entry().unwrap()]);
    let after = f.epochs();
    assert_eq!((after.memory, after.cfg, after.calls), (before.memory, before.cfg, before.calls));
}

/// Erasing a store, moving a load: the digest names each by id and kind.
#[test]
fn memory_erased_and_moved_are_named() {
    let mut module = module();
    let f = module.function_mut("f").expect("@f").1;
    let (load, _) = named(f, "l");
    let st = store(f);
    let mark = f.mark();
    let before = f.epochs();
    let join = block(f, "join");
    let ret = f.terminator(join).unwrap();
    f.move_to(load, Position::Before(ret)).expect("moved");
    f.erase(st).expect("erased");
    let touched = f.touched_since(mark).expect("history");
    assert_eq!(touched.memory_moved.iter().collect::<Vec<_>>(), [load]);
    assert_eq!(touched.memory_removed.iter().collect::<Vec<_>>(), [st]);
    assert!(touched.memory_added.is_empty() && touched.memory_rewritten.is_empty());
    assert_eq!(f.epochs().memory, before.memory + 2);
}

/// An instruction made and erased inside the window was reported as removed:
/// a consumer then dropped an entry it never held.
#[test]
fn an_instruction_added_and_erased_in_the_window_is_in_neither_set() {
    let mut module = module();
    let f = module.function_mut("f").expect("@f").1;
    let (x, _) = named(f, "x");
    let (y, _) = named(f, "y");
    let mark = f.mark();
    let copy = f.clone_instruction(x);
    f.insert(copy, Position::Before(y)).expect("placed");
    f.erase(copy).expect("erased");
    let touched = f.touched_since(mark).expect("history");
    assert!(touched.values_added.is_empty() && touched.values_removed.is_empty(), "{touched:?}");
    assert!(touched.operands_changed.is_empty());
}

/// A branch retargeted: the old edge is removed and the new one added, and a
/// terminator replaced by one naming the same block leaves the edge alone.
#[test]
fn a_retargeted_branch_removes_one_edge_and_adds_another() {
    let mut module = module();
    let f = module.function_mut("f").expect("@f").1;
    let (entry, yes, no, join) = (f.entry().unwrap(), block(f, "yes"), block(f, "no"), block(f, "join"));
    let mark = f.mark();
    let br = f.terminator(entry).unwrap();
    f.set_operand(br, 1, Operand::Block(join));
    let touched = f.touched_since(mark).expect("history");
    let mut cfg = touched.cfg.clone();
    cfg.sort_by_key(|&(from, to, edge)| (from.0, to.0, edge == Edge::Added));
    assert_eq!(cfg, [(entry, yes, Edge::Removed), (entry, join, Edge::Added)]);
    let _ = no;
}

/// A pointer solve was redone for an integer edit, and kept for a pointer one:
/// `pointers` names the pointer values an edit reached and no integer.
#[test]
fn pointers_name_the_pointer_values_an_edit_reached_and_no_integer() {
    let mut module = module();
    let digest = |module: &mut Module, edit: &dyn Fn(&mut Function)| {
        let (context, f) = module.function_mut("f").expect("@f");
        let mark = f.mark();
        edit(f);
        f.touched_since(mark).expect("history").with_pointers(f, context)
    };
    let integer = digest(&mut module, &|f| {
        let (x, _) = named(f, "x");
        f.set_operand(x, 1, Operand::Value(f.parameters()[0]));
    });
    assert!(integer.pointers.is_empty(), "{:?}", integer.pointers);
    let pointer = digest(&mut module, &|f| {
        let st = store(f);
        f.erase(st).expect("erased");
    });
    let p = module.function_mut("f").expect("@f").1.parameters()[2];
    assert_eq!(pointer.pointers.iter().collect::<Vec<_>>(), [p]);
}

/// gvn replaced a function by its edited fork and every mark taken before the
/// fork stopped resolving: `touched_since` was none, and each analysis kept for
/// update, and each memo holding a body's mark, started from nothing.
/// Adopting the fork keeps the history of both.
#[test]
fn adopting_a_fork_keeps_the_marks_taken_before_it() {
    let mut module = module();
    let f = module.function_mut("f").expect("@f").1;
    let (x, _) = named(f, "x");
    let (y, _) = named(f, "y");
    let a = Operand::Value(f.parameters()[0]);
    let mark = f.mark();
    f.set_operand(x, 1, a);
    let mut fork = f.fork();
    fork.set_operand(y, 1, a);
    f.adopt(fork);
    let touched = f.touched_since(mark).expect("the mark resolves after the fork");
    let mut changed: Vec<_> = touched.operands_changed.iter().collect();
    changed.sort_by_key(|value| value.0);
    assert_eq!(changed, [named(f, "x").1, named(f, "y").1]);
}

/// Two forks of one function, edited apart by one change each, were one point
/// (the same lineage and the same position): a held access, a table kept for
/// update and any memo keyed by a mark answered one fork with the other's body
/// (gvn forks two numberings of a body and prices both). A fork's points are
/// its own, the function's before the fork are its too, and a sibling's do not
/// resolve.
#[test]
fn two_forks_edited_apart_are_not_one_point() {
    let mut module = module();
    let f = module.function_mut("f").expect("@f").1;
    let (x, _) = named(f, "x");
    let (y, _) = named(f, "y");
    let a = Operand::Value(f.parameters()[0]);
    let before = f.mark();
    let (mut one, mut other) = (f.fork(), f.fork());
    one.set_operand(x, 1, a);
    other.set_operand(y, 1, a);
    assert_ne!(one.mark(), other.mark(), "two forks, one change each, are one point");
    assert!(one.changes_since(before).is_some() && other.changes_since(before).is_some());
    assert!(one.changes_since(other.mark()).is_none(), "a sibling's point resolved");
    f.adopt(one);
    assert!(f.changes_since(before).is_some(), "the function's point before the fork");
    assert!(f.changes_since(other.mark()).is_none(), "the discarded sibling's point resolved");
}

/// A fork that changed nothing is the function: adopting it leaves the points
/// taken of the function equal to the function's now.
#[test]
fn adopting_a_fork_that_changed_nothing_keeps_the_mark() {
    let mut module = module();
    let f = module.function_mut("f").expect("@f").1;
    let before = f.mark();
    let fork = f.fork();
    f.adopt(fork);
    assert_eq!(f.mark(), before);
}

/// A rewrite reached only the blocks of the uses it feeds: the walk the
/// analyses each kept for themselves marked every block or none.
#[test]
fn an_edit_reaches_the_blocks_that_read_what_it_makes_and_no_others() {
    let mut module = module();
    let f = module.function_mut("f").expect("@f").1;
    let (yes, no, join) = (block(f, "yes"), block(f, "no"), block(f, "join"));
    let mark = f.mark();
    let (y, _) = named(f, "y");
    f.set_operand(y, 1, Operand::Value(f.parameters()[0]));
    let reach = f.touched_since(mark).expect("history").reach(f);
    assert_eq!(reach.blocks.iter().collect::<std::collections::BTreeSet<_>>(), [yes, join].into_iter().collect());
    assert!(!reach.blocks.contains(&no));
    assert_eq!(reach.conditions.iter().collect::<Vec<_>>(), [join]);
    assert!(!reach.cfg_changed());
}

/// A branch retargeted is a change of the CFG, whether or not the log says
/// the instruction was rewritten.
#[test]
fn a_retargeted_branch_is_a_change_of_the_cfg_in_the_reach() {
    let mut module = module();
    let f = module.function_mut("f").expect("@f").1;
    let (entry, join) = (f.entry().unwrap(), block(f, "join"));
    let mark = f.mark();
    let br = f.terminator(entry).unwrap();
    f.set_operand(br, 1, Operand::Block(join));
    assert!(f.touched_since(mark).expect("history").reach(f).cfg_changed());
}

/// An operand replaced: the digest names the value it read before as well as
/// the one it reads now, and the instruction's own result. The pressure update
/// kept a private list of these.
#[test]
fn a_replaced_operand_names_the_value_it_read_and_the_one_it_reads() {
    let mut module = module();
    let f = module.function_mut("f").expect("@f").1;
    let (x, result) = named(f, "x");
    let a = f.parameters()[0];
    let c = f.parameters()[1];
    let mark = f.mark();
    f.set_operand(x, 0, Operand::Value(c));
    let touched = f.touched_since(mark).expect("history");
    assert_eq!(touched.named.iter().collect::<std::collections::BTreeSet<_>>(), [a, c, result].into_iter().collect());
}

/// A switch of many arms retargeted, then its whole list replaced: the digest
/// names exactly the edges that differ from the mark's, and costs a scan of the
/// log once, not once for each terminator and each arm (`cases(N)` at -O1:
/// `analysis dominated-edges` read 3.4x a doubling with a pass per arm).
#[test]
fn the_edges_of_a_many_armed_switch_are_found_without_a_scan_per_arm() {
    let arms = 200_usize;
    let mut text = String::from("define i32 @f(i32 %a) {\nentry:\n  switch i32 %a, label %d [\n");
    for k in 0..arms {
        text += &format!("    i32 {k}, label %b{k}\n");
    }
    text += "  ]\nd:\n  ret i32 0\n";
    for k in 0..arms {
        text += &format!("b{k}:\n  br label %d\n");
    }
    text += "}\n";
    let mut module = parse::module(&text).unwrap_or_else(|error| panic!("{error}"));
    let f = module.function_mut("f").expect("@f").1;
    let (entry, d) = (f.entry().unwrap(), block(f, "d"));
    let (first, last) = (block(f, "b0"), block(f, &format!("b{}", arms - 1)));
    let mark = f.mark();
    let switch = f.terminator(entry).unwrap();
    // Arm 0 now goes to the last arm's block; every other arm is as it was.
    let at = f.instruction(switch).operands.iter().position(|one| *one == Operand::Block(first)).unwrap();
    f.set_operand(switch, at, Operand::Block(last));
    let touched = f.touched_since(mark).expect("history");
    assert_eq!(touched.cfg, [(entry, first, Edge::Removed)]);
    // The list replaced by one naming `d` alone: every other edge goes.
    let mut operands = f.instruction(switch).operands.clone();
    for operand in operands.iter_mut().filter(|one| matches!(one, Operand::Block(_))) {
        *operand = Operand::Block(d);
    }
    f.set_operands(switch, operands);
    let touched = f.touched_since(mark).expect("history");
    assert!(touched.cfg.iter().all(|&(from, _, edge)| from == entry && edge != Edge::Added), "{:?}", touched.cfg);
    assert_eq!(touched.cfg.len(), arms, "each arm's block but d: {:?}", touched.cfg.len());
}

/// The edge search looked at the whole log once for each terminator it touched
/// (`cases(N)` at -O1: `analysis dominated-edges` read 3.4x a doubling where
/// the walk it replaced read 2.0): its work is now linear in the edits, and
/// doubling the terminators touched doubles the entries and operands it reads.
#[test]
fn the_edge_search_reads_the_log_once_not_once_for_each_terminator_touched() {
    let visits = |blocks: usize| {
        let mut text = String::from("define i32 @f(i32 %a) {\nentry:\n  br label %b0\n");
        for k in 0..blocks {
            text += &format!("b{k}:\n  br label %b{}\n", k + 1);
        }
        text += &format!("b{blocks}:\n  ret i32 %a\nexit:\n  ret i32 0\n}}\n");
        let mut module = parse::module(&text).unwrap_or_else(|error| panic!("{error}"));
        let f = module.function_mut("f").expect("@f").1;
        let exit = block(f, "exit");
        let mark = f.mark();
        for k in 0..blocks {
            let br = f.terminator(block(f, &format!("b{k}"))).unwrap();
            f.set_operand(br, 0, Operand::Block(exit));
        }
        crate::touched::VISITED.with(|visited| visited.set(0));
        let touched = f.touched_since(mark).expect("history");
        assert_eq!(touched.cfg.len(), 2 * blocks, "each terminator's edge moved");
        crate::touched::VISITED.with(std::cell::Cell::get)
    };
    let (small, big) = (visits(64), visits(128));
    assert!(small > 0, "premise: the edges were searched");
    assert!(big as f64 <= 2.3 * small as f64, "{small} -> {big} entries and operands read for 64 -> 128 terminators");
}

/// `reach_of` read every operand of every list an edit replaced, to name the
/// values the edit touched, which it never reads (`cases(N)` at -O2: `analysis
/// dominated-edges` read 2.42x a doubling, 85% of it here): a switch of N
/// arms rewritten whole for each of N edits is N^2 operands, and doubling the
/// arms now reads none.
#[test]
fn reach_does_not_read_the_operand_lists_an_edit_replaced() {
    let scanned = |arms: usize| {
        let mut text = String::from("define i32 @f(i32 %a) {\nentry:\n  switch i32 %a, label %d [\n");
        for k in 0..arms {
            text += &format!("    i32 {k}, label %b{k}\n");
        }
        text += "  ]\nd:\n  ret i32 0\n";
        for k in 0..arms {
            text += &format!("b{k}:\n  br label %d\n");
        }
        text += "}\n";
        let mut module = parse::module(&text).unwrap_or_else(|error| panic!("{error}"));
        let f = module.function_mut("f").expect("@f").1;
        let (entry, d) = (f.entry().unwrap(), block(f, "d"));
        let switch = f.terminator(entry).unwrap();
        let mark = f.mark();
        for k in 0..arms {
            let mut operands = f.instruction(switch).operands.clone();
            let arm = block(f, &format!("b{k}"));
            let at = operands.iter().position(|one| *one == Operand::Block(arm)).unwrap();
            operands[at] = Operand::Block(d);
            f.set_operands(switch, operands);
        }
        let changes = f.changes_since(mark).expect("history").to_vec();
        crate::touched::SCANNED.with(|scanned| scanned.set(0));
        let reach = f.reach_of(&changes).expect("no parameters changed");
        assert!(reach.rewired && reach.blocks.contains(&entry), "premise: the switch was seen rewritten");
        crate::touched::SCANNED.with(std::cell::Cell::get)
    };
    let (small, big) = (scanned(64), scanned(128));
    assert!(big as f64 <= 2.3 * (small.max(1)) as f64, "{small} -> {big} operands read for 64 -> 128 arms");
    // The digest that names values still reads them: the counter is alive.
    let mut text = String::from(
        "define i32 @f(i32 %a) {\nentry:\n  switch i32 %a, label %d [\n    i32 0, label %b\n  ]\nd:\n  ret i32 0\nb:\n  ret i32 1\n}\n",
    );
    let mut module = parse::module(&text).unwrap();
    text.clear();
    let f = module.function_mut("f").expect("@f").1;
    let (entry, d) = (f.entry().unwrap(), block(f, "d"));
    let (switch, mark) = (f.terminator(entry).unwrap(), f.mark());
    let mut operands = f.instruction(switch).operands.clone();
    operands.truncate(2);
    operands.push(Operand::Block(d));
    f.set_operands(switch, operands);
    crate::touched::SCANNED.with(|scanned| scanned.set(0));
    f.touched_since(mark).expect("history");
    assert!(crate::touched::SCANNED.with(std::cell::Cell::get) > 0, "premise: the full digest reads the lists");
}
