use crate::{parse, print};

const DATALAYOUT: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-i32:16-i64:16\"\n";

/// Every `.ll` under `tests/`: hand-written mappings, clang's output, and
/// with `invalid`, the ones LLVM's verifier refuses instead.
fn fixtures(invalid: bool) -> Vec<(std::path::PathBuf, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let mut out = Vec::new();
    for dir in std::fs::read_dir(&root).expect("tests/").flatten() {
        if (dir.file_name() == "invalid") != invalid {
            continue;
        }
        for file in std::fs::read_dir(dir.path()).into_iter().flatten().flatten() {
            if file.path().extension().is_some_and(|one| one == "ll") {
                out.push((file.path(), std::fs::read_to_string(file.path()).expect("readable")));
            }
        }
    }
    out
}

/// `text` read and written back.
fn round(text: &str) -> String {
    print::module(&parse::module(text).unwrap_or_else(|error| panic!("{error}\n{text}")))
}

fn refusal(text: &str) -> String {
    parse::module(text).expect_err("refused").to_string()
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
fn every_fixture_prints_to_a_fixed_point() {
    let fixtures = fixtures(false);
    assert!(fixtures.len() >= 17, "{} fixtures", fixtures.len());
    for (path, text) in fixtures {
        let module = parse::module(&text).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        assert_eq!(crate::verify::verify(&module), Vec::<String>::new(), "{}", path.display());
        let once = round(&text);
        assert_eq!(round(&once), once, "{}", path.display());
    }
}

#[test]
fn a_value_may_be_used_before_its_definition() {
    let text = format!(
        "{DATALAYOUT}\ndefine i16 @count(i16 %n) {{\nentry:\n  br label %head\n\nhead:\n  %i = phi i16 [ 0, %entry ], [ %next, %head ]\n  %next = add nsw i16 %i, 1\n  %more = icmp slt i16 %next, %n\n  br i1 %more, label %head, label %done\n\ndone:\n  ret i16 %next\n}}\n"
    );
    assert_eq!(round(&text), text);
}

#[test]
fn a_forward_use_must_agree_with_its_definition() {
    let text = "define i16 @f() {\nentry:\n  br label %b\n\nb:\n  %x = phi i16 [ 0, %entry ], [ %y, %b ]\n  %y = zext i16 %x to i32\n  br label %b\n}\n";
    assert_eq!(refusal(text), "line 7: %y is used as i16 but defined as i32");
}

#[test]
fn a_value_never_defined_is_refused() {
    let text = "define i16 @f() {\n  ret i16 %ghost\n}\n";
    assert_eq!(refusal(text), "line 2: %ghost is used but never defined");
}

#[test]
fn unnamed_values_count_the_entry_block() {
    let text = "define i16 @f(i16 %0) {\n  %2 = add i16 %0, 1\n  ret i16 %2\n}\n";
    assert_eq!(round(text), text);
    assert_eq!(refusal(&text.replace("%2", "%3")), "line 2: expected to be numbered %2");
}

#[test]
fn constructs_outside_the_subset_are_refused() {
    assert_eq!(refusal("define i16 @f() {\n  ret i16 undef\n}\n"), "line 2: `undef` is outside MIR's subset of LLVM");
    assert_eq!(refusal("@x = global fp128 zeroinitializer\n"), "line 1: `fp128` is outside MIR's subset of LLVM");
    assert_eq!(refusal("target triple = \"i386\"\n"), "line 1: `triple` is outside MIR's subset of LLVM");
}

#[test]
fn attribute_groups_may_follow_their_use() {
    let text = "declare void @stop(i16) #0\n\nattributes #0 = { noreturn memory(argmem: read) \"kind\"=\"error\" }\n";
    assert_eq!(round(text), "declare void @stop(i16) noreturn memory(argmem: read) \"kind\"=\"error\"\n");
}

#[test]
fn floating_constants_print_as_llvm_does() {
    // As LLVM 20's llvm-dis writes them.
    let text = "@a = global double 1.5\n@b = global double 0.1\n@c = global double 0x3FD5555555555555\n@d = global float 0x3FB99999A0000000\n";
    assert_eq!(round(text), "@a = global double 1.500000e+00\n@b = global double 1.000000e-01\n@c = global double 0x3FD5555555555555\n@d = global float 0x3FB99999A0000000\n");
    assert_eq!(refusal("@d = global float 0.1\n"), "line 1: 0.1 is not exactly a float");
}

#[test]
fn integer_constants_hold_their_bits_and_print_signed() {
    assert_eq!(round("@a = global i16 65535\n"), "@a = global i16 -1\n");
    assert_eq!(refusal("@a = global i16 65536\n"), "line 1: 65536 does not fit i16");
}

#[test]
fn globals_keep_their_definition_order_whatever_uses_them_first() {
    let text = "@first = global ptr @second\n@second = global i16 7\n";
    assert_eq!(round(text), text);
    assert_eq!(refusal("@first = global ptr addrspace(1) @second\n@second = global i16 7\n"), "line 1: a global in address space 0 used as ptr addrspace(1)");
}

#[test]
fn metadata_keeps_its_numbers_whatever_mentions_it_first() {
    // Numbered by first mention, an attachment's node swapped with the one
    // it refers to, and clang's loop metadata never printed the same twice.
    let text = "define void @f() {\n  ret void, !x !1\n}\n\n!0 = !{!\"first\"}\n!1 = !{!0}\n";
    assert_eq!(round(text), text);
}

/// LLVM's `initializes`, which MIR did not read.
#[test]
fn initializes_is_read_and_printed_as_llvm_spells_it() {
    let text = "declare void @f(ptr initializes((0, 4), (8, 12)), ptr initializes((-2, 0)))\n";
    assert_eq!(round(text), text);
}

#[test]
fn every_invalid_fixture_is_refused_for_its_reason() {
    let fixtures = fixtures(true);
    assert!(fixtures.len() >= 9, "{} fixtures", fixtures.len());
    for (path, text) in fixtures {
        let reason = text.lines().find_map(|line| line.strip_prefix("; invalid: ")).expect("a `; invalid:` line");
        let found = match parse::module(&text) {
            Err(error) => vec![error.to_string()],
            Ok(module) => crate::verify::verify(&module),
        };
        assert!(found.iter().any(|one| one.contains(reason)), "{}: {found:?}", path.display());
    }
}

/// LLVM replaces an intrinsic declaration's attributes with the intrinsic's
/// own; kept from the text, `cold` and `noundef` would claim what LLVM does
/// not, and a missing `memory(none)` would hide that the call is pure.
#[test]
fn an_intrinsic_declaration_takes_llvms_attributes() {
    let module = parse::module("declare i16 @llvm.smax.i16(i16 noundef, i16) cold\n").unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(print::module(&module), "declare i16 @llvm.smax.i16(i16, i16) nocallback nofree nosync nounwind speculatable willreturn memory(none)\n");
}

/// Inline assembly's registers are in its callee's name; the declaration
/// must answer them, and a block that reaches no memory keeps its place
/// among the ports as their own state does.
#[test]
fn an_inline_assembly_declaration_is_checked_against_its_name() {
    let name = "llrm.ia16.asm.cd1a.ax.cx_dx.flags.n";
    let block = crate::intrinsics::asm(name).expect("parses");
    assert_eq!((block.code, block.inputs, block.outputs, block.clobbers, block.memory), (vec![0xcd, 0x1a], vec!["ax".to_owned()], vec!["cx".to_owned(), "dx".to_owned()], vec!["flags".to_owned()], false));
    assert_eq!(crate::intrinsics::asm_name(&crate::intrinsics::asm(name).unwrap()), name);
    let problems = |text: &str| crate::verify::verify(&parse::module(text).unwrap_or_else(|error| panic!("{error}")));
    let right = format!("declare {{i16, i16}} @{name}(i16)\n");
    assert_eq!(problems(&right), Vec::<String>::new());
    assert_eq!(print::module(&parse::module(&right).unwrap()), format!("declare {{ i16, i16 }} @{name}(i16) nounwind memory(inaccessiblemem: readwrite)\n"));
    assert!(problems(&format!("declare i16 @{name}(i16)\n"))[0].contains("incorrect return type"));
    assert!(problems(&format!("declare {{i16, i16}} @{name}(i16, i16)\n"))[0].contains("incorrect argument type"));
}

/// BCC stores 3.125L as 00 00 00 00 00 00 00 C8 00 40; a long double
/// global llrm lays down must hold the same ten bytes.
#[test]
fn test_a_double_is_laid_down_in_x87_extended_form() {
    let extended = |value: f64| crate::types::x87_extended(value.to_bits());
    assert_eq!(extended(3.125), [0, 0, 0, 0, 0, 0, 0, 0xC8, 0x00, 0x40]);
    assert_eq!(extended(-2.25), [0, 0, 0, 0, 0, 0, 0, 0x90, 0x00, 0xC0]);
    assert_eq!(extended(0.0), [0; 10]);
    assert_eq!(extended(f64::from_bits(1)), [0, 0, 0, 0, 0, 0, 0, 0x80, 0xCD, 0x3B]);
}

/// What a call does to memory the program can name, less what it does to
/// memory it cannot: a routine that ends the program by writing its own
/// state touches nothing the loop around the call can see.
#[test]
fn a_call_to_inaccessible_memory_touches_nothing_nameable() {
    let module = crate::parse::module(
        "declare void @stop() noreturn memory(inaccessiblemem: readwrite)
declare void @any()

define void @f() {
b0:
  call void @stop()
  call void @any()
  ret void
}
",
    )
    .expect("a module");
    let callees = crate::memory::callees(&module);
    let (_, _, function) = module.functions().find(|(_, global, _)| global.name.as_deref() == Some("f")).expect("@f");
    let calls = function.walk().map(|(_, inst)| inst).filter(|&inst| matches!(function.instruction(inst).opcode, crate::opcode::Opcode::Call(_))).collect::<Vec<_>>();
    let stop = crate::memory::accessible(&module.context, &callees, function, calls[0]);
    let any = crate::memory::accessible(&module.context, &callees, function, calls[1]);
    assert_eq!((stop, any), (crate::memory::Effects::NONE, crate::memory::Effects::ANY));
    assert!(crate::memory::of(&module.context, &callees, function, calls[0]).writes, "it still writes what it can reach");
}

fn dominance_problems(text: &str) -> Vec<String> {
    crate::verify::verify(&parse::module(text).unwrap_or_else(|error| panic!("{error}"))).into_iter().filter(|one| one.contains("dominate")).collect()
}

/// A value used in a branch arm that does not contain its definition, in
/// the join after it, or before it in the same block, is not dominated; the
/// verifier that runs after every pass must say so.
#[test]
fn a_use_not_dominated_by_its_definition_is_reported() {
    let diamond = |join: &str| {
        format!(
            "define i16 @f(i1 %c, i16 %x) {{
b0:
  br i1 %c, label %l, label %r
l:
  %a = add i16 %x, 1
  br label %j
r:
  br label %j
j:
{join}
}}
"
        )
    };
    assert_eq!(dominance_problems(&diamond("  ret i16 %x")), Vec::<String>::new());
    assert_eq!(dominance_problems(&diamond("  ret i16 %a")).len(), 1, "a definition in one arm does not reach the join");
    assert_eq!(dominance_problems(&diamond("  %p = phi i16 [ %a, %l ], [ %x, %r ]\n  ret i16 %p")), Vec::<String>::new());
    assert_eq!(dominance_problems(&diamond("  %p = phi i16 [ %x, %l ], [ %a, %r ]\n  ret i16 %p")).len(), 1, "a phi input must dominate its own edge");
}

