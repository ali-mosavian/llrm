use crate::edit::ValueMap;
use crate::module::{BlockId, Change, Function, InstId, Module, Operand, Use, ValueId};
use crate::opcode::{BinaryOp, Flags, Opcode};
use crate::{Position, parse, print};

const TEXT: &str = "define i16 @f(i16 %a) {\nentry:\n  %x = add i16 %a, 1\n  %y = mul i16 %x, 2\n  ret i16 %y\n}\n";

fn module(text: &str) -> Module {
    parse::module(text).unwrap_or_else(|error| panic!("{error}"))
}

fn function(module: &mut Module) -> &mut Function {
    module.function_mut("f").expect("@f").1
}

/// The instruction defining `name`, and its value.
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

#[test]
fn replacing_every_use_lets_the_definition_go() {
    let mut module = module(TEXT);
    let f = function(&mut module);
    let (x, value) = named(f, "x");
    let (y, _) = named(f, "y");
    assert_eq!(f.users(value).snapshot(), [Use { user: y, index: 0 }]);
    assert_eq!(f.erase(x), Err(format!("instruction {}'s result still has 1 users", x.0)));
    let a = f.parameters()[0];
    f.replace_all_uses_with(value, Operand::Value(a));
    f.erase(x).expect("unused now");
    assert!(f.check_uses().is_empty(), "{:?}", f.check_uses());
    assert_eq!(
        f.take_changes(),
        [
            Change::Operand { inst: y, index: 0, was: Operand::Value(value) },
            Change::Rewritten(y),
            Change::Erased { inst: x, block: f.entry().unwrap(), next: Some(y) }
        ]
    );
    assert_eq!(print::module(&module), "define i16 @f(i16 %a) {\nentry:\n  %y = mul i16 %a, 2\n  ret i16 %y\n}\n");
}

#[test]
fn an_erased_id_is_never_reused() {
    let mut module = module(TEXT);
    let i16 = module.context.types.int(16);
    let one = module.context.int(i16, 1);
    let f = function(&mut module);
    let (y, value) = named(f, "y");
    let (x, _) = named(f, "x");
    let ret = f.terminator(f.entry().unwrap()).unwrap();
    let z = f.create_instruction(
        Opcode::Binary(BinaryOp::Sub),
        i16,
        vec![Operand::Value(value), Operand::Constant(one)],
        Flags::default(),
        Some("x"),
    );
    assert!(z.0 > ret.0 && z.0 > x.0 && z.0 > y.0);
    f.insert(z, Position::Before(ret)).expect("placed");
    let result = f.instruction(z).result.unwrap();
    f.set_operand(ret, 0, Operand::Value(result));
    assert!(f.check_uses().is_empty(), "{:?}", f.check_uses());
    assert!(print::module(&module).contains("  %x1 = sub i16 %y, 1\n  ret i16 %x1\n"), "{}", print::module(&module));
}

#[test]
fn a_clone_uses_what_its_original_uses() {
    let mut module = module(TEXT);
    let f = function(&mut module);
    let (x, value) = named(f, "x");
    let (y, _) = named(f, "y");
    let copy = f.clone_instruction(y);
    f.insert(copy, Position::Before(y)).expect("placed");
    assert_eq!(f.users(value).iter().count(), 2);
    assert_eq!(f.take_changes()[0], Change::Cloned { from: y, to: copy });
    assert_eq!(f.erase(x), Err(format!("instruction {}'s result still has 2 users", x.0)));
    assert!(f.check_uses().is_empty(), "{:?}", f.check_uses());
}

#[test]
fn blocks_split_by_moving_and_renaming_their_uses() {
    let text = "define i16 @f(i1 %c) {\nentry:\n  br i1 %c, label %yes, label %no\n\nyes:\n  ret i16 1\n\nno:\n  ret i16 2\n}\n";
    let mut module = module(text);
    let f = function(&mut module);
    let entry = f.entry().unwrap();
    let no = f.successors(entry)[1];
    let middle = f.create_block(Some("no"));
    f.insert_block(middle, Some(entry)).expect("placed");
    f.replace_block_uses_with(no, middle);
    let ret = f.terminator(no).unwrap();
    f.move_to(ret, Position::End(middle)).expect("moved");
    f.erase_block(no).expect("empty and unnamed by any terminator");
    assert_eq!(f.predecessors(middle), [entry]);
    assert!(f.check_uses().is_empty(), "{:?}", f.check_uses());
    assert_eq!(
        print::module(&module),
        "define i16 @f(i1 %c) {\nentry:\n  br i1 %c, label %yes, label %no1\n\nno1:\n  ret i16 2\n\nyes:\n  ret i16 1\n}\n"
    );
}

#[test]
fn the_use_list_check_notices_an_operand_changed_behind_its_back() {
    let mut module = module(TEXT);
    let f = function(&mut module);
    let (y, _) = named(f, "y");
    let a = f.parameters()[0];
    f.instructions[y.0 as usize].operands[0] = Operand::Value(a);
    assert_eq!(f.check_uses().len(), 2, "{:?}", f.check_uses());
}

/// A refused body goes whole, and what calls the function still verifies.
#[test]
fn deleting_a_body_leaves_a_declaration() {
    let mut module = module(TEXT);
    let f = function(&mut module);
    f.delete_body();
    assert!(f.check_uses().is_empty(), "{:?}", f.check_uses());
    assert!(f.is_declaration());
    assert_eq!(print::module(&module), "declare i16 @f(i16)\n");
}

/// A result derived at a mark is brought up to date from what changed after it,
/// only while that is on record and of the same function: a mark of a copy's
/// original names changes no one can list.
#[test]
fn a_mark_names_the_changes_since_while_they_are_on_record_of_that_function() {
    let mut module = module(TEXT);
    let f = function(&mut module);
    let (x, value) = named(f, "x");
    let (y, _) = named(f, "y");
    let a = f.parameters()[0];
    let mark = f.mark();
    assert_eq!(f.changes_since(mark), Some(&[][..]));
    f.replace_all_uses_with(value, Operand::Value(a));
    assert_eq!(
        f.changes_since(mark),
        Some(&[Change::Operand { inst: y, index: 0, was: Operand::Value(value) }, Change::Rewritten(y)][..]),
        "the log keeps what the operand was, for the readers of the edges and the values it stopped reading"
    );
    let copy = f.clone();
    assert_eq!(copy.changes_since(mark), None, "a copy's edits are its own");
    assert_eq!(copy.changes_since(copy.mark()), Some(&[][..]));
    f.erase(x).expect("unused now");
    assert_eq!(f.changes_since(mark).map(<[Change]>::len), Some(3));
    f.take_changes();
    assert_eq!(f.changes_since(mark).map(<[Change]>::len), Some(3), "taken, and still on the log");
    assert_eq!(f.changes_since(f.mark()), Some(&[][..]));
}

