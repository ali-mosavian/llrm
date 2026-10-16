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