/// Within a block a use before the definition, and a use of the value
/// itself, are not dominated.
#[test]
fn a_use_before_its_definition_in_one_block_is_reported() {
    let before = "define i16 @f(i16 %x) {
b0:
  %b = add i16 %a, 1
  %a = add i16 %x, 1
  ret i16 %b
}
";
    assert_eq!(dominance_problems(before).len(), 1);
}

/// A module its frontend made with a use its definition does not dominate
/// was reported as the first pass's doing ("after mem2reg: ..."), sending
/// the search to the wrong crate; it is reported before any pass runs.
#[test]
fn a_module_made_wrong_is_reported_before_the_first_pass() {
    let mut module = parse::module(
        "define i16 @f(i16 %x) {
b0:
  %b = add i16 %a, 1
  %a = add i16 %x, 1
  ret i16 %b
}
",
    )
    .expect("parses");
    struct Idle;
    impl crate::passes::FunctionPass for Idle {
        fn name(&self) -> &'static str {
            "idle"
        }

        fn run(&mut self, _: &mut crate::passes::Unit, _: &mut crate::passes::Analyses) -> crate::passes::PreservedAnalyses {
            crate::passes::PreservedAnalyses::all()
        }
    }
    let mut manager = crate::passes::PassManager { verify_each: true, ..Default::default() };
    manager.add(Idle);
    let error = manager.run_module(&mut module, std::rc::Rc::new(crate::target::Neutral)).unwrap_err();
    assert!(error.starts_with("before the first pass:") && error.contains("dominate"), "{error}");
}