/// A parameter the passes removed leaves the others named by the position they
/// had: `-g` named `scale(p, factor)`'s `factor` the second, and the function
/// that kept only it made the first parameter `p`'s, a cell and a name that
/// were not its own.
#[test]
fn a_parameter_keeps_the_position_it_was_named_by() {
    let mut parsed = module("define i16 @f(i16 %a, i16 %b, i16 %c) {\nentry:\n  %x = add i16 %b, 1\n  ret i16 %x\n}\n");
    let (context, f) = parsed.function_mut("f").expect("@f");
    assert_eq!((f.parameter_origin(0), f.parameter_origin(1)), (Some(0), Some(1)));
    f.remove_parameter(context, 2);
    f.remove_parameter(context, 0);
    assert_eq!(f.parameter_origin(0), Some(1));
    assert_eq!(f.parameter_origin(1), None);
    let i16 = context.types.int(16);
    f.insert_parameters(context, 0, &[i16]);
    assert_eq!((f.parameter_origin(0), f.parameter_origin(1)), (None, Some(1)));
}

const RECORDED: &str = "define i16 @f(i16 %a) {\nentry:\n  %p = alloca i16\n  #dbg_declare(ptr %p, !0)\n  %x = add i16 %a, 1\n  #dbg_value(i16 %x, !0)\n  %y = mul i16 %x, 2\n  #dbg_gone(!0)\n  ret i16 %y\n}\n\n!0 = !{!\"v\"}\n";

fn records(function: &Function) -> Vec<(u32, crate::DebugWhat)> {
    function.debug_records().iter().map(|one| (one.before.0, one.what)).collect()
}

/// What `-g` says of a variable stands before an instruction, as LLVM's debug
/// records print: a declare, a value, and one that says nothing; read back, it
/// prints as it was, its variable the same node.
#[test]
fn debug_records_print_and_parse_as_they_were() {
    let mut parsed = module(RECORDED);
    assert_eq!(print::module(&parsed), RECORDED);
    let f = function(&mut parsed);
    let (x, value) = named(f, "x");
    let (y, _) = named(f, "y");
    let (p, pointer) = named(f, "p");
    let ret = f.terminator(f.entry().unwrap()).unwrap();
    assert_eq!(
        records(f),
        [
            (x.0, crate::DebugWhat::Declare(Operand::Value(pointer))),
            (y.0, crate::DebugWhat::Value(Operand::Value(value))),
            (ret.0, crate::DebugWhat::Gone)
        ]
    );
    let _ = p;
}

/// A value another stands for is the other in the records that named it: a
/// debugger that was told `x` is told `a`, not a value that is no more.
#[test]
fn replacing_a_value_by_its_equal_replaces_it_in_the_records_too() {
    let mut parsed = module(RECORDED);
    let f = function(&mut parsed);
    let (_, value) = named(f, "x");
    let a = f.parameters()[0];
    f.replace_value(value, Operand::Value(a));
    assert!(
        f.debug_records().iter().any(|one| one.what == crate::DebugWhat::Value(Operand::Value(a))),
        "{:?}",
        f.debug_records()
    );
    assert!(!f.debug_records().iter().any(|one| one.what == crate::DebugWhat::Value(Operand::Value(value))));
}

/// A pass that rewrites a value's uses to rebuild a loop around it (`unroll`
/// replaced the loop phi by its start value after the uses past the loop had
/// been given the last trip's) does not say the variable is the start value: it
/// was, at the end of the loop, the last trip's. A record stays on the value it
/// named; once that value is erased it says nothing.
#[test]
fn rewriting_the_uses_of_a_value_leaves_the_records_naming_it() {
    let mut parsed = module(RECORDED);
    let f = function(&mut parsed);
    let (_, value) = named(f, "x");
    let a = f.parameters()[0];
    f.replace_all_uses_with(value, Operand::Value(a));
    assert!(
        f.debug_records().iter().any(|one| one.what == crate::DebugWhat::Value(Operand::Value(value))),
        "{:?}",
        f.debug_records()
    );
}

/// An erased value has no record that names it (a record is no use, so the
/// erase goes through): they say it is gone. An erased instruction's records
/// stand before what followed it, where the source said they were; so the
/// verifier finds none dangling.
#[test]
fn erasing_leaves_records_gone_or_with_the_next_instruction() {
    let mut parsed = module(RECORDED);
    {
        let f = function(&mut parsed);
        let (x, value) = named(f, "x");
        let (y, _) = named(f, "y");
        let a = f.parameters()[0];
        // y reads x: x cannot go until y reads a.
        f.set_operand(y, 0, Operand::Value(a));
        assert!(f.users(value).is_empty());
        f.erase(x).expect("unused now");
        assert!(
            f.debug_records().iter().any(|one| one.before == y && one.what == crate::DebugWhat::Gone),
            "{:?}",
            f.debug_records()
        );
        // The record that stood before x stands before y, which followed it.
        let before_x: Vec<u32> = f.debug_records().iter().map(|one| one.before.0).collect();
        assert!(before_x.iter().all(|&at| at != x.0), "{before_x:?}");
        let ret = f.terminator(f.entry().unwrap()).unwrap();
        f.set_operand(ret, 0, Operand::Value(a));
        f.erase(y).expect("unused now");
        assert!(
            f.debug_records().iter().all(|one| one.before == ret || matches!(one.what, crate::DebugWhat::Declare(_))),
            "{:?}",
            f.debug_records()
        );
    }
    assert!(crate::verify::verify(&parsed).is_empty(), "{:?}", crate::verify::verify(&parsed));
}

/// A moved instruction leaves the records that stood before it where they were:
/// they say what the source said at that point, and the instruction going
/// elsewhere does not move the point.
#[test]
fn moving_an_instruction_leaves_its_records_in_place() {
    let mut parsed = module(RECORDED);
    let f = function(&mut parsed);
    let (y, _) = named(f, "y");
    let (x, _) = named(f, "x");
    let ret = f.terminator(f.entry().unwrap()).unwrap();
    f.move_to(y, Position::Before(x)).expect("moves");
    // y reads x, which now follows it: the record of the value stood before y,
    // and stays before `ret`, which followed y.
    assert!(f.debug_records().iter().all(|one| one.before != y), "{:?}", f.debug_records());
    assert!(f.debug_records().iter().any(|one| one.before == ret && matches!(one.what, crate::DebugWhat::Value(_))));
}

