use crate::function::{Operand, ValueId, ValueInfo};
use crate::interpret::{Trap, run};
use crate::types::MirContext;
use crate::verify::{verify, verify_module};
use crate::{View, parse, print};

const SAMPLE: &str = "\
module sample

function sum(n: i16) -> i16
entry:
    goto head

head:
    i: i16 = phi
        from entry: 0
        from body: next
    total: i16 = phi
        from entry: 0
        from body: added
    more = compare.signed i < n
    if more goto body else done

body:
    next = add.wrap i, 1
    added = add.wrap total, next
    goto head

done:
    return total
end

function widen(x: i16, flag: i1) -> i32
entry:
    wide: i32 = sext x
    other: i32 = zext -1:i16
    picked = select flag, wide, other
    if flag goto same as left else same as right

same:
    joined: i32 = phi
        from left: picked
        from right: 1
    return joined
end
";

fn parsed(text: &str) -> (MirContext, crate::Module) {
    let mut context = MirContext::new();
    let module = parse::module(&mut context, text).unwrap_or_else(|error| panic!("{error}"));
    (context, module)
}

/// The verifier's complaints about the one function in `body`.
fn complaints(body: &str) -> Vec<String> {
    let (context, module) = parsed(&format!("module m\n\n{body}"));
    verify_module(&context, &module)
}

#[test]
fn the_crate_depends_on_nothing() {
    // MIR may name no decoder, object file or target; the dependency graph enforces it.
    let manifest = include_str!("../Cargo.toml");
    let dependencies = manifest.split("[dependencies]").nth(1).expect("a [dependencies] table");
    let entries: Vec<&str> =
        dependencies.lines().map(str::trim).take_while(|line| !line.starts_with('[')).filter(|line| !line.is_empty() && !line.starts_with('#')).collect();
    assert!(entries.is_empty(), "{entries:?}");
}

#[test]
fn normal_text_round_trips_byte_stably() {
    let (context, module) = parsed(SAMPLE);
    assert!(verify_module(&context, &module).is_empty(), "{:?}", verify_module(&context, &module));
    assert_eq!(print::module(&context, &module, View::Normal), SAMPLE);
}

#[test]
fn the_normalized_view_ignores_names() {
    let renamed = SAMPLE.replace("total", "running").replace("head", "loop").replace("next", "step");
    let (context, one) = parsed(SAMPLE);
    let (other_context, other) = parsed(&renamed);
    assert_ne!(print::module(&context, &one, View::Normal), print::module(&other_context, &other, View::Normal));
    assert_eq!(print::module(&context, &one, View::Normalized), print::module(&other_context, &other, View::Normalized));
    let normalized = print::module(&context, &one, View::Normalized);
    let (again_context, again) = parsed(&normalized);
    assert_eq!(print::module(&again_context, &again, View::Normalized), normalized);
}

#[test]
fn the_interpreter_wraps_at_the_type_width() {
    let (context, module) = parsed(SAMPLE);
    let sum = &module.functions[0];
    assert_eq!(run(&context, sum, &[10], 10_000), Ok(vec![55]));
    assert_eq!(run(&context, sum, &[400], 10_000), Ok(vec![80_200 % 65_536]));
    assert_eq!(run(&context, sum, &[400], 100), Err(Trap::OutOfFuel));
}

#[test]
fn the_interpreter_takes_each_edge_into_a_shared_block_apart() {
    let (context, module) = parsed(SAMPLE);
    let widen = &module.functions[1];
    assert_eq!(run(&context, widen, &[0xFFFE, 1], 100), Ok(vec![0xFFFF_FFFE]), "sext -2, along left");
    assert_eq!(run(&context, widen, &[0xFFFE, 0], 100), Ok(vec![1]), "along right");
}

#[test]
fn signed_and_unsigned_comparisons_differ_below_zero() {
    let (context, module) = parsed(
        "module m\n\nfunction below(a: i8, b: i8) -> (i1, i1)\nentry:\n    s = compare.signed a < b\n    u = compare.unsigned a < b\n    return s, u\nend\n",
    );
    assert_eq!(run(&context, &module.functions[0], &[0xFF, 1], 10), Ok(vec![1, 0]));
}

#[test]
fn an_undefined_use_is_a_verifier_error() {
    let (mut context, mut module) = parsed(SAMPLE);
    let sum = &mut module.functions[0];
    let ghost = ValueId(sum.values.len() as u32);
    sum.values.push(ValueInfo { ty: context.int(16), name: Some("ghost".to_owned()) });
    let returned = sum.blocks[3].instructions.last_mut().unwrap();
    returned.operands[0] = Operand::Value(ghost);
    let found = verify(&context, sum);
    assert!(found.iter().any(|one| one.contains("uses ghost, which nothing defines")), "{found:?}");
}

#[test]
fn a_use_its_definition_does_not_dominate_is_refused() {
    let found = complaints(
        "function f(c: i1) -> i16\nentry:\n    if c goto yes else no\n\nyes:\n    one: i16 = copy 1\n    goto join\n\nno:\n    goto join\n\njoin:\n    return one\nend\n",
    );
    assert_eq!(found, ["f: one is used in join where its definition does not dominate"]);
}

#[test]
fn a_phi_reads_along_every_edge_into_its_block() {
    let found = complaints(
        "function f(c: i1) -> i16\nentry:\n    if c goto yes else join\n\nyes:\n    goto join\n\njoin:\n    x: i16 = phi from yes: 1\n    return x\nend\n",
    );
    assert_eq!(found, ["f: phi (#2) has no input along edge1"]);
}

#[test]
fn operand_types_follow_the_opcode_schema() {
    let found = complaints(
        "function f(a: i16, b: i32) -> i16\nentry:\n    c = compare a == b\n    d: i8 = sext a\n    return a\nend\n",
    );
    assert_eq!(found, ["f: compare (#0) has a i32 operand where it needs i16", "f: sext (#1): sext cannot take i16 to i8"]);
}

#[test]
fn a_terminator_ends_its_block_and_nothing_follows_it() {
    let (context, mut module) = parsed("module m\n\nfunction f() -> i16\nentry:\n    return 1\nend\n");
    let function = &mut module.functions[0];
    let returned = function.blocks[0].instructions[0].clone();
    function.blocks[0].instructions.push(crate::Instruction { id: crate::InstructionId(1), ..returned });
    assert_eq!(verify(&context, function), ["block entry has a terminator before its end"]);
}

#[test]
fn a_constant_states_its_type_where_its_place_does_not() {
    let mut context = MirContext::new();
    let refused = parse::module(&mut context, "module m\n\nfunction f() -> i16\nentry:\n    x = add.wrap 1, 2\n    return x\nend\n");
    assert_eq!(refused.unwrap_err().message, "x's type cannot be read off its operands; state it");
    let (context, module) = parsed("module m\n\nfunction f() -> i16\nentry:\n    x = add.wrap 1:i16, 2\n    return x\nend\n");
    let printed = print::module(&context, &module, View::Normal);
    assert!(printed.contains("    x: i16 = add.wrap 1, 2\n"), "{printed}");
}

#[test]
fn an_undefined_name_is_a_parse_error() {
    let mut context = MirContext::new();
    let refused = parse::module(&mut context, "module m\n\nfunction f() -> i16\nentry:\n    return y\nend\n");
    assert_eq!(refused.unwrap_err().to_string(), "line 5: y is not defined");
}