/// The recurrences a loop carries, for the loop corpus's `mir-ivs`: a far
/// pointer walked beside a counter is two, though only the counter has a
/// `Recurrence`. `wordlen` in BASIC counted one before the pass walked its
/// far pointer as an integer offset and two after, for the same loop.
#[test]
fn a_walked_pointer_is_a_recurrence_the_loop_carries() {
    let module = crate::parse::module(
        "define i16 @f(ptr addrspace(1) %far, i16 %n) {
entry:
  br label %l

l:
  %i = phi i16 [ 0, %entry ], [ %i.next, %l ]
  %p = phi ptr addrspace(1) [ %far, %entry ], [ %p.next, %l ]
  %v = load i16, ptr addrspace(1) %p
  %p.next = getelementptr i8, ptr addrspace(1) %p, i16 2
  %i.next = add i16 %i, 1
  %c = icmp slt i16 %i.next, %n
  br i1 %c, label %l, label %d

d:
  ret i16 %i.next
}

define i16 @g(ptr addrspace(1) %far, i16 %n) {
entry:
  br label %l

l:
  %i = phi i16 [ 0, %entry ], [ %i.next, %l ]
  %p = phi ptr addrspace(1) [ %far, %entry ], [ %p, %l ]
  %v = load i16, ptr addrspace(1) %p
  %i.next = add i16 %i, 1
  %c = icmp slt i16 %i.next, %n
  br i1 %c, label %l, label %d

d:
  ret i16 %i.next
}
",
    )
    .expect("a module");
    let counted = |name: &str| {
        let (_, _, function) = module.functions().find(|(_, global, _)| global.name.as_deref() == Some(name)).expect("a function");
        let tree = crate::dominators::DominatorTree::new(function);
        let loops = crate::loops::LoopInfo::new(function, &tree);
        let evolution = crate::scalarevolution::Evolution::new(&module.context, function, &loops);
        evolution.counted(&module.context, function, &loops, loops.loops[0].header)
    };
    assert_eq!((counted("f"), counted("g")), (2, 1), "a counter and a walked pointer, then a counter and a pointer that stays");
}