/// A branch to its own block, which nothing else enters, has no block to hand
/// its records to but its own: erased, they were left standing before the
/// erased branch ("a debug record of !0 stands before instruction N, which is
/// no longer in the function").
#[test]
fn erasing_a_branch_to_its_own_block_leaves_no_record_dangling() {
    let mut parsed = module(
        "define i16 @f(i16 %a) {\nentry:\n  ret i16 %a\n\nspin:\n  #dbg_value(i16 %a, !0)\n  br label %spin\n}\n\n!0 = !{!\"v\"}\n",
    );
    let f = function(&mut parsed);
    let spin = f.layout()[1];
    let jump = f.terminator(spin).expect("a branch");
    f.erase(jump).expect("a branch defines nothing");
    assert!(f.debug_records().iter().all(|one| one.before != jump), "{:?}", f.debug_records());
}

/// A record whose anchor is no longer in the function is the verifier's to
/// find.
#[test]
fn the_verifier_finds_a_record_before_an_erased_instruction() {
    let mut parsed = module(RECORDED);
    let f = function(&mut parsed);
    let (x, _) = named(f, "x");
    let y = named(f, "y").0;
    let a = f.parameters()[0];
    f.set_operand(y, 0, Operand::Value(a));
    f.erase(x).expect("unused now");
    f.debug_records.push(crate::DebugRecord {
        before: x,
        variable: crate::MetadataId(0),
        what: crate::DebugWhat::Gone,
        expression: None,
        location: None,
    });
    let found = crate::verify::verify(&parsed);
    assert!(
        found.iter().any(|one| one.contains("debug record") && one.contains("no longer in the function")),
        "{found:?}"
    );
}

/// A parameter the passes removed (dead-argument elimination) was named by a
/// record, which is no use of it: the record stood naming a value of no
/// function, and printing the body (the pipeline compares bodies by their text)
/// panicked on `hanoi` at -Os with `-g`.
#[test]
fn removing_a_parameter_leaves_the_records_that_named_it_gone() {
    let mut parsed = module(
        "define i16 @f(i16 %a, i16 %b) {\nentry:\n  #dbg_value(i16 %b, !0)\n  ret i16 %a\n}\n\n!0 = !{!\"v\"}\n",
    );
    let (context, f) = parsed.function_mut("f").expect("@f");
    f.remove_parameter(context, 1);
    let text = print::module(&parsed);
    assert!(text.contains("#dbg_gone(!0)") && !text.contains("#dbg_value"), "{text}");
}

/// An analysis computed before the last `take_changes` could not be brought up
/// to date: the log was emptied, so five in six of them, asked again after
/// another pass, were derived afresh.
#[test]
fn taken_changes_stay_for_an_analysis_computed_before_them() {
    let mut module = module(TEXT);
    let f = function(&mut module);
    let before = f.mark();
    let (x, _) = named(f, "x");
    let (y, value) = named(f, "y");
    let a = f.parameters()[0];
    f.replace_all_uses_with(value, Operand::Value(a));
    assert_eq!(f.take_changes().len(), 2);
    let _ = (x, y);
    assert_eq!(f.changes_since(before).map(<[Change]>::len), Some(2), "the taken change is gone from the log");
    assert!(f.take_changes().is_empty(), "a change was handed out twice");
}

/// A copy carried the original's whole log (up to 64k changes since the log
/// stopped being emptied, #984): gvn clones the body to number it, and
/// Vec<Change>::clone was 16% of compiling a program with a hundred inlines. A
/// copy's edits are its own.
#[test]
fn a_copy_starts_with_an_empty_log_and_equals_the_function_it_copies() {
    let mut module = module(TEXT);
    let f = function(&mut module);
    let (_, value) = named(f, "x");
    let a = f.parameters()[0];
    f.replace_all_uses_with(value, Operand::Value(a));
    assert!(!f.changes.0.is_empty());
    let copy = f.clone();
    assert!(copy.changes.0.is_empty(), "the copy carries {} changes", copy.changes.0.len());
    assert!(*f == copy, "the log is no part of what a function is");
    let mark = copy.mark();
    assert_eq!(copy.changes_since(mark), Some(&[][..]));
}

/// A move logs what follows it as the position named, without finding it in the
/// block: that scan made each append of a straight-line body linear, and `hir
/// to mir` 3.3x for twice the size (#992).
#[test]
fn a_move_logs_what_the_position_named() {
    let mut module = module(TEXT);
    let f = function(&mut module);
    let (x, _) = named(f, "x");
    let (y, _) = named(f, "y");
    let block = f.entry().unwrap();
    f.take_changes();
    f.move_to(y, Position::Before(x)).unwrap();
    f.move_to(x, Position::End(block)).unwrap();
    assert_eq!(
        f.take_changes(),
        [
            Change::Moved { inst: y, block, next: Some(x), from: block },
            Change::Moved { inst: x, block, next: None, from: block }
        ]
    );
}

/// `n` unused adds, and a return.
fn adds(n: usize) -> String {
    let body: String = (0..n).map(|i| format!("  %x{i} = add i16 %a, {i}\n")).collect();
    format!("define i16 @f(i16 %a) {{\nentry:\n{body}  ret i16 %a\n}}\n")
}

#[test]
fn erasing_instructions_one_at_a_time_or_together_leaves_the_same_function() {
    // Each erase looked its instruction up in the block and shifted the rest:
    // 200 of 400 cost 200 scans of the block. On a 2048-statement function
    // the lookups were 7.5% of the whole compile, growing fourfold a doubling.
    let erase = |batch: bool| {
        let mut module = module(&adds(400));
        let f = function(&mut module);
        let gone: Vec<InstId> = (0..400).step_by(2).map(|i| named(f, &format!("x{i}")).0).collect();
        if batch {
            f.erase_all(&gone).expect("unused");
        } else {
            gone.iter().rev().for_each(|&one| f.erase(one).expect("unused"));
        }
        assert!(f.check_uses().is_empty());
        print::module(&module)
    };
    assert_eq!(erase(true), erase(false));
}

/// Each use an erased instruction made was looked for in its operand's list of
/// uses: 200 adds of one parameter that go were 200 scans of up to 400 uses
/// (`Function::erase` x3.8 a doubling on the diamond ladder). A use unlinks
/// through its own links, so no entry of a value's list is read.
#[test]
fn erasing_users_of_one_value_reads_none_of_its_other_uses() {
    let erase = |batch: bool| {
        let mut module = module(&adds(400));
        let f = function(&mut module);
        let gone: Vec<InstId> = (0..400).step_by(2).map(|i| named(f, &format!("x{i}")).0).collect();
        crate::uselist::WALKED.with(|walked| walked.set(0));
        if batch {
            f.erase_all(&gone).expect("unused");
        } else {
            gone.iter().rev().for_each(|&one| f.erase(one).expect("unused"));
        }
        let walked = crate::uselist::WALKED.with(|walked| walked.get());
        assert!(f.check_uses().is_empty());
        (print::module(&module), walked)
    };
    let (one_by_one, many_scans) = erase(false);
    let (together, few_scans) = erase(true);
    assert_eq!(together, one_by_one);
    assert_eq!((many_scans, few_scans), (0, 0), "list entries read erasing 200 of 400 users of one value");
}

fn branches(n: usize) -> String {
    let body: String = (0..n).map(|i| format!("b{i}:\n  br label %end\n")).collect();
    format!("define void @f() {{\nentry:\n  br label %end\n{body}end:\n  ret void\n}}\n")
}

/// The terminators naming one block were looked for in its list of uses, as
/// the values' were: 200 branches to `%end` that go were 200 scans of up to
/// 400 uses. A block's uses unlink through their own links too.
#[test]
fn erasing_branches_to_one_block_reads_none_of_its_other_uses() {
    let mut module = module(&branches(400));
    let f = function(&mut module);
    let layout = f.layout().to_vec();
    let gone: Vec<InstId> = (0..400).step_by(2).map(|i| f.terminator(layout[1 + i]).expect("a branch")).collect();
    crate::uselist::WALKED.with(|walked| walked.set(0));
    gone.iter().rev().for_each(|&one| f.erase(one).expect("unused"));
    let walked = crate::uselist::WALKED.with(|walked| walked.get());
    assert!(f.check_uses().is_empty());
    // `sole_successor_start` looks at two entries of the list, never the whole.
    assert!(walked <= 2 * 200, "{walked} list entries read erasing 200 of 400 branches to one block");
}

/// A phi's incoming blocks were on the block's use list beside the
/// terminators naming it: asking for the single predecessor of a block with 30
/// phis walked 31 entries, the pin's `getSinglePredecessor` one or two
/// (`isImpliedByDomCondition` 120 Ir a call against 7). A phi's edges are
/// kept apart, as LLVM keeps them beside the phi.
#[test]
fn a_blocks_predecessors_do_not_walk_the_phis_that_name_it() {
    let phis: String = (0..30).map(|i| format!("  %p{i} = phi i32 [ {i}, %entry ]\n")).collect();
    let mut module = module(&format!("define void @f() {{\nentry:\n  br label %end\nend:\n{phis}  ret void\n}}\n"));
    let f = function(&mut module);
    let end = f.layout()[1];
    crate::uselist::WALKED.with(|walked| walked.set(0));
    let single = f.sole_predecessor(end);
    let walked = crate::uselist::WALKED.with(|walked| walked.get());
    assert_eq!(single, Some(f.layout()[0]));
    assert!(walked <= 2, "{walked} list entries read for the one predecessor of a block named by 30 phis");
    assert!(f.check_uses().is_empty());
    assert_eq!(f.phi_block_users(end).iter().count(), 0);
    assert_eq!(f.phi_block_users(f.layout()[0]).iter().count(), 30);
}

/// A branch's targets were read by the scan that serves a switch (a match, a
/// scan for a repeat and a push for each operand: 101 Ir a call, 64 K calls in
/// d_faces -O2). They are read straight into the list, a conditional branch to
/// one block twice naming it once.
#[test]
fn successors_of_a_branch_take_no_scan() {
    let mut module = module(
        "define void @f(i1 %c) {\nentry:\n  br i1 %c, label %a, label %b\na:\n  br i1 %c, label %b, label %b\nb:\n  br label %entry\n}\n",
    );
    let f = function(&mut module);
    let layout = f.layout().to_vec();
    crate::module::GENERIC_SUCCESSOR_SCANS.with(|scans| scans.set(0));
    let found: Vec<Vec<_>> = layout.iter().map(|&at| f.successors(at).into_iter().collect()).collect();
    let scans = crate::module::GENERIC_SUCCESSOR_SCANS.with(std::cell::Cell::get);
    assert_eq!(found, [vec![layout[1], layout[2]], vec![layout[2]], vec![layout[0]]]);
    assert_eq!(scans, 0, "branches read the long way");
}

/// The phis of its successor named a block in its list of uses, where LLVM
/// keeps a phi's incoming blocks apart: `predecessors` of a block whose
/// successor has 40 phis read 41 entries on every ask. Its list holds
/// terminators only.
#[test]
fn predecessors_of_a_block_read_none_of_its_phis() {
    let phis: String = (0..40).map(|i| format!("  %p{i} = phi i32 [ {i}, %mid ]\n")).collect();
    let text =
        format!("define void @f() {{\nentry:\n  br label %mid\nmid:\n  br label %end\nend:\n{phis}  ret void\n}}\n");
    let mut module = module(&text);
    let f = function(&mut module);
    let mid = f.layout()[1];
    crate::uselist::WALKED.with(|walked| walked.set(0));
    let preds = f.predecessors(mid);
    let sole = f.sole_predecessor(mid);
    let walked = crate::uselist::WALKED.with(|walked| walked.get());
    assert_eq!((preds.len(), sole.is_some()), (1, true));
    assert!(walked <= 2, "{walked} list entries read asking the predecessors of a block named by 40 phis");
}

/// A block is a `Value` in LLVM and its predecessors are its use list, the
/// newest use first: `predecessors` ran oldest first.
#[test]
fn predecessors_run_newest_first() {
    let mut module = module(&branches(3));
    let f = function(&mut module);
    let layout = f.layout().to_vec();
    let end = *layout.last().expect("the end block");
    assert_eq!(f.predecessors(end).iter().copied().collect::<Vec<_>>(), [layout[3], layout[2], layout[1], layout[0]]);
}

/// LLVM's `Value::addUse` puts the new use at the head: `users()` runs newest
/// first, where it ran oldest first, and `replace_all_uses_with` follows it.
#[test]
fn users_run_newest_first() {
    let mut module = module(
        "define i16 @f(i16 %a) {\nentry:\n  %p = add i16 %a, 1\n  %q = add i16 %a, 2\n  %r = add i16 %a, 3\n  ret i16 %r\n}\n",
    );
    let f = function(&mut module);
    let (p, _) = named(f, "p");
    let (q, _) = named(f, "q");
    let (r, _) = named(f, "r");
    let a = f.parameters()[0];
    let users: Vec<InstId> = f.users(a).iter().map(|one| one.user).collect();
    assert_eq!(users, [r, q, p], "the parameter's users, newest first");
}

#[test]
fn erase_all_refuses_a_result_something_outside_it_uses() {
    let mut module =
        module("define i16 @f(i16 %a) {\nentry:\n  %x = add i16 %a, 1\n  %y = mul i16 %x, 2\n  ret i16 %a\n}\n");
    let f = function(&mut module);
    let (x, _) = named(f, "x");
    assert!(f.erase_all(&[x]).is_err());
    let (y, _) = named(f, "y");
    f.erase_all(&[x, y]).expect("y uses x, both go");
    assert!(f.is_erased(x) && f.is_erased(y) && f.check_uses().is_empty());
}