/// `releases` read and written back, stated of a parameter: a copy of a routine that frees its
/// argument has to know it.
#[test]
fn releases_is_a_parameter_attribute_that_round_trips() {
    let text = format!("{DATALAYOUT}\ndeclare void @free(ptr releases)\n");
    let once = crate::print::module(&crate::parse::module(&text).unwrap());
    assert!(once.contains("declare void @free(ptr releases)"), "{once}");
}

/// `noretain` read and written back, stated of a parameter and of a call's
/// argument; before it existed the parser refused the attribute.
#[test]
fn noretain_is_a_parameter_attribute_that_round_trips() {
    let text = format!("{DATALAYOUT}\ndeclare void @erase(ptr nocapture noretain)\n\ndefine void @f(ptr %p) {{\nb0:\n  call void @erase(ptr noretain %p)\n  ret void\n}}\n");
    let once = round(&text);
    assert!(once.contains("declare void @erase(ptr nocapture noretain)") && once.contains("call void @erase(ptr noretain %p)"), "{once}");
    assert_eq!(round(&once), once);
}

/// Each use asked `instruction_dominates` of its definition, which scanned the block for both: the
/// verifier was 12% of compiling 800 BASIC statements (#560). The positions are made once.
#[test]
fn test_verifying_a_block_does_not_scan_it_for_each_use() {
    let chain: String = (1..300).map(|at| format!("  %v{at} = add i16 %v{}, 1\n", at - 1)).collect();
    let text = format!("{DATALAYOUT}define i16 @f(i16 %v0) {{\nentry:\n{chain}  ret i16 %v299\n}}\n");
    let module = parse::module(&text).unwrap_or_else(|error| panic!("{error}"));
    let before = crate::dominators::scans();
    assert_eq!(crate::verify::verify(&module), Vec::<String>::new());
    assert_eq!(crate::dominators::scans() - before, 0, "a use scanned its block");
}

/// A use before its definition in one block is still refused, by position.
#[test]
fn test_a_use_before_its_definition_in_one_block_is_refused() {
    let text = format!("{DATALAYOUT}define i16 @f(i16 %a) {{\nentry:\n  %y = add i16 %x, 1\n  %x = add i16 %a, 1\n  ret i16 %y\n}}\n");
    let problems = crate::verify::verify(&parse::module(&text).unwrap_or_else(|error| panic!("{error}")));
    assert!(problems.iter().any(|one| one.contains("does not dominate")), "{problems:?}");
}