/// A terminator that names many blocks lists each once, in operand order,
/// however many: the list of a switch of N cases was found by a scan of what
/// was listed, N^2 for N distinct targets.
#[test]
fn a_terminators_successors_and_a_blocks_predecessors_are_listed_once_each_in_order() {
    for cases in [3_usize, 40, 300] {
        let mut text = String::from("define i32 @f(i32 %a) {\nentry:\n  switch i32 %a, label %d [\n");
        // Every case but the last pair goes to its own block; the others
        // repeat.
        for k in 0..cases {
            text += &format!("    i32 {k}, label %b{}\n", if k % 3 == 2 { k - 1 } else { k });
        }
        text += "  ]\nd:\n  ret i32 0\n";
        for k in 0..cases {
            text += &format!("b{k}:\n  br label %d\n");
        }
        text += "}\n";
        let mut m = module(&text);
        let f = function(&mut m);
        let entry = f.entry().expect("an entry");
        let found = f.successors(entry);
        let names: Vec<String> = found.iter().map(|&block| f.block(block).name.clone().expect("a name")).collect();
        let mut expected = vec!["d".to_owned()];
        for k in 0..cases {
            let name = format!("b{}", if k % 3 == 2 { k - 1 } else { k });
            if !expected.contains(&name) {
                expected.push(name);
            }
        }
        assert_eq!(names, expected, "{cases} cases");
        let into_d = f.predecessors(found[0]);
        assert_eq!(into_d.len(), 1 + cases, "{cases} cases: the entry, and each b block once");
    }
}

/// `sole_predecessor` is `predecessors` of exactly one block, said without the
/// list: a switch naming a block twice and a phi naming its predecessor are
/// still one; two terminators are two; none is none.
#[test]
fn a_block_has_a_sole_predecessor_when_one_block_names_it() {
    let text = "define i32 @f(i32 %a) {
entry:
  switch i32 %a, label %twice [ i32 1, label %twice
                                i32 2, label %join ]
twice:
  br label %join
join:
  %p = phi i32 [ 1, %entry ], [ 2, %twice ]
  ret i32 %p
orphan:
  ret i32 0
}
";
    let mut m = module(text);
    let f = function(&mut m);
    let block =
        |name: &str| f.layout().iter().copied().find(|&b| f.block(b).name.as_deref() == Some(name)).expect(name);
    let (entry, twice, join, orphan) = (block("entry"), block("twice"), block("join"), block("orphan"));
    assert_eq!(f.sole_predecessor(twice), Some(entry), "two cases of one switch name it");
    assert_eq!(f.sole_predecessor(join), None, "the entry and `twice` name it");
    assert_eq!(f.sole_predecessor(entry), None, "no terminator names the entry");
    assert_eq!(f.sole_predecessor(orphan), None);
    for b in [entry, twice, join, orphan] {
        assert_eq!(f.sole_predecessor(b).is_some(), f.predecessors(b).len() == 1);
    }
}

/// A value read by many places goes to another in one step, and a phi of many
/// inputs is cleared in one: the use lists agree with the operands after both.
/// (A scan of the list for each use made the first U^2 and, over the dead phis
/// of a promotion, the second N^3.)
#[test]
fn a_value_replaced_and_a_phi_cleared_leave_the_use_lists_true() {
    let n = 60;
    let mut text = String::from("define i32 @f(i32 %a, i32 %b) {\nentry:\n  switch i32 %a, label %d [\n");
    for k in 0..n {
        text += &format!("    i32 {k}, label %b{k}\n");
    }
    text += "  ]\n";
    for k in 0..n {
        text += &format!("b{k}:\n  %x{k} = add i32 %a, {k}\n  br label %d\n");
    }
    text += "d:\n  %p = phi i32 ";
    text += &(0..n).map(|k| format!("[%b, %b{k}]")).collect::<Vec<_>>().join(", ");
    text += ", [0, %entry]\n  %q = add i32 %p, %p\n  %r = mul i32 %q, %b\n  ret i32 %r\n}\n";
    let mut m = module(&text);
    let f = function(&mut m);
    let (_, b) = (0, f.parameters()[1]);
    let (_, a) = (0, f.parameters()[0]);
    assert!(f.users(b).has_n_uses_or_more(n + 1), "premise: %b is read many times");
    f.replace_value(b, Operand::Value(a));
    assert!(f.users(b).is_empty());
    assert!(f.check_uses().is_empty(), "{:?}", f.check_uses());
    let (phi, _) = named(f, "p");
    f.set_operands(phi, Vec::new());
    assert!(f.check_uses().is_empty(), "{:?}", f.check_uses());
    assert!(f.instruction(phi).operands.is_empty());
}

/// Names looked at to make `count` instructions that all ask for the name `t`.
fn naming_cost(count: usize) -> usize {
    let mut module = module(TEXT);
    let i16 = module.context.types.int(16);
    let one = module.context.int(i16, 1);
    let f = function(&mut module);
    crate::edit::NAMES_LOOKED_AT.with(|looked| looked.set(0));
    for _ in 0..count {
        let operand = Operand::Constant(one);
        f.create_instruction(Opcode::Binary(BinaryOp::Add), i16, vec![operand, operand], Flags::default(), Some("t"));
    }
    crate::edit::NAMES_LOOKED_AT.with(std::cell::Cell::get)
}

/// `hir to mir` of 1024 `switch` cases cost 13,840 Minstr, 15x more than the
/// scan it replaced, because each new name hashed every name of the function:
/// four times the names must cost about four times the looks, not sixteen.
/// (Counted, not timed: a test may not read the clock.)
#[test]
fn naming_a_value_does_not_look_at_every_name_of_the_function() {
    let (few, many) = (naming_cost(600), naming_cost(2400));
    assert!(many < 6 * few, "600 names looked at {few} times, 2400 names {many}");
}

#[test]
fn a_name_is_free_again_when_its_block_is_erased() {
    let mut module = module(TEXT);
    let f = function(&mut module);
    let first = f.create_block(Some("b"));
    assert_eq!(f.create_block(Some("b")), BlockId(2));
    assert_eq!(f.block(BlockId(2)).name.as_deref(), Some("b1"));
    f.erase_block(first).unwrap();
    let again = f.create_block(Some("b"));
    assert_eq!(f.block(again).name.as_deref(), Some("b"));
}

/// `is_placed` answers in O(1) what a scan of the layout did
/// (`induction::basics` asked it of every loop's header: N blocks scanned per
/// loop, a quarter of `analysis bounded` at N=4096): it follows create, insert,
/// erase and `delete_body`.
#[test]
fn a_block_is_placed_from_its_insertion_to_its_erasure() {
    let text = "define i16 @f(i1 %c) {\nentry:\n  br i1 %c, label %yes, label %no\n\nyes:\n  ret i16 1\n\nno:\n  ret i16 2\n}\n";
    let mut module = module(text);
    let f = function(&mut module);
    let entry = f.entry().unwrap();
    let no = f.successors(entry)[1];
    let made = f.create_block(None);
    let agrees =
        |f: &Function| (0..=made.0).map(BlockId).all(|block| f.is_placed(block) == f.layout().contains(&block));
    assert!(!f.is_placed(made) && agrees(f), "a new block is in no layout");
    f.insert_block(made, Some(entry)).expect("placed");
    assert!(f.is_placed(made) && agrees(f), "an inserted one is");
    let ret = f.terminator(no).unwrap();
    f.move_to(ret, Position::End(made)).expect("moved");
    f.replace_block_uses_with(no, made);
    f.erase_block(no).expect("empty");
    assert!(!f.is_placed(no) && agrees(f), "an erased one is not");
    f.delete_body();
    assert!(!f.is_placed(made) && !f.is_placed(entry) && agrees(f), "a deleted body has none");
}

/// The list API of LLVM's `BasicBlock`: ends, neighbours and order of a block's
/// instructions, true again after an edit.
#[test]
fn a_block_answers_ends_neighbours_and_order_after_every_edit() {
    let mut module = module(TEXT);
    let f = function(&mut module);
    let entry = f.entry().unwrap();
    let (x, _) = named(f, "x");
    let (y, _) = named(f, "y");
    let ret = f.terminator(entry).unwrap();
    assert_eq!((f.block(entry).len(), f.block(entry).first(), f.block(entry).last()), (3, Some(x), Some(ret)));
    assert_eq!(f.iter_block(entry).collect::<Vec<_>>(), [x, y, ret]);
    assert_eq!((f.prev_in_block(x), f.next_in_block(x), f.next_in_block(ret)), (None, Some(y), None));
    assert!(f.comes_before(x, y) && !f.comes_before(y, x) && !f.comes_before(x, x));
    // The numbers were taken: an edit must make them false.
    f.move_to(x, Position::Before(ret)).expect("moved");
    assert!(f.comes_before(y, x), "x moved after y");
    assert_eq!((f.prev_in_block(x), f.next_in_block(y)), (Some(y), Some(x)));
}

#[test]
fn an_instruction_goes_after_another() {
    let mut module = module(TEXT);
    let f = function(&mut module);
    let (x, _) = named(f, "x");
    let (y, _) = named(f, "y");
    f.move_to(x, Position::After(y)).expect("moved");
    assert_eq!(f.iter_block(f.entry().unwrap()).take(2).collect::<Vec<_>>(), [y, x]);
}

/// A run moved as one is the run moved one by one.
#[test]
fn a_run_spliced_is_the_run_moved_one_by_one() {
    let run = |together: bool| {
        let mut module = module(&adds(400));
        let f = function(&mut module);
        let moved: Vec<InstId> = (0..100).map(|i| named(f, &format!("x{i}")).0).collect();
        let entry = f.entry().unwrap();
        let ret = f.terminator(entry).unwrap();
        if together {
            f.splice(&moved, Position::Before(ret)).expect("spliced");
        } else {
            moved.iter().for_each(|&one| f.move_to(one, Position::Before(ret)).expect("moved"));
        }
        assert!(f.check_uses().is_empty());
        print::module(&module)
    };
    assert_eq!(run(true), run(false));
}

/// Erasing several empty blocks at once leaves the others in order, and a
/// block that still holds an instruction refuses the whole call.
#[test]
fn erase_blocks_removes_each_empty_block_in_one_pass() {
    let mut module = module("define void @f() {\nentry:\n  br label %c\n\nc:\n  ret void\n}\n");
    let f = function(&mut module);
    let (a, b) = (f.create_block(Some("a")), f.create_block(Some("b")));
    f.insert_block(a, None).expect("placed");
    f.insert_block(b, None).expect("placed");
    let entry = f.entry().unwrap();
    assert!(f.erase_blocks(&[a, entry]).is_err());
    assert!(f.is_placed(a));
    f.erase_blocks(&[a, b]).expect("empty and unnamed");
    assert_eq!(f.layout().len(), 2);
    assert!(!f.is_placed(a) && !f.is_placed(b));
}
/// `set_icmp_predicate` is `CmpInst::setPredicate`: the compare reads the same
/// operands under another predicate, and the log tells of the rewrite.
#[test]
fn an_icmp_takes_another_predicate() {
    let mut module = module("define i1 @f(i16 %a) {\nentry:\n  %c = icmp slt i16 %a, 5\n  ret i1 %c\n}\n");
    let f = function(&mut module);
    let (c, _) = named(f, "c");
    f.take_changes();
    f.set_icmp_predicate(c, crate::opcode::IntPredicate::Ult);
    assert_eq!(f.instruction(c).opcode, Opcode::ICmp(crate::opcode::IntPredicate::Ult));
    assert_eq!(f.take_changes(), vec![Change::Rewritten(c)]);
}

/// Taking `!range` off a load changes what is known of its result, so the log
/// tells of the rewrite: EarlyCSE took it off the surviving load of two and
/// logged nothing, the manager kept the ranger's answers of the old range and
/// `LLRM_CHECK_REPLAY` panicked on bintree.nib (#2369).
#[test]
fn test_unannotate_logs_the_rewrite_of_the_instruction() {
    let mut module = module(
        "define i16 @f(ptr %p) {\nentry:\n  %v = load i16, ptr %p, !range !0\n  ret i16 %v\n}\n\n!0 = !{i16 0, i16 -32768}\n",
    );
    let f = function(&mut module);
    let (v, _) = named(f, "v");
    f.take_changes();
    f.unannotate(v, "range");
    assert!(f.instruction(v).metadata.is_empty(), "premise: the range is gone");
    assert_eq!(f.take_changes(), vec![Change::Rewritten(v)]);
    f.unannotate(v, "range");
    assert!(f.take_changes().is_empty(), "nothing was taken off the second time");
}

/// `take_name` is `Value::takeName`: the new value is named what the old one
/// was and the old one has none, as SCCP's `sext` -> `zext nneg` keeps `%s`.
#[test]
fn test_take_name_moves_the_name_to_the_other_value() {
    let mut module =
        module("define i16 @f(i16 %a) {\nentry:\n  %s = add i16 %a, 1\n  %t = add i16 %a, 2\n  ret i16 %t\n}\n");
    let f = function(&mut module);
    let ((s, sv), (t, tv)) = (named(f, "s"), named(f, "t"));
    f.take_name(sv, tv);
    assert_eq!((f.value(sv).name.clone(), f.value(tv).name.clone()), (None, Some("s".to_string())));
    let _ = (s, t);
}

/// A block cloned without a remap reads the original's values; after the
/// remap its instructions read the copy's, and the original is untouched.
#[test]
fn a_cloned_block_reads_its_own_values_once_remapped() {
    let mut module = module(TEXT);
    let f = function(&mut module);
    let entry = f.entry().unwrap();
    let (_, x) = named(f, "x");
    let mut map = ValueMap::default();
    let copy = f.clone_block(entry, &mut map);
    f.insert_block(copy, Some(entry)).expect("placed");
    let Operand::Value(x_copy) = map.values[&x] else { panic!("a value") };
    assert_ne!(x_copy, x);
    assert_eq!(map.blocks[&entry], copy);
    for inst in f.iter_block(copy).collect::<Vec<_>>() {
        f.remap_instruction(inst, &map);
    }
    let mul = f.iter_block(copy).nth(1).unwrap();
    assert_eq!(f.instruction(mul).operands[0], Operand::Value(x_copy));
    let (y, _) = named(f, "y");
    assert_eq!(f.instruction(y).operands[0], Operand::Value(x));
    assert!(f.check_uses().is_empty(), "{:?}", f.check_uses());
}

const JOIN: &str = "define i16 @f(i1 %c, i16 %a, i16 %b) {\nentry:\n  br i1 %c, label %p1, label %p2\np1:\n  switch i16 %a, label %join [\n    i16 0, label %p3\n  ]\np2:\n  br label %join\np3:\n  br label %join\njoin:\n  %m = phi i16 [ %a, %p1 ], [ %b, %p2 ], [ 7, %p3 ]\n  ret i16 %m\n}\n";

/// `removeIncomingValue` moves the LAST input into the removed one's slot; the
/// order-preserving removal left `[%b, %p2], [7, %p3]` where `opt` leaves
/// `[7, %p3], [%b, %p2]`, so every later phi walk differed from LLVM's.
#[test]
fn removing_a_predecessor_moves_the_last_phi_input_into_its_slot() {
    let mut module = module(JOIN);
    let (context, f) = module.function_mut("f").expect("@f");
    let block = |f: &Function, name: &str| {
        f.layout().iter().copied().find(|&b| f.block(b).name.as_deref() == Some(name)).expect(name)
    };
    let (join, p1) = (block(f, "join"), block(f, "p1"));
    let (m, _) = named(f, "m");
    f.predecessor_removed(context, join, p1, true);
    let from: Vec<Operand> = f.instruction(m).operands.chunks(2).map(|pair| pair[1]).collect();
    assert_eq!(from, vec![Operand::Block(block(f, "p3")), Operand::Block(block(f, "p2"))]);
}

/// With every other input the phi itself, `hasConstantValue` is poison and
/// the phi goes; the skip-self version returned nothing and left the phi.
#[test]
fn a_phi_left_with_only_itself_becomes_poison() {
    let text = "define i16 @f(i1 %c, i16 %a) {\nentry:\n  br i1 %c, label %loop, label %out\nloop:\n  %m = phi i16 [ %a, %entry ], [ %m, %loop ], [ %m, %loop ]\n  br i1 %c, label %loop, label %out\nout:\n  ret i16 %a\n}\n";
    let mut module = module(text);
    let (context, f) = module.function_mut("f").expect("@f");
    let loop_block = f.layout().iter().copied().find(|&b| f.block(b).name.as_deref() == Some("loop")).unwrap();
    let entry = f.entry().unwrap();
    let (m, _) = named(f, "m");
    f.predecessor_removed(context, loop_block, entry, false);
    assert!(f.is_erased(m), "the phi of itself alone is replaced by poison and erased");
}

/// `setName` on a name a live value holds appends a number; the unnumbered
/// copy of a cloned instruction would otherwise share its original's name and
/// print as two definitions of `%x`.
#[test]
fn a_name_a_value_holds_gets_a_number_when_set_again() {
    let mut module = module(TEXT);
    let f = function(&mut module);
    let (_, x) = named(f, "x");
    let (copy, _) = named(f, "y");
    let duplicate = f.clone_instruction(copy);
    f.insert(duplicate, Position::Before(copy)).expect("placed");
    let value = f.instruction(duplicate).result.expect("a value");
    f.set_name(value, "x");
    assert_eq!(f.value(x).name.as_deref(), Some("x"));
    assert_eq!(f.value(value).name.as_deref(), Some("x1"));
}

/// `moveAfter` puts a block right behind another: merging an entry block into
/// its successor leaves the successor first in the layout, or the function has
/// no entry.
#[test]
fn a_block_moved_after_another_stands_right_behind_it() {
    let mut module = module("define void @f() {\na:\n  br label %b\nb:\n  br label %c\nc:\n  ret void\n}\n");
    let f = function(&mut module);
    let layout = f.layout().to_vec();
    f.move_block_after(layout[2], layout[0]).expect("placed");
    assert_eq!(f.layout(), &[layout[0], layout[2], layout[1]]);
    f.move_block_after(layout[0], layout[1]).expect("placed");
    assert_eq!(f.layout(), &[layout[2], layout[1], layout[0]]);
}

/// A block created with `InsertBefore` stands right in front of its block in
/// the layout: DFAJumpThreading's unfolded blocks come out in the pin's order.
#[test]
fn a_block_inserted_before_another_stands_right_in_front_of_it() {
    let mut module = module("define void @f() {\na:\n  br label %b\nb:\n  br label %c\nc:\n  ret void\n}\n");
    let f = function(&mut module);
    let layout = f.layout().to_vec();
    let fresh = f.create_block(None);
    f.insert_block_before(fresh, layout[2]).expect("placed");
    assert_eq!(f.layout(), &[layout[0], layout[1], fresh, layout[2]]);
    let first = f.create_block(None);
    f.insert_block_before(first, layout[0]).expect("placed");
    assert_eq!(f.layout()[0], first);
    assert!(f.insert_block_before(first, layout[1]).is_err(), "a placed block is not placed again");
}

/// LLVM's `hasOneUse`, `hasNUses` and `hasNUsesOrMore` on a value with 0, 1, 2
/// and 3 uses: the counts are read from the list at its boundaries, `len` is
/// not there.
#[test]
fn a_use_list_answers_has_n_uses_at_its_boundaries() {
    let mut module = module(
        "define i16 @f(i16 %a) {\nentry:\n  %x = add i16 %a, 1\n  %y = add i16 %a, 2\n  %z = add i16 %a, 3\n  ret i16 %a\n}\n",
    );
    let f = function(&mut module);
    let a = f.parameters()[0];
    // %a: three adds and the ret.
    assert!(f.users(a).has_n_uses(4) && !f.users(a).has_n_uses(3) && !f.users(a).has_n_uses(5));
    assert!(f.users(a).has_n_uses_or_more(4) && !f.users(a).has_n_uses_or_more(5) && f.users(a).has_n_uses_or_more(0));
    let (_, x) = named(f, "x");
    assert!(f.users(x).is_empty() && f.users(x).has_n_uses(0) && !f.users(x).has_one_use());
    assert!(!f.users(x).has_n_uses_or_more(1) && f.users(x).has_n_uses_or_more(0));
    let ret = f.terminator(f.entry().unwrap()).unwrap();
    assert!(f.users(a).snapshot().iter().any(|one| one.user == ret));
    let (y, _) = named(f, "y");
    f.set_operand(ret, 0, Operand::Value(named(f, "y").1));
    assert!(f.users(named(f, "y").1).has_one_use() && f.users(a).has_n_uses(3), "{y:?}");
}

/// A copied block was named by the block it was copied from and a suffix; the
/// name is unique, as a value's is, and the block it is made for has none.
#[test]
fn test_set_block_name_names_a_block_apart_from_the_names_held() {
    let mut module =
        module("define i16 @f(i1 %c) {\nentry:\n  br i1 %c, label %a, label %b\na:\n  ret i16 1\nb:\n  ret i16 2\n}\n");
    let f = function(&mut module);
    let copy = f.create_block(None);
    assert_eq!(f.block(copy).name, None);
    f.set_block_name(copy, "a");
    assert_eq!(f.block(copy).name.as_deref(), Some("a1"), "`a` is held by a block");
    let another = f.create_block(None);
    f.set_block_name(another, "a.jt1");
    f.set_block_name(f.layout()[1], "a.jt1");
    assert_ne!(f.block(f.layout()[1]).name.as_deref(), f.block(another).name.as_deref());
}

/// `BasicBlock::takeName` and `setName`: the block that takes a name is named
/// it, the other goes unnamed, and a name in use gets a number appended.
#[test]
fn a_block_takes_the_name_of_another_and_a_taken_name_gets_a_number() {
    let mut parsed = module("define i16 @f(i16 %a) {\nentry:\n  br label %next\n\nnext:\n  ret i16 %a\n}\n");
    let f = function(&mut parsed);
    let (entry, next) = (f.layout()[0], f.layout()[1]);
    let fresh = f.create_block(None);
    f.take_block_name(entry, fresh);
    assert_eq!((f.block(entry).name.as_deref(), f.block(fresh).name.as_deref()), (None, Some("entry")));
    f.set_block_name(entry, "next");
    assert_eq!(f.block(entry).name.as_deref(), Some("next1"));
    assert_eq!(f.block(next).name.as_deref(), Some("next"));
}

/// A `switch` with two cases to one block is two edges: a phi there has two
/// inputs from it, and the exit phis of lcssa, lcssamerges and LSR gave one.
#[test]
fn a_switch_with_two_cases_to_a_block_is_two_edge_predecessors() {
    let text = "define i16 @f(i16 %x) {\nentry:\n  switch i16 %x, label %d [\n    i16 1, label %t\n    i16 2, label %t\n  ]\n\nt:\n  ret i16 1\n\nd:\n  ret i16 2\n}\n";
    let mut module = module(text);
    let f = function(&mut module);
    let entry = f.entry().unwrap();
    let t = f.successors(entry)[1];
    assert_eq!(f.predecessors(t), [entry]);
    assert_eq!(f.edge_predecessors(t), [entry, entry]);
}

/// `hir to mir` grew a function's instruction arena by doubling (98 growths,
/// 1.09 M Ir on d_faces) though the builder knew the count: after
/// `reserve_instructions(n)`, `n` instructions move nothing.
#[test]
fn test_reserved_instructions_do_not_move_the_arena() {
    let mut module = module(TEXT);
    let i16 = module.context.types.int(16);
    let one = module.context.int(i16, 1);
    let f = function(&mut module);
    f.reserve_instructions(100);
    let at = f.instruction_capacity();
    assert!(at >= f.instruction_count() + 100);
    for _ in 0..100 {
        let operand = Operand::Constant(one);
        f.create_instruction(Opcode::Binary(BinaryOp::Add), i16, vec![operand, operand], Flags::default(), None);
    }
    assert_eq!(f.instruction_capacity(), at);
}

/// Asking whether a switch of 256 cases has one successor listed its 256
/// targets (`Function::successors` 12 K Ir a call, 8,720 calls in a 256-state
/// machine: SimplifyCFG x3.5 a doubling, the pin's x1.8). It stops at the
/// second distinct target.
#[test]
fn the_sole_successor_of_a_switch_stops_at_a_second_target() {
    let cases: String = (0..256).map(|i| format!("    i32 {i}, label %t{}\n", i % 2)).collect();
    let text = format!(
        "define void @f(i32 %x) {{\nentry:\n  switch i32 %x, label %t0 [\n{cases}  ]\nt0:\n  br label %t1\nt1:\n  ret void\n}}\n"
    );
    let mut module = module(&text);
    let f = function(&mut module);
    let layout = f.layout().to_vec();
    crate::module::GENERIC_SUCCESSOR_SCANS.with(|scans| scans.set(0));
    let (switch, jump) = (f.sole_successor(layout[0]), f.sole_successor(layout[1]));
    let scans = crate::module::GENERIC_SUCCESSOR_SCANS.with(std::cell::Cell::get);
    assert_eq!((switch, jump), (None, Some(layout[2])));
    assert_eq!(scans, 0, "the successors of the switch were listed whole");
}

/// Removing one entry of a phi of 256 arms unlinked and linked all 512
/// operands (`set_operands` 64 K Ir a call, 1,050 calls in a 256-state
/// machine). The arms before the removed one keep their uses; the use lists
/// stay consistent.
#[test]
fn rewriting_a_phi_after_its_first_arms_relinks_only_what_changed() {
    let arms: String = (0..256).map(|i| format!("[ {i}, %e ], ")).collect();
    let text = format!(
        "define i32 @f() {{\nentry:\n  br label %m\ne:\n  br label %m\nm:\n  %p = phi i32 {}\n  ret i32 %p\n}}\n",
        arms.trim_end_matches(", ")
    );
    let mut module = module(&text);
    let f = function(&mut module);
    let m = f.layout()[2];
    let phi = f.iter_block(m).next().expect("the phi");
    let mut operands = f.instruction(phi).operands.clone();
    let last = operands.len();
    operands.truncate(last - 2);
    crate::uselist::LINKS.with(|links| links.set(0));
    f.set_operands(phi, operands);
    let links = crate::uselist::LINKS.with(std::cell::Cell::get);
    assert!(links <= 4, "{links} operands relinked for a rewrite that changed the end");
    assert!(f.check_uses().is_empty());
}
