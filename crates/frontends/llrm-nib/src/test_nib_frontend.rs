//! Port of `tests/test_nib_frontend.py`.
//!
//! skipped: `execute.run` assertions (`qbopt/hir/execute.py` is tools-only),
//! and the tests made of nothing else; test_dos_bootstrap_enters_the_runtime_before_language_main
//! (reads runtime sources, no compiler).

use llrm_core::abi::nib as rt;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use regex::Regex;

use super::compile as nib_compile;
use super::driver;
use llrm_core::backend::masm;
use llrm_core::hir::{self, model};
use llrm_core::driver::Options;
use llrm_core::support::pyjson::{self, Json};

pub(crate) fn root() -> PathBuf {
    PathBuf::from(env!("LLRM_ROOT"))
}

pub(crate) fn fixture(name: &str) -> PathBuf {
    root().join("tests/fixtures/nib").join(name)
}

/// `driver.parsed(source)`.
pub(crate) fn parsed(source: &Path) -> model::Program {
    driver::parsed(source, &crate::real_mode(), None).unwrap_or_else(|error| panic!("{}: {error}", source.display()))
}

/// The `FrontendError` `driver.parsed(source)` raises.
fn refused(source: &Path) -> String {
    driver::parsed(source, &crate::real_mode(), None).expect_err("the frontend refuses").0
}

/// A tag is within the tags its enum has, stated once of the tag's member, not of each load:
/// every load of it, as many as the program makes, carries `!range` in the emitted MIR
/// with no instruction fact from the frontend. A load a later change forgets to tag no
/// longer loses the fact.
#[test]
fn every_load_of_an_enums_tag_has_its_range_from_one_statement() {
    use llrm_core::hir::facts::Subject;
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(&directory, "shape.nib", "enum Shape:\n    circle(radius: i16)\n    square(side: i16)\n    tri(base: i16, height: i16)\n\nfn area(shape: &Shape) -> i16:\n    match shape:\n        .circle(r):\n            return r * r * 3\n        .square(s):\n            return s * s\n        .tri(b, h):\n            return b * h // 2\n\nfn sides(shape: &Shape) -> i16:\n    match shape:\n        .circle(_):\n            return 0\n        .square(_):\n            return 4\n        .tri(_, _):\n            return 3\n\nfn main() -> i16:\n    let a = Shape.square(side=4)\n    return area(a) + sides(a)\n");
    let program = parsed(&source);
    let module = &program.modules[0];
    let fields: Vec<_> = module.facts.iter().filter(|one| matches!(one.subject, Subject::Field { .. })).collect();
    assert_eq!(fields.len(), 1, "one statement for the one enum");
    assert!(module.facts.iter().all(|one| !(matches!(one.subject, Subject::Instruction { .. }) && matches!(one.fact, llrm_mir::facts::Fact::Range(bounds) if bounds.hi == 2))), "no load is stated of its own");
    let emitted = hir::mir::emit(&program, &llrm_x86_m16::layout());
    let text: String = emitted.iter().map(|one| llrm_mir::print::module(&one.module)).collect();
    let tag_loads: Vec<&str> = text.lines().filter(|one| one.contains("load i8")).collect();
    assert!(tag_loads.len() >= 2, "{text}");
    assert!(tag_loads.iter().all(|one| one.contains("!range")), "{text}");
}

/// A view lies within one segment, so a dimension of 2-byte elements is at most 32767: stated of its
/// load, or `lo <u len` leaves a negative `lo` possible and no pass can hoist an index check out
/// of a loop (quicksort's `sort` tested `j <u len` on every trip). A byte view says nothing: 65535 is its type's whole range.
#[test]
fn a_views_dimension_load_states_what_its_segment_holds() {
    let directory = tempfile::tempdir().expect("a directory");
    let program = |element: &str| {
        let source = written(&directory, &format!("view_{element}.nib"), &format!("fn at(a: &[{element}], i: i16) -> {element}:\n    return a[i]\n\nfn main() -> i16:\n    let a: {element}[4] = [1] * 4\n    return i16(at(a, 2))\n"));
        let text: String = hir::mir::emit(&parsed(&source), &llrm_x86_m16::layout()).iter().map(|one| llrm_mir::print::module(&one.module)).collect();
        text.lines().filter(|one| one.contains("load i16") && one.contains("!range")).count()
    };
    assert_eq!(program("i16"), 1, "the length of a view of i16 is stated at most 32767");
    assert_eq!(program("u8"), 0, "a byte view's length may be any u16");
}

/// A fact stated once of any member, not only a tag, reaches every load and store of it,
/// through a reference and through a local: each field access names its member.
#[test]
fn a_fact_of_a_member_reaches_every_load_and_store_of_it() {
    use llrm_core::hir::facts::Subject;
    use llrm_mir::facts::{Bounds, Fact};
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(&directory, "pair.nib", "struct P:\n    mut a: i16\n    mut b: i16\n\nfn bump(p: &mut P) -> void:\n    p.b = p.b + 1\n\nfn main() -> i16:\n    let mut q = P(a=1, b=2)\n    q.b = q.b + 5\n    bump(q)\n    return q.b + q.a\n");
    let mut program = parsed(&source);
    let owner = program.modules[0].types.iter().find(|one| one.name == "P").expect("the struct P").id;
    // The accesses of member b, as the frontend wrote them: loads and stores, by reference and by place.
    let member_of = |operand: &model::Operand| match operand {
        model::Operand::ProjectedPlace(one) => one.member,
        model::Operand::IndirectPlace(one) => one.member,
        _ => None,
    };
    let (mut loads, mut stores, mut reference, mut local) = (0, 0, 0, 0);
    for function in &program.modules[0].functions {
        for instruction in function.blocks.iter().flat_map(|block| &block.instructions) {
            let Some(member) = instruction.operands.first().and_then(member_of).filter(|one| one.owner == owner && one.offset == 2) else { continue };
            let _ = member;
            match instruction.op {
                model::Op::Store => stores += 1,
                _ => loads += 1,
            }
            match instruction.operands[0] {
                model::Operand::IndirectPlace(_) => reference += 1,
                _ => local += 1,
            }
        }
    }
    assert!(loads >= 3 && stores >= 2 && reference >= 2 && local >= 3, "loads {loads} stores {stores} reference {reference} local {local}");
    program.modules[0].facts.push(llrm_core::hir::facts::Stated { subject: Subject::Field { owner, offset: 2 }, fact: Fact::Range(Bounds { lo: 0, hi: 100 }), source: None });
    program.modules[0].facts.push(llrm_core::hir::facts::Stated { subject: Subject::Field { owner, offset: 2 }, fact: Fact::Align(2), source: None });
    let text: String = hir::mir::emit(&program, &llrm_x86_m16::layout()).iter().map(|one| llrm_mir::print::module(&one.module)).collect();
    let ranged = text.lines().filter(|one| one.contains("load i16") && one.contains("!range")).count();
    let aligned_loads = text.lines().filter(|one| one.contains("load i16") && one.contains("align 2")).count();
    let aligned_stores = text.lines().filter(|one| one.contains("store i16") && one.contains("align 2")).count();
    assert_eq!((ranged, aligned_loads, aligned_stores), (loads, loads, stores), "{text}");
}

/// Every Nib program's emitted MIR lints clean: `lint::poison` called each stated
/// wrap `poison` and each array filled an element at a time "stored after use",
/// so `hir-mir` and the corpus tool dropped 56 of 124 programs, `sum_three` among them.
#[test]
fn the_mir_of_sum_three_lints_clean() {
    let program = parsed(&fixture("sum_three.nib"));
    for emitted in hir::mir::emit(&program, &llrm_x86_m16::layout()) {
        assert_eq!(emitted.refused, Vec::<(String, String)>::new());
        assert_eq!(llrm_mir::lint::poison(&emitted.module), Vec::<String>::new());
    }
}

/// An enum value is built whole: every byte of it is written, the payload of a
/// variant without one zero, so that a value flowing as one integer has no
/// undefined bytes (#290); `lint::poison` finds a load that reads them.
fn lint_of(name: &str) -> Vec<String> {
    let program = parsed(&PathBuf::from(env!("LLRM_ROOT")).join(name));
    hir::mir::emit(&program, &llrm_x86_m16::layout()).iter().flat_map(|emitted| llrm_mir::lint::poison(&emitted.module)).collect()
}

#[test]
fn the_enum_values_of_digits_write_all_their_bytes() {
    assert_eq!(lint_of("examples/digits.nib"), Vec::<String>::new());
}

/// `tmp_path / name` holding `text`.
fn written(directory: &tempfile::TempDir, name: &str, text: &str) -> PathBuf {
    let path = directory.path().join(name);
    std::fs::write(&path, text).expect("writes the source");
    path
}

/// `masm.text(nib_compile.assembled(program, entry=entry, options=options))`.
pub(crate) fn listing(program: &model::Program, entry: &str, options: &Options) -> String {
    listing_on(program, entry, options, "386")
}

/// `listing` with `cpu=cpu`.
fn listing_on(program: &model::Program, entry: &str, options: &Options, cpu: &'static str) -> String {
    let options = Options { machine: llrm_core::abi::machine::Machine { cpu: cpu.to_owned(), ..options.machine.clone() }, ..options.clone() };
    let module = nib_compile::assembled(program, entry, &options, &crate::real_mode().os).expect("assembles");
    masm::text(&module).expect("prints")
}

/// `text.split(start, 1)[1].split(end, 1)[0]`.
fn between<'t>(text: &'t str, start: &str, end: &str) -> &'t str {
    let after = text.split_once(start).unwrap_or_else(|| panic!("{start:?} not in listing")).1;
    after.split_once(end).map_or(after, |(inside, _)| inside)
}

/// The driver's options at `-{name}` on Nib's machine.
pub(crate) fn level(name: &str) -> Options {
    let mut flags = llrm_core::driver::flags::Flags::default();
    flags.take(&[format!("-{name}")], &mut 0).expect("a level");
    { let bound = llrm_driver::target(&flags, Some(&["x86-m16"])).unwrap(); bound.options(&flags, nib_compile::machine(&llrm_x86_m16::M16, &crate::real_mode().os)) }
}

/// -Os with no inlining: the function under test stays a function, as it does where more than
/// one call reaches it (tuned for size the last call of a private function is inlined).
fn os_calls_kept() -> Options {
    let mut options = level("Os");
    options.pipeline.inline = llrm_transforms::inline::Threshold::new(0);
    options
}

/// -O2 with neither unrolling nor peeling.
fn unrolled_or_peeled_none() -> Options {
    let mut options = level("O2");
    options.pipeline.unroll = false;
    options.pipeline.peel = false;
    options
}

/// `program`'s module MIR after the pipeline, as text.
fn optimized_mir(program: &model::Program, options: &Options) -> String {
    let (mut mir, _) = llrm_core::driver::emitted(program, options).expect("emits");
    llrm_core::driver::optimized(&mut mir, options).expect("optimizes");
    llrm_mir::print::module(&mir.modules[0])
}

#[allow(non_snake_case)]
pub(crate) fn O2() -> Options {
    level("O2")
}

/// `program` compiled to an object.
fn object_of(program: &model::Program, entry: &str, source: &Path, options: &Options, layout: llrm_core::backend::objbuild::CodeLayout) -> Result<Vec<u8>, String> {
    nib_compile::object(&nib_compile::assembled(program, entry, options, &crate::real_mode().os)?, source, layout)
}

fn types(program: &model::Program) -> std::collections::BTreeMap<&str, &model::Type> {
    program.modules[0].types.iter().map(|one| (one.name.as_str(), one)).collect()
}

fn function<'p>(program: &'p model::Program, name: &str) -> &'p model::Function {
    program.modules[0].functions.iter().find(|one| one.name == name).expect("the function exists")
}

/// `program`'s MIR as its HIR emits it, before any pass.
fn emitted_text(program: &model::Program) -> String {
    hir::mir::emit(program, &llrm_x86_m16::layout()).iter().map(|one| llrm_mir::print::module(&one.module)).collect()
}

/// `@name`'s definition in `text`.
fn defined<'t>(text: &'t str, name: &str) -> &'t str {
    let start = text.find(&format!("@{name}(")).and_then(|at| text[..at].rfind("define ")).unwrap_or_else(|| panic!("no @{name}\n{text}"));
    &text[start..start + text[start..].find("\n}\n").expect("its end")]
}

fn instructions(function: &model::Function) -> impl Iterator<Item = &model::Instruction> {
    function.blocks.iter().flat_map(|block| &block.instructions)
}

fn program() -> model::Program {
    parsed(&fixture("control.nib"))
}

#[test]
fn test_frontend_document_crosses_the_strict_common_hir_boundary() {
    let program = program();
    assert_eq!(program.dialect, model::Dialect::Nib);
    assert_eq!(program.runtime, model::RuntimeProfile::Freestanding);
    let names: Vec<&str> = program.modules[0].functions.iter().map(|one| one.name.as_str()).collect();
    assert_eq!(names, ["step", "count"]);
    assert_eq!(hir::decode(&hir::encode(&program, None).expect("encodes")).expect("decodes"), program);
}

#[test]
fn test_frontend_lowers_control_flow_and_calls_to_existing_mir() {
    let text = emitted_text(&program());
    let count = defined(&text, "count");
    for one in ["call addrspace(1) i16 @step(", "br i1 ", " = load ", "store "] {
        assert!(count.contains(one), "{one}\n{count}");
    }
    assert!(defined(&text, "step").contains(" = add "), "{text}");
}

#[test]
fn test_frontend_json_is_deterministic_and_replayable() {
    let directory = tempfile::tempdir().expect("a directory");
    let first = directory.path().join("first.json");
    let second = directory.path().join("second.json");
    driver::parsed(&fixture("control.nib"), &crate::real_mode(), Some(&first)).expect("parses");
    driver::parsed(&fixture("control.nib"), &crate::real_mode(), Some(&second)).expect("parses");
    let first = std::fs::read(first).expect("dumped");
    assert_eq!(first, std::fs::read(second).expect("dumped"));
    let Json::Dict(document) = pyjson::loads(&String::from_utf8(first).expect("utf-8")).expect("JSON") else {
        panic!("not an object");
    };
    assert_eq!(document.get("schema"), Some(&Json::Int(5)));
}

#[test]
fn test_type_error_is_reported_above_hir() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "wrong.nib",
        "fn wrong(value: i16) -> i16:\n    if value:\n        return 1\n    return 0\n",
    );
    assert!(refused(&source).contains("expected bool"));
}

#[test]
fn test_all_primitive_types_cross_hir_with_their_exact_representation() {
    let program = parsed(&fixture("primitives.nib"));
    let types = types(&program);
    let names: BTreeSet<&str> = types.keys().copied().collect();
    assert_eq!(
        names,
        // The runtime's routines take a `&string` as the far pointer to its descriptor.
        BTreeSet::from(["void", "bool", "char", "i8", "u8", "i16", "u16", "i32", "u32", "f32", "f64", "string", "addr", "$slice[char]", "&$slice[char]"])
    );
    let integral: Vec<(i64, Option<bool>)> =
        ["char", "i8", "u8", "i16", "u16", "i32", "u32"].iter().map(|name| (types[name].width, types[name].signed)).collect();
    assert_eq!(
        integral,
        [(1, Some(false)), (1, Some(true)), (1, Some(false)), (2, Some(true)), (2, Some(false)), (4, Some(true)), (4, Some(false))]
    );
    assert_eq!((types["bool"].width, types["void"].width), (1, 0));
    // The x87 evaluates in extended precision and rounds on store, as DOS C does.
    assert_eq!(types["f32"].evaluation, model::FloatEvaluation::Extended80);
    assert_eq!(types["f64"].evaluation, model::FloatEvaluation::Extended80);
    let sizes: Vec<usize> = program.modules[0].data.iter().map(|one| one.bytes.len()).collect();
    assert_eq!(sizes, [4, 8]);
}

#[test]
fn test_unsigned_and_floating_operations_keep_their_semantics_in_mir() {
    let text = emitted_text(&parsed(&fixture("primitives.nib")));

    assert!(defined(&text, "unsigned_divide").contains(" = udiv "), "{text}");
    assert!(!defined(&text, "unsigned_divide").contains(" = sdiv "), "{text}");
    assert!(defined(&text, "unsigned_remainder").contains(" = urem "), "{text}");
    assert!(defined(&text, "float_product").contains(" = fmul "), "{text}");
    assert!(defined(&text, "unsigned_less").contains(" = icmp ult "), "{text}");
}

#[test]
fn test_fixed_point_types_scale_literals_and_keep_storage_width_in_mir() {
    let program = parsed(&fixture("fixed.nib"));
    let types = types(&program);
    assert_eq!((types["fixed8"].width, types["fixed8"].signed), (2, Some(true)));
    assert_eq!((types["fixed16"].width, types["fixed16"].signed), (4, Some(true)));
    assert_eq!((types["$i64"].width, types["$i64"].signed), (8, Some(true)));

    let fixed_literals = function(&program, "fixed_literals");
    let constants: Vec<&model::Operand> = instructions(fixed_literals)
        .flat_map(|instruction| &instruction.operands)
        .filter(|operand| matches!(operand, model::Operand::Constant(_)))
        .collect();
    assert!(constants.contains(&&model::Operand::constant(types["fixed16"].id, 98_304)));
    assert!(constants.contains(&&model::Operand::constant(types["fixed16"].id, 147_456)));

    let decimal_prints: Vec<&model::Instruction> =
        instructions(fixed_literals).filter(|one| one.callee.as_deref() == Some(rt::PRINT_Q4)).collect();
    assert_eq!(decimal_prints.len(), 2);
    for decimal_print in decimal_prints {
        let [raw, fraction] = decimal_print.operands.as_slice() else { panic!("two operands") };
        let model::Operand::ValueRef(raw) = raw else { panic!("a value") };
        let value_type = fixed_literals.values.iter().find(|one| one.id == raw.value).unwrap().r#type;
        assert_eq!(value_type, types["i32"].id);
        assert_eq!(fraction, &model::Operand::constant(types["u8"].id, 16));
    }

    let text = emitted_text(&program);
    let (product, quotient) = (defined(&text, "product"), defined(&text, "quotient"));
    // fixed8 uses a 32-bit intermediate; fixed16 is already based on i32 and
    // stays one fixed-point operation, never widened to i64.
    assert!(["sext i16", "mul i32", "ashr i32"].iter().all(|one| product.contains(one)), "{product}");
    assert!(quotient.contains("@llvm.sdiv.fix.i32("), "{quotient}");
    assert!(!["sext", "shl", " sdiv ", "i64"].iter().any(|one| quotient.contains(one)), "{quotient}");
}

/// nbody's `nbody` as its HIR emits it.
fn nbody_emitted() -> String {
    let text = emitted_text(&parsed(&fixture("nbody.nib")));
    defined(&text, "nbody").to_owned()
}

#[test]
fn test_fixed_i32_product_stays_a_storage_width_operation() {
    // Native nbody used to route every Q23.9 product through generic i64 MIR.
    let nbody = nbody_emitted();
    assert!(nbody.contains("@llvm.smul.fix.i32(") && nbody.contains("@llvm.sdiv.fix.i32("), "{nbody}");
    assert!(!nbody.contains("i64"), "{nbody}");
}

#[test]
fn test_fixed_i32_arithmetic_never_enters_generic_int64_legalization() {
    // nbody's Q23.9 inner loop expanded one division to 311 inline bytes.
    let assembly = listing(&parsed(&fixture("nbody.nib")), "main", &O2());
    let calls: BTreeSet<&str> = assembly.lines().filter_map(|line| line.trim().strip_prefix("call far ptr ")).collect();
    assert!(calls.iter().any(|callee| callee.starts_with("N$")), "premise: nbody prints through the runtime\n{assembly}");
    assert!(calls.iter().all(|callee| callee.starts_with("N$") || *callee == "_nbody"), "{calls:?}");
}

#[test]
fn test_nbody_arrays_strings_and_print_cross_hir_and_verify_in_mir() {
    let program = parsed(&fixture("nbody.nib"));
    let module = &program.modules[0];
    let types = types(&program);
    let scalar = types["scalar"];
    assert_eq!((scalar.kind, scalar.width, scalar.signed), (model::TypeKind::Integer, 4, Some(true)));
    let vec2i = types["vec2i"];
    assert_eq!((vec2i.kind, vec2i.width), (model::TypeKind::Opaque, 8));
    let body = types["body"];
    assert_eq!((body.kind, body.width), (model::TypeKind::Opaque, 16));
    let array = types["[body; 6]"];
    assert_eq!((array.element, array.rank, array.bounds.clone(), array.width), (Some(body.id), 1, vec![(0, 5)], 96));

    let string = types["string"];
    assert_eq!((string.element, string.width, string.address), (Some(types["char"].id), 2, model::AddressKind::Near));
    for literal in &module.data {
        // Static and read-only (section 13), then length and capacity.
        assert_eq!(&literal.bytes[..2], [0x08, 0]);
        let length = literal.bytes[2] | literal.bytes[3] << 8;
        let capacity = literal.bytes[4] | literal.bytes[5] << 8;
        assert_eq!(length, capacity);
        assert_eq!(length, literal.bytes.len() as i64 - 7);
        assert_eq!(*literal.bytes.last().unwrap(), 0);
    }

    let callables: std::collections::BTreeMap<&str, &model::Callable> =
        module.callables.iter().map(|one| (one.name.as_str(), one)).collect();
    assert!(!callables[rt::PRINT_STRING].defined);
    assert!(!callables[rt::PRINT_Q4].defined);
    assert_eq!(callables[rt::PRINT_Q4].parameter_types, [types["i32"].id, types["u8"].id]);
    assert!(!callables[rt::PRINT_NEWLINE].defined);
    assert!(module.functions[0].calls.iter().all(|call| call.distance == model::CallDistance::Far));
    let fixed_id = callables[rt::PRINT_Q4].id;
    assert!(module.functions[0].calls.iter().filter(|call| call.callee == Some(fixed_id)).all(|call| call.order == [1, 0]));

    let nbody = nbody_emitted();
    for one in ["getelementptr", "br i1 ", "call addrspace(1) void @N$", "@llvm.sdiv.fix.i32(", "@llvm.smul.fix.i32(", " = load ", "store "] {
        assert!(nbody.contains(one), "{one}\n{nbody}");
    }

    let operands: Vec<&model::Operand> = instructions(&module.functions[0]).flat_map(|one| &one.operands).collect();
    let projections: Vec<&model::ProjectedPlace> = operands
        .iter()
        .filter_map(|operand| match operand {
            model::Operand::ProjectedPlace(one) => Some(one),
            _ => None,
        })
        .collect();
    assert!(!projections.is_empty());
    assert_eq!(projections.iter().map(|one| one.offset).collect::<BTreeSet<_>>(), BTreeSet::from([0, 4, 8, 12]));
    assert!(instructions(&module.functions[0]).any(|one| one.op == model::Op::Ne));
    let range_counter = module.functions[0].places.iter().find(|place| place.name == "$range_step_no").unwrap();
    assert_eq!(range_counter.r#type, types["i32"].id);
}

#[test]
fn test_nbody_string_places_point_after_the_descriptor() {
    let program = parsed(&fixture("nbody.nib"));
    let strings: Vec<&model::Place> =
        program.modules[0].functions[0].places.iter().filter(|place| place.name.starts_with("$string")).collect();
    assert!(!strings.is_empty());
    assert!(strings.iter().all(|place| place.offset == 6));
}

#[test]
fn test_nbody_native_loops_eliminate_redundant_index_arithmetic() {
    // Nib nbody emitted 52 `sub index,0; shl index,4` address chains.
    let assembly = listing(&parsed(&fixture("nbody.nib")), "main", &O2());

    assert!(!assembly.contains("sub si, 0"));
    assert!(!assembly.contains("sub di, 0"));
    let scaled_indices = assembly.matches("shl si, 4").count() + assembly.matches("shl di, 4").count();
    assert!(scaled_indices <= 2);
}

#[test]
fn test_nbody_position_loop_uses_one_end_relative_byte_offset() {
    // -O2 unrolls the loop away.
    let assembly = listing(&parsed(&fixture("nbody.nib")), "main", &os_calls_kept());
    let function = between(&assembly, "_nbody proc near", "_nbody endp");
    // The innermost loop closing on `jne` that adds each velocity to its position.
    let loop_ = Regex::new(r"(?m)^(L\w+):\n")
        .unwrap()
        .captures_iter(function)
        .filter_map(|found| {
            let start = found.get(0).unwrap().start();
            let end = function[start..].find(&format!("    jne {}\n", &found[1]))?;
            Some(&function[start..start + end])
        })
        .filter(|one| one.matches("    add dword ptr [").count() == 2)
        .min_by_key(|one| one.len())
        .unwrap_or_else(|| panic!("no position loop:\n{function}"));

    assert!(!loop_.contains("shl "), "{loop_}");
    assert!(!loop_.contains("lea "), "{loop_}");
    assert!(!loop_.contains("cmp "), "{loop_}");
    assert!(Regex::new(r"    add e?(?:ax|bx|cx|dx|si|di), \d+\n$").unwrap().is_match(loop_), "{loop_}");
}

#[test]
fn test_nbody_velocity_fields_are_stored_once_per_update() {
    let assembly = listing(&parsed(&fixture("nbody.nib")), "main", &O2());
    let function = between(&assembly, "_nbody proc near", "_nbody endp");
    // The only stores through a body's index are its velocity's two fields.
    let stored: Vec<String> = Regex::new(r"mov dword ptr (\[bp\+[sd]i[-+]\d+\]), e(?:ax|bx|cx|dx|si|di)\n")
        .unwrap()
        .captures_iter(function)
        .map(|found| found[1].to_owned())
        .collect();

    assert_eq!(stored.len(), 2, "{function}");
    for field in &stored {
        let loads = Regex::new(&format!(r"e(?:ax|bx|cx|dx|si|di), dword ptr {}\n", regex::escape(field))).unwrap();
        assert_eq!(loads.find_iter(function).count(), 1, "{field}\n{function}");
    }
}

const STRIDE: &str = "\
struct sample:
    tag: i16
    mut value: i32
    delta: i32

fn update(v: &[i32]) -> i32:
    let mut samples: sample[5] = [
        sample(tag=0, value=v[0], delta=v[1]),
        sample(tag=0, value=v[1], delta=v[2]),
        sample(tag=0, value=v[2], delta=v[3]),
        sample(tag=0, value=v[3], delta=v[4]),
        sample(tag=0, value=v[4], delta=v[5]),
    ]
    for current in &mut samples:
        current.value += current.delta
    return samples[0].value + samples[4].value

fn main() -> i16:
    let v: i32[6] = [1, 2, 3, 4, 5, 6]
    update(&v)
    return 0
";

#[test]
fn test_counted_struct_loop_uses_its_record_width_as_the_byte_stride() {
    // The end-relative recurrence is an affine-loop rule, not a body/16 rule.
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(&directory, "stride.nib", STRIDE);

    // -O2 unrolls the loop away.
    let assembly = listing_on(&parsed(&source), "main", &os_calls_kept(), "486");
    let update = between(&assembly, "_update proc near", "_update endp");

    // -5 * sizeof(sample), with sizeof(sample) == 10.
    let offset = Regex::new(r"    mov ([sd]i), -50\n").unwrap().captures(update).unwrap_or_else(|| panic!("{update}"))[1].to_owned();
    let body = closed_on_jne(update).unwrap_or_else(|| panic!("no loop closes on jne:\n{update}"));
    assert!(body.contains(&format!("add {offset}, 10\n")), "{body}");
    assert!(body.contains(&format!("dword ptr [bp+{offset}+2]")), "{body}");
    assert!(body.contains(&format!("dword ptr [bp+{offset}+6]")), "{body}");
    assert!(!update.contains(&format!("shl {offset}")), "{update}");
}

#[test]
fn test_os_copies_no_loop_into_larger_code() {
    // -O2 copies the five-record update loop; -Os must not grow the code doing so.
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "stride.nib",
        "\
struct sample:
    tag: i16
    mut value: i32
    delta: i32

fn total(samples: &[sample]) -> i32:
    let mut sum: i32 = 0
    for one in &samples:
        sum += one.value
    return sum

fn update(v: &[i32]) -> i32:
    let mut samples: sample[5] = [
        sample(tag=0, value=v[0], delta=v[1]),
        sample(tag=0, value=v[1], delta=v[2]),
        sample(tag=0, value=v[2], delta=v[3]),
        sample(tag=0, value=v[3], delta=v[4]),
        sample(tag=0, value=v[4], delta=v[5]),
    ]
    for current in &mut samples:
        current.value += current.delta
    return total(&samples)

fn main() -> i16:
    let v: i32[6] = [1, 2, 3, 4, 5, 6]
    update(&v)
    return 0
",
    );
    let bytes = |options: &Options| -> usize {
        object_of(&parsed(&source), "main", &source, options, llrm_core::backend::objbuild::CodeLayout::OneSegment).expect("writes").len()
    };
    let uncopied = unrolled_or_peeled_none();
    assert_ne!(listing(&parsed(&source), "main", &level("O2")), listing(&parsed(&source), "main", &uncopied), "premise: -O2 copies the loop");
    assert!(bytes(&level("Os")) <= bytes(&uncopied));
}

#[test]
fn test_fixed_array_storage_has_a_prefix_descriptor() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "array_descriptor.nib",
        "fn main() -> i16:\n    let mut values: i16[3] = [10, 20, 30]\n    print(values.len)\n    return values[0]\n",
    );

    let program = parsed(&source);
    let function = &program.modules[0].functions[0];
    let values = function.places.iter().find(|place| place.name == "values").unwrap();
    let descriptor = |name: &str| function.places.iter().find(|place| place.name == name).unwrap();
    assert_eq!(values.offset, -6);
    assert_eq!(values.extent, Some(6));
    assert_eq!(descriptor("$values.length").offset, values.offset - 4);
    assert_eq!(descriptor("$values.capacity").offset, values.offset - 2);

    // Nothing reads the words, so nothing stores them: kept volatile,
    // they held priced_unroll's `value` at 4717 instructions on the isel
    // path to the old path's 190.
    let assembly = listing(&parsed(&source), "main", &O2());
    assert!(!assembly.contains("mov word ptr [bp-10], 3"));
    assert!(!assembly.contains("mov word ptr [bp-8], 3"));
}

#[test]
fn test_borrowed_array_call_builds_one_view_from_the_direct_payload() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "array_borrow.nib",
        "fn bump(values: &mut [u16]) -> void:\n    values[1] += 3\nfn main() -> i16:\n    let mut values: u16[3] = [10, 20, 30]\n    bump(&mut values)\n    return 0\n",
    );

    let program = parsed(&source);
    let caller = function(&program, "main");
    let values = caller.places.iter().find(|place| place.name == "values").unwrap();
    let addresses: Vec<&model::Instruction> = instructions(caller).filter(|one| one.op == model::Op::Address).collect();
    let payload_address = addresses.iter().find(|one| one.operands == [model::Operand::place_ref(values.id)]).unwrap();
    let view = caller.places.iter().find(|place| place.name == "$slice_values").unwrap();
    let view_address = addresses.iter().find(|one| one.operands == [model::Operand::place_ref(view.id)]).unwrap();
    let call = instructions(caller)
        .find(|one| one.op == model::Op::Call && one.callee.as_deref() == Some("bump"))
        .unwrap();
    assert_eq!(call.operands, [model::Operand::value_ref(view_address.results[0])]);
    let pointer = caller.values.iter().find(|value| value.id == payload_address.results[0]).unwrap();
    let pointer_type = program.modules[0].types.iter().find(|one| one.id == pointer.r#type).unwrap();
    assert_eq!(pointer_type.width, 4);
    assert_eq!(pointer_type.address, model::AddressKind::Far);

    let assembly = listing_on(&program, "main", &O2(), "486");
    let bump = between(&assembly, "_bump proc near", "_bump endp");
    let main = between(&assembly, "_main proc far", "_main endp");
    // The payload's address is the view's pointer, the view's address the argument.
    let payload = Regex::new(r"    lea ([a-z]+), \[bp-\d+\]\n").unwrap().captures(main).unwrap_or_else(|| panic!("{main}"))[1].to_owned();
    assert!(Regex::new(&format!(r"    mov word ptr \[bp-\d+\], {payload}\n")).unwrap().is_match(main), "{main}");
    assert!(Regex::new(r"    mov [a-z]+, ss\n").unwrap().is_match(main));
    assert!(main.contains("call _bump"));
    // `bump` is internal and called directly: it pops its own view, a stack pointer of one word, `ret 2`.
    assert!(!main.contains("add sp, 2") && bump.contains("ret 2"), "{main}{bump}");
    assert!(bump.contains("es:["));
    assert!(!object_of(&program, "main", &source, &O2(), llrm_core::backend::objbuild::CodeLayout::OneSegment).expect("writes").is_empty());
}

#[test]
fn test_borrow_rules_reject_shared_mutation_and_aliasing_mutable_arguments() {
    let directory = tempfile::tempdir().expect("a directory");
    let shared = written(&directory, "shared.nib", "fn bad(values: &[u16]) -> void:\n    values[0] = 2\n");
    assert!(refused(&shared).contains("immutable"));

    let aliased = written(
        &directory,
        "aliased.nib",
        "fn use(left: &mut [u16], right: &[u16]) -> void:\n    left[0] += right[0]\nfn bad() -> void:\n    let mut values: u16[1] = [1]\n    use(&mut values, &values)\n",
    );
    assert!(refused(&aliased).contains("aliases a mutable argument"));
}

#[test]
fn test_readonly_array_borrow_keeps_payload_initialization_visible_to_callee() {
    // sum returned stack garbage after DSE erased every payload store before its read-only call.
    let assembly = listing_on(&parsed(&fixture("sum.nib")), "main", &O2(), "486");
    let main = between(&assembly, "_main proc far", "_main endp");

    assert!(main.contains("call _sum"), "premise: the call stays\n{main}");

    assert!((1..7).all(|value| main.contains(&format!(", {value}"))));
}

#[test]
fn test_array_parameter_is_one_unsized_view_pointer() {
    let program = parsed(&fixture("sum.nib"));
    let module = &program.modules[0];
    let function = function(&program, "sum");
    let by_id = |id: i64| module.types.iter().find(|one| one.id == id).unwrap();
    let pointer = by_id(function.values[0].r#type);
    let descriptor = by_id(pointer.element.unwrap());
    let element = by_id(descriptor.element.unwrap());
    let metadata = module.types.iter().find(|one| one.name == "u16").unwrap();
    let descriptor_loads: Vec<&model::Operand> = instructions(function)
        .filter(|one| one.op == model::Op::Load && matches!(one.operands[0], model::Operand::DescriptorPlace(_)))
        .map(|one| &one.operands[0])
        .collect();

    assert_eq!(function.parameters.len(), 1);
    assert_eq!(pointer.kind, model::TypeKind::Pointer);
    assert_eq!(pointer.rank, 1);
    assert_eq!((descriptor.kind, descriptor.width), (model::TypeKind::Opaque, 8));
    assert_eq!(element.name, "i16");
    assert_eq!(
        descriptor_loads,
        [&model::Operand::DescriptorPlace(model::DescriptorPlace {
            base: function.parameters[0],
            field: model::DescriptorField::Length,
            r#type: metadata.id,
        })]
    );
}

#[test]
fn test_runtime_bounded_array_loop_advances_its_payload_address() {
    // sum rebuilt `payload + index * 2` on every trip despite its invariant runtime bound.
    let assembly = listing_on(&parsed(&fixture("sum.nib")), "main", &O2(), "486");
    let function = between(&assembly, "_sum proc near", "_sum endp");
    let hot = closed_on_jne(function).unwrap_or_else(|| panic!("no loop closes on jne:\n{function}"));

    assert!(!Regex::new(r"\b(?:imul|shl|lea)\b").unwrap().is_match(&hot), "{hot}");
    assert!(function.contains("xor ax, ax"));
    assert!(!function.contains("dec "));
    assert!(Regex::new(r"\badd\s+(?:si|di|bx),\s*2\s*\n(?:L\w+:\n)?\s*jne\b").unwrap().is_match(&hot), "{hot}");
}

#[test]
fn test_three_array_initializer_keeps_the_fixed_frame_address_component() {
    // sum_three wrote locals through EAX+SI after a secondary-base rewrite lost BP.
    // The call kept: inlined, the sums fold to 1110 and no element is stored.
    let mut kept = O2();
    kept.pipeline.inline = llrm_transforms::inline::Threshold::new(0);
    let assembly = listing(&parsed(&fixture("sum_three.nib")), "main", &kept);
    let main = between(&assembly, "_main proc far", "call _sum_three");
    let stored: BTreeSet<i64> = Regex::new(r"mov word ptr \[bp-\d+\], (\d+)\n")
        .unwrap()
        .captures_iter(main)
        .map(|found| found[1].parse().unwrap())
        .collect();
    let elements = [1, 2, 3, 4, 10, 20, 30, 40, 100, 200, 300, 400];

    assert!(elements.iter().all(|one| stored.contains(one)), "{main}");
    assert!(!Regex::new(r"ptr \[e").unwrap().is_match(main), "{main}");
}

#[test]
fn test_scoped_array_range_is_one_descriptor_pointer_and_executes() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "slice.nib",
        "fn sum(values: &[i16]) -> i16:\n    let mut total: i16 = 0\n    for value in &values:\n        total += value\n    return total\nfn main() -> i16:\n    let values: i16[4] = [1, 2, 3, 4]\n    return sum(&values[1:3])\n",
    );

    let program = parsed(&source);
    let function = function(&program, "sum");
    let pointer = program.modules[0].types.iter().find(|one| one.id == function.values[0].r#type).unwrap();

    assert_eq!(function.parameters.len(), 1);
    assert_eq!(pointer.kind, model::TypeKind::Pointer);
    assert_eq!(pointer.width, 4);
    assert_eq!(pointer.rank, 1);
}

#[test]
fn test_a_range_is_not_a_slice() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "range_slice.nib",
        "fn sum(values: &[i16]) -> i16:\n    return values[0]\nfn main() -> i16:\n    let values: i16[4] = [1, 2, 3, 4]\n    return sum(&values[1..3])\n",
    );

    refused(&source);
}

#[test]
fn test_data_is_an_explicit_pointer_escape_hatch() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "data.nib",
        "fn data(values: &[i16]) -> addr:\n    return values.data()\nfn main() -> i16:\n    let values: i16[2] = [4, 9]\n    data(&values)\n    return 0\n",
    );

    let program = parsed(&source);
    let types = types(&program);
    assert_eq!(types["addr"].kind, model::TypeKind::Pointer);
    assert_eq!((types["addr"].width, types["addr"].address), (4, model::AddressKind::Far));
    assert!(!object_of(&program, "main", &source, &O2(), llrm_core::backend::objbuild::CodeLayout::OneSegment).expect("writes").is_empty());
}

#[test]
fn test_string_descriptor_methods_and_value_iteration_need_no_runtime() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "string_view.nib",
        "fn first(text: &string) -> char:\n    for byte in text:\n        return byte\n    return '\\0'\nfn size(text: string) -> u16:\n    return text.len + text.capacity\nfn main() -> u16:\n    let text: string = \"abc\"\n    if first(text) == 'a':\n        return size(text)\n    return 0\n",
    );

    let program = parsed(&source);
    assert!(program.modules[0].callables.iter().all(|one| !["len", "capacity", "iter", "next"].contains(&one.name.as_str())));
}

#[test]
fn test_return_inside_sequence_iteration_reaches_object_generation() {
    // `first` once left a dead increment block with non-dominating SSA values.
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "first.nib",
        "fn first(text: string) -> char:\n    for byte in text:\n        return byte\n    return '\\0'\nfn main() -> i16:\n    if first(\"metal\") == 'm':\n        print(\"ok\")\n        return 0\n    return 1\n",
    );

    let program = parsed(&source);
    assert!(!object_of(&program, "main", &source, &O2(), llrm_core::backend::objbuild::CodeLayout::OneSegment).expect("writes").is_empty());
}

#[test]
fn test_bounded_comprehension_materializes_and_generator_fuses() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "comprehension.nib",
        "fn main() -> i16:\n    let values: i16[4] = [1, 2, 3, 4]\n    let doubled = [value * 2 for value in values]\n    let mut total: i16 = 0\n    for value in (item + 1 for item in doubled):\n        total += value\n    return total\n",
    );

    let program = parsed(&source);
    assert!(program.modules[0].callables.iter().all(|one| !["iter", "next", "collect", "append"].contains(&one.name.as_str())));
    assert!(!object_of(&program, "main", &source, &O2(), llrm_core::backend::objbuild::CodeLayout::OneSegment).expect("writes").is_empty());
}

#[test]
fn test_dictionary_comprehension_deduplicates_and_has_explicit_lookup() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "dictionary.nib",
        "fn main() -> i16:\n    let values: i16[4] = [1, 2, 1, 3]\n    let table = {item: item * 10 for item in values}\n    return table.get(1, 0) + table.get(3, 0) + table.get(9, 5)\nfn count() -> u16:\n    let values: i16[4] = [1, 2, 1, 3]\n    let table = {item: item * 10 for item in values}\n    return table.len\n",
    );

    let program = parsed(&source);
    assert!(!object_of(&program, "main", &source, &O2(), llrm_core::backend::objbuild::CodeLayout::OneSegment).expect("writes").is_empty());
}

#[test]
fn test_a_repeat_literal_in_the_frame_is_one_string_fill() {
    // The fill loop stepped its byte address to zero under `!=`, which `fill` missed: 64 stores in a loop.
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "frame_fill.nib",
        "@export(\"cdecl16\")\nfn value(k: i16) -> i32:\n    let mut a: i32[64] = [0] * 64\n    unsafe:\n        a[k] = 5\n        return a[k] + a[k + 1]\n",
    );
    let assembly = listing(&parsed(&source), "value", &O2());
    let body = &assembly[assembly.find("_value proc").unwrap()..assembly.find("_value endp").unwrap()];

    assert!(body.contains("rep stosd"));
    assert!(!Regex::new(r"\bj\w+\s").unwrap().is_match(body));
}

#[test]
fn test_a_fill_leaves_the_rest_of_its_function_priceable() {
    // FILL had no price, so any function holding one refused every unroll: the 4-trip sum stayed a loop.
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "priced_fill.nib",
        "fn value(v: &[i16]) -> i32:\n    let mut a: i32[64] = [0] * 64\n    let mut total: i16 = 0\n    unsafe:\n        for i in 0..4:\n            total += v[i]\n        a[total] = 5\n    return a[1]\nfn main() -> i16:\n    let v: i16[4] = [1, 2, 3, 4]\n    return i16(value(&v))\n",
    );
    let assembly = listing(&parsed(&source), "main", &O2());
    let body = &assembly[assembly.find("_value proc").unwrap()..assembly.find("_value endp").unwrap()];

    assert!(body.contains("rep stosd"));
    assert!(!Regex::new(r"\bj\w+\s").unwrap().is_match(body));
}

#[test]
fn test_an_array_field_fills_and_copies_as_one_run_each() {
    // A 128-byte field's `[0] * 128` was 128 byte stores, and each copy of its struct 128 loads and stores.
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "field_runs.nib",
        "struct File:\n    handle: i16\n    mut buffer: u8[128]\n    mut start: u16\n\nfn opened(h: i16) -> File:\n    return File(handle=h, buffer=[0] * 128, start=0)\n\nfn relay(h: i16) -> File:\n    let f = opened(h)\n    return f\n\nfn main() -> i16:\n    let f = relay(3)\n    return f.handle + i16(f.buffer[5])\n",
    );
    // `opened` is inlined into `relay`: its fill and relay's copies are both there.
    let assembly = listing(&parsed(&source), "main", &O2());
    let relay = between(&assembly, "_relay proc", "_relay endp");

    assert!(relay.contains("rep stos"), "{relay}");
    assert!(relay.contains("rep movs") || Regex::new(r"\bj\w+\s").unwrap().is_match(relay), "{relay}");
    assert!(relay.matches("byte ptr").count() < 8, "{relay}");
}

#[test]
fn test_unroll_is_priced_against_the_loop_as_optimized() {
    // Unroll compared its settled copy with the loop mid-round: `b`'s fill became eight at -Os, not one.
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "priced_unroll.nib",
        concat!(
            "type fix = fixed i32, fraction=8\n",
            "fn value(k: i16) -> fix:\n",
            "    let mut a: fix[8, 8] = [[0] * 8] * 8\n",
            "    let mut b: fix[8, 8] = [[0] * 8] * 8\n",
            "    for i in 0..8:\n",
            "        for j in 0..8:\n",
            "            a[i, j] = fix(i * 3 + j + 1) / 4\n",
            "            if i == j:\n",
            "                b[i, j] = 2\n",
            "            else:\n",
            "                b[i, j] = fix((i + j) % 3) / 2\n",
            "    return a[k, 1] + b[k, 2]\n",
            "fn main() -> i16:\n",
            "    return i16(value(3) + value(4))\n",
        ),
    );
    let assembly = listing_on(&parsed(&source), "main", &level("Os"), "486");
    let body = &assembly[assembly.find("_value proc").unwrap()..assembly.find("_value endp").unwrap()];

    // Each fill is one string fill or one rolled store, never eight copies.
    let fills = Regex::new(r"rep stos[bwd]\n").unwrap().find_iter(body).count() + Regex::new(r"mov dword ptr \[[^\]]*\], 0\n").unwrap().find_iter(body).count();
    assert_eq!(fills, 2, "{body}");
}

#[test]
fn test_a_negative_index_is_out_of_bounds() {
    // The check compared signed, so `i < 0` proved `i < 8` and the optimizer deleted the panic.
    let directory = tempfile::tempdir().expect("a directory");
    let text = "@export(\"cdecl16\")\nfn value(i: i16) -> i16:\n    let a: i16[8] = [1] * 8\n    if i < 0:\n        return a[i]\n    return 0\n";
    let program = parsed(&written(&directory, "settled.nib", text));
    let mir = optimized_mir(&program, &O2());
    let panics = mir.lines().filter(|line| line.contains(" call ") && line.contains(rt::ERROR_BOUNDS)).count();

    assert_eq!(panics, 1, "{mir}");
}

#[test]
fn test_a_new_counter_steps_where_no_condition_is_live() {
    // A rotated loop branches on flags its body set; the pointer step went between them: Unlowered.
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "struct_view.nib",
        concat!(
            "struct sample:\n",
            "    tag: i16\n",
            "    mut value: i32\n",
            "    delta: i32\n",
            "@extern(\"cdecl16\")\n",
            "fn get() -> i16\n",
            "fn total(samples: &[sample]) -> i32:\n",
            "    let mut sum: i32 = 0\n",
            "    for one in &samples:\n",
            "        sum += one.value\n",
            "    return sum\n",
            "fn main() -> i16:\n",
            "    let mut s: sample[100] = [sample(tag=0, value=0, delta=2)] * 100\n",
            "    unsafe:\n",
            "        s[get()].value = i32(get())\n",
            "    return i16(total(&s))\n",
        ),
    );
    let assembly = listing(&parsed(&source), "main", &O2());
    let body = between(&assembly, "_main proc", "_main endp");

    assert!(Regex::new(r"add e?(?:si|di|bx|cx|dx|ax), \d+\n(?:L\w+:\n)?    jne").unwrap().is_match(body), "{body}");
}

#[test]
fn test_ill_formed_operators_conversions_and_repeats_are_rejected() {
    for body in [
        "    return 1 << 16\n",
        "    return 1 << -1\n",
        "    let a: i16 = 1\n    let b: u16 = 1\n    return i16(a + b)\n",
        "    let a: i8 = -1\n    let b: u16 = 1\n    return i16(a < b)\n",
        "    return i16(bool(1))\n",
        "    return i16(u8(300))\n",
        "    return i16(f64(1) & f64(2))\n",
        "    let a: i16[4] = [0] * 3\n    return a[0]\n",
    ] {
        let directory = tempfile::tempdir().expect("a directory");
        let source = written(&directory, "rejected.nib", &format!("fn value() -> i16:\n{body}"));
        refused(&source);
    }
}

#[test]
fn test_not_is_not_an_operand_of_a_tighter_operator() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(&directory, "not.nib", "fn value() -> bool:\n    return true == !false\n");
    refused(&source);
}

#[test]
fn test_a_float_does_not_convert_to_fixed_point() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "float_fixed.nib",
        "type fix = fixed i32, fraction=8\nfn value() -> i16:\n    let x: f64 = 1.5\n    return i16(fix(x))\n",
    );
    refused(&source);
}

#[test]
fn test_ranked_arrays_index_fill_and_borrow_row_major() {
    let program = parsed(&fixture("ranked.nib"));
    assert_eq!(program.array_order, model::ArrayOrder::RowMajor);
}

/// From `tests/test_hir_execute.py`, less its `execute.run`.
#[test]
fn test_borrowed_struct_arrays_and_reborrows_keep_scoped_mutation() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "struct_array_borrow.nib",
        "struct point:\n    mut x: i16\n    y: i16\nfn nudge(point: &mut point) -> void:\n    point.x += point.y\nfn update(points: &mut [point]) -> void:\n    for point in &mut points:\n        nudge(&mut point)\nfn calculate() -> i16:\n    let mut points: point[2] = [point(1, 2), point(10, 20)]\n    update(&mut points)\n    return points[0].x + points[1].x\n",
    );

    // Each nudge reaches the caller's array: 1 + 2 and 10 + 20.
    let assembly = listing(&parsed(&source), "calculate", &O2());
    let calculate = between(&assembly, "_calculate proc far", "_calculate endp");
    assert!(calculate.contains("mov ax, 33"), "{calculate}");
}

#[test]
fn test_ranked_arrays_reject_the_wrong_rank_or_shape() {
    for body in [
        "    let a: i16[2, 2] = [[0] * 2] * 2\n    return a[0]\n",
        "    let a: i16[2, 2, 2, 2, 2] = [[[[[0] * 2] * 2] * 2] * 2] * 2\n    return 0\n",
        "    let a: i16[2, 2] = [[1, 2], [3]]\n    return 0\n",
        "    let a: i16[2, 2] = [[0] * 3] * 2\n    return 0\n",
        "    let a: i16[2, 2] = [[0] * 2] * 2\n    return a.dim[2]\n",
        "    let a: i16[2, 2] = [[0] * 2] * 2\n    return first(&a)\n",
        "    let a: i16[2, 2] = [[0] * 2] * 2\n    let mut t: i16 = 0\n    for x in a:\n        t += x\n    return t\n",
    ] {
        let directory = tempfile::tempdir().expect("a directory");
        let source = written(
            &directory,
            "ranked_rejected.nib",
            &format!("fn first(values: &[i16]) -> i16:\n    return values[0]\nfn value() -> i16:\n{body}"),
        );
        refused(&source);
    }
}

#[test]
fn test_a_loop_past_max_completely_peel_times_stays_rolled() {
    // Copies were built for any trip count the simulation priced as folding: deedlines'
    // 16384-trip loops became 360K operations. LLVM analyses at most 10 iterations; past that nothing is copied.
    let directory = tempfile::tempdir().expect("a directory");
    let rolled = |trips: i16| {
        // `total * 3 + i` has no closed form, so only peeling removes the loop.
        let text = format!("@export(\"cdecl16\")\nfn value(k: i16) -> i16:\n    let mut total: i16 = k\n    for i in 0..{trips}:\n        total = total * 3 + i\n    return total\n");
        optimized_mir(&parsed(&written(&directory, "settled.nib", &text)), &O2()).contains(" = phi ")
    };
    assert!(!rolled(10), "within the cap the loop is copied out");
    assert!(rolled(11));
}

/// `_sum_three proc near` .. `endp` for the 486.
fn sum_three_on_486() -> String {
    let assembly = listing_on(&parsed(&fixture("sum_three.nib")), "main", &O2(), "486");
    between(&assembly, "_sum_three proc near", "_sum_three endp").to_owned()
}

/// Python's `re.search(r"(L\w+):\n(?:.*\n)*?\s*jne\s+\1\n", function)`: from the first
/// label a later `jne` returns to, through the nearest such `jne`.
fn closed_on_jne(function: &str) -> Option<String> {
    let lines: Vec<&str> = function.split_inclusive('\n').collect();
    for (at, line) in lines.iter().enumerate() {
        let Some(label) = line.strip_suffix(":\n").filter(|label| label.starts_with('L')) else { continue };
        let back = format!("jne {label}");
        if let Some(end) = lines[at + 1..].iter().position(|one| {
            one.ends_with('\n') && one.split_whitespace().collect::<Vec<_>>().join(" ") == back
        }) {
            return Some(lines[at..=at + 1 + end].concat());
        }
    }
    None
}

#[test]
fn test_a_loop_whose_latch_copies_ends_on_its_branch() {
    // sum_three left the latch's copies in the exit edge, so the loop ran je out plus jmp back.
    let function = sum_three_on_486();
    let top = closed_on_jne(&function).unwrap_or_else(|| panic!("no loop closes on jne:\n{function}"));
    assert!(!top.contains("jmp"), "{top}");
}

const COLUMN: &str = "\
fn column(m: &[i32, 2], j: i16) -> i32:
    let mut total: i32 = 0
    for k in 0..m.dim[0]:
        unsafe:
            total += m[k, j]
    return total

fn main() -> i16:
    let m: i32[4, 4] = [[1] * 4] * 4
    column(&m, 1)
    return 0
";

fn column_loop() -> String {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(&directory, "column.nib", COLUMN);
    let assembly = listing_on(&parsed(&source), "main", &O2(), "486");
    let function = between(&assembly, "_column proc near", "_column endp");
    closed_on_jne(function).unwrap_or_else(|| panic!("no loop closes on jne:\n{function}"))
}

#[test]
fn test_a_column_read_steps_a_pointer_by_its_runtime_stride() {
    // `k * dim + j` had no pointer: strength could not multiply a runtime step, so every trip rebuilt it.
    let loop_ = column_loop();
    assert!(!loop_.contains("shl") && !loop_.contains("[bp"), "{loop_}");
}

#[test]
fn test_a_pointer_stepped_by_a_runtime_stride_is_one_recurrence() {
    // Strength reduced the pointer's own step, leaving a lagging copy: `xchg` and `jmp` on every trip.
    let loop_ = column_loop();
    assert!(!Regex::new(r"\b(?:jmp|xchg)\b|mov \w\w, \w\w\n").unwrap().is_match(&loop_), "{loop_}");
}

#[test]
fn test_a_byte_argument_is_pushed_as_a_word() {
    // A u8 or char argument reached the push as `push al`, which the assembler rejects.
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "byte_argument.nib",
        "@extern(\"cdecl16\")\nfn put(c: char) -> void\n\nfn main() -> i16:\n    unsafe:\n        put('7')\n    return 0\n",
    );
    let assembly = listing(&parsed(&source), "main", &O2());
    assert!(assembly.contains("call far ptr _put"), "{assembly}");
    assert!(!Regex::new(r"push [abcd]l\b").unwrap().is_match(&assembly), "{assembly}");
}

/// `while true:` branched on a constant, which lowering refused: "branch condition must be a value".
#[test]
fn test_a_branch_on_a_constant_lowers_as_a_jump() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "forever.nib",
        "fn main() -> i16:\n    let mut n = 3\n    while true:\n        if n == 0:\n            return 7\n        n -= 1\n    return 0\n",
    );
    listing(&parsed(&source), "main", &O2());
}

/// A vec borrowed as `&[T]` takes DGROUP's selector for its far data
/// pointer, which the object writer could not name: "KeyError: (grp, 0)".
#[test]
fn test_a_vec_view_names_dgroup_in_the_object() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "view.nib",
        "fn total(values: &[i16]) -> i16:\n    let mut sum = 0\n    for value in values:\n        sum += value\n    return sum\n\nfn main() -> i16:\n    let values = [x * x for x in [1, 2, 3]]\n    return total(values)\n",
    );
    object_of(&parsed(&source), "main", &source, &level("O2"), llrm_core::backend::objbuild::CodeLayout::OneSegment).expect("writes an object");
}

/// `v[0].bump()` passed the element's near pointer where `&mut T` is far:
/// the host ran it, DOS bumped whatever the stale segment pointed at.
#[test]
fn test_a_method_on_a_vec_element_takes_a_far_pointer() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "bump.nib",
        "struct T:\n    mut n: i16\n\nfn T.bump(self: &mut T) -> void:\n    self.n += 1\n\nfn main() -> i16:\n    let mut v: vec[T] = [T(n=0)]\n    v[0].bump()\n    return v[0].n\n",
    );
    parsed(&source);
}

/// `for t in v: t.get()` borrowed the loop binding's near element pointer
/// for a far `&T`: "call ... passes argument 0 in the wrong width".
#[test]
fn test_a_method_on_a_loop_binding_over_a_vec_takes_a_far_pointer() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "each.nib",
        "struct T:\n    mut n: i16\n\nfn T.get(self: &T) -> i16:\n    return self.n\n\nfn main() -> i16:\n    let v: vec[T] = [T(n=2), T(n=3)]\n    let mut total = 0\n    for t in v:\n        total += t.get()\n    return total\n",
    );
    parsed(&source);
}

/// An `extern` function is called by its C symbol, and an `export`ed one is
/// public under its own, so C can call it back.
#[test]
fn test_foreign_functions_link_by_their_c_symbols() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "interop.nib",
        "@extern(\"cdecl16\", name=\"_sum_all\")\nfn total(values: *far i16, count: u16) -> i32\n\n@export(\"cdecl16\")\nfn weight(value: i16) -> i16:\n    return value * 2\n\nfn main() -> i16:\n    let values: i16[2] = [1, 2]\n    unsafe:\n        return i16(total(&values, 2))\n",
    );
    let module = nib_compile::assembled(&parsed(&source), "main", &level("O2"), &crate::real_mode().os)
    .expect("assembles");
    assert_eq!(module.publics, ["_weight", "_main"]);
    assert!(
        module
            .externs
            .contains(&("_sum_all".to_owned(), "far".to_owned())),
        "{:?}",
        module.externs
    );
}

#[test]
/// Every float program failed in the backend: binary32 evaluation is not what
/// an x87 load encodes, a float argument was pushed as ten bytes, an unread
/// float parameter was loaded anyway, and truncation was named fistp.
fn test_float_arguments_comparisons_and_truncation_reach_the_object() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "floats.nib",
        "fn unused(x: f32) -> i16:\n    return 1\n\nfn above(x: f32) -> i16:\n    if x > 1.0:\n        return i16(x)\n    return 0\n\nfn main() -> i16:\n    return above(2.5) + unused(1.5)\n",
    );
    object_of(&parsed(&source), "main", &source, &level("O2"), llrm_core::backend::objbuild::CodeLayout::OneSegment).expect("writes an object");
}

#[test]
/// pascal16 pushes the first argument first, names symbols in upper case, and
/// the callee removes the arguments with `retf n`.
fn test_pascal_functions_push_in_order_and_clean_up_after_themselves() {
    let source = std::path::PathBuf::from(concat!(env!("LLRM_ROOT"), "/examples/pascal/levels.nib"));
    let module =
        nib_compile::assembled(&parsed(&source), "main", &level("O2"), &crate::real_mode().os).expect("assembles");
    assert_eq!(module.publics, ["CLAMP", "_main"]);
    assert!(module.externs.contains(&("SCALE".to_owned(), "far".to_owned())), "{:?}", module.externs);
    let text = masm::text(&module).expect("prints");
    let clamp = &text[text.find("CLAMP proc far").expect("CLAMP")..text.find("CLAMP endp").expect("its end")];
    assert!(clamp.contains("retf 6") && !clamp.contains("retf\n"), "{clamp}");
    // scale(level, 255, 100): 255 is pushed first, so it is the high word of the pair.
    assert!(text.contains(&format!("pushd {}", 255 << 16 | 100)), "{text}");
}

#[test]
/// QB returns a float through a hidden near pointer, its last parameter. A
/// pascal16 function whose last parameter merely has that type returned
/// its result there instead of in st(0).
fn test_a_pascal_float_result_returns_in_st0_whatever_its_last_parameter() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "half.nib",
        "@export(\"pascal16\")\nfn half(value: f32, out: *near f32) -> f32:\n    return value / 2.0\n\nfn main() -> i16:\n    return 0\n",
    );
    let text = listing_on(&parsed(&source), "main", &level("O2"), "486");
    let half = between(&text, "HALF proc far", "HALF endp");
    assert!(!half.contains("fstp") && half.contains("retf 6"), "{half}");
}

#[test]
/// A pascal16 extern returns a float in st(0), as its ABI says. The call
/// read it BASIC's way: the result's address from ax, then `fld [bx]`.
fn test_a_pascal_float_result_is_read_from_st0() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "scaled.nib",
        "@extern(\"pascal16\")\nfn scale(value: f32) -> f32\n\n@export(\"pascal16\")\nfn twice(value: f32) -> f32:\n    unsafe:\n        return scale(value) * 2.0\n",
    );
    let text = listing_on(&parsed(&source), "main", &level("O2"), "486");
    let twice = between(&text, "TWICE proc far", "TWICE endp");
    assert!(twice.contains("call far ptr SCALE") && !twice.contains("[bx]"), "{twice}");
}

#[test]
/// A near raw pointer to a module struct is its offset. It was taken as the
/// far address and copied to the near type, which the HIR refused.
fn test_a_near_raw_pointer_to_a_module_struct_is_its_offset() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "near.nib",
        "@repr(\"c16\", pack=1)\nstruct Pair:\n    low: u16\n    high: u16\n\nvar pair: Pair = Pair(low=1, high=2)\n\n@extern(\"pascal16\")\nfn take(pair: *near Pair) -> u16\n\n@export(\"pascal16\")\nfn give() -> u16:\n    unsafe:\n        return take(&pair)\n",
    );
    let text = listing_on(&parsed(&source), "main", &level("O2"), "486");
    assert!(between(&text, "GIVE proc far", "GIVE endp").contains("push offset"), "{text}");
}

#[test]
/// Any integer converts to a float: a parameter, a temporary, a constant, of
/// any width or sign. Only one already in memory in an x87 format did;
/// `f64(high)` of an i16 parameter was "integer-to-float conversion needs a place",
/// and the rich route refused every unsigned one: "UIToFP of a float".
#[test]
fn test_any_integer_operand_converts_to_a_float() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "floats.nib",
        "@export(\"pascal16\")\nfn mixed(small: i8, byte: u8, word: u16, long: u32, high: i16) -> f64:\n    return f64(high) + f64(small) + f64(byte) + f64(word) + f64(long) + f64(high + 1) + f64(u16(7))\n",
    );
    let options = llrm_driver::m16_options(nib_compile::machine(&llrm_x86_m16::M16, &crate::real_mode().os));
    let module = nib_compile::assembled(&parsed(&source), "main", &options, &crate::real_mode().os).unwrap_or_else(|error| panic!("{error}"));
    let text = masm::text(&module).expect("prints");
    let mixed = between(&text, "MIXED proc far", "MIXED endp");
    // Each unsigned is read signed at twice its width: u8 a word, u16 a
    // dword, u32 a qword whose high dword is zero.
    assert!(mixed.contains("movzx ax, dl") && mixed.contains("movzx eax, cx"), "{mixed}");
    let qword = Regex::new(r"mov dword ptr \[bp-(\d+)\], ebx\n\s*mov dword ptr \[bp-(\d+)\], 0\n\s*fild qword ptr \[bp-(\d+)\]").unwrap();
    let cells = qword.captures(mixed).unwrap_or_else(|| panic!("{mixed}"));
    let at = |group: usize| cells[group].parse::<i64>().unwrap();
    assert_eq!((at(1), at(2)), (at(3), at(3) - 4), "{mixed}");
}

/// An unsigned integer converts to the float of its value, not of its bits
/// read signed: 65535 as a u16 is 65535.0, not -1.0.
#[test]
fn test_an_unsigned_integer_converts_to_its_value() {
    use llrm_mir::interpret::{self, Val};
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "unsigned.nib",
        "@export(\"cdecl16\")\nfn byte(x: u8) -> f64:\n    return f64(x)\n\n\
         @export(\"cdecl16\")\nfn word(x: u16) -> f64:\n    return f64(x)\n\n\
         @export(\"cdecl16\")\nfn long(x: u32) -> f64:\n    return f64(x)\n",
    );
    let program = parsed(&source);
    let options = llrm_driver::m16_options(nib_compile::machine(&llrm_x86_m16::M16, &crate::real_mode().os));
    let (mut mir, _) = llrm_core::driver::emitted(&program, &options).expect("emits");
    llrm_core::driver::optimized(&mut mir, &options).expect("optimizes");
    for (name, width, bits, value) in [("byte", 8, 0xff, 255.0), ("word", 16, 0xffff, 65535.0), ("long", 32, 0xffff_ffff, 4294967295.0_f64)] {
        let got = interpret::run(&mir.modules[0], &format!("_{name}"), vec![Val::Int { bits, width }], 1_000).unwrap_or_else(|trap| panic!("{name}: {trap:?}"));
        assert!(matches!(got, Val::Float(_, found) if found == f64::to_bits(value)), "{name}: {got:?}");
    }
}

#[test]
/// Section 9.2: an aggregate of 4 bytes or less comes back in registers,
/// with no hidden slot pointer; a larger one still takes the slot.
fn test_small_aggregates_return_in_registers() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "r.nib",
        "struct Point:\n    mut x: i16\n    y: i16\n\n@repr(\"c16\", pack=1)\nstruct Cell:\n    glyph: u8\n    count: i16\n\n\
         struct Box:\n    low: Point\n    high: Point\n\n\
         fn point(x: i16, y: i16) -> Point:\n    return Point(x=x, y=y)\n\n\
         fn cell(glyph: u8) -> Cell:\n    return Cell(glyph=glyph, count=300)\n\n\
         fn box(p: i16) -> Box:\n    return Box(low=point(p, p), high=point(p, p))\n\n\
         fn main() -> i16:\n    let c = cell(65)\n    return point(3, 4).y + box(1).high.x + i16(c.glyph)\n",
    );
    let program = parsed(&source);
    let shape = |name: &str| {
        let function = function(&program, name);
        (function.parameters.len(), types(&program).values().find(|one| one.id == function.result_type).expect("a type").width)
    };
    assert_eq!([shape("point"), shape("cell"), shape("box")], [(2, 4), (1, 4), (2, 0)]);
    object_of(&program, "main", &source, &level("O2"), llrm_core::backend::objbuild::CodeLayout::OneSegment).expect("writes an object");
}

#[test]
/// Points-to read a call's pointer result as the contents of the escaped
/// cells the call reads: a new vec's buffer took the frame array a view had
/// published, and its elements were written through SS, not DS.
fn test_a_call_result_does_not_point_into_a_frame_the_call_can_read() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "grow.nib",
        "fn total(values: &[i16]) -> i16:\n    return 0\n\n\
         fn main() -> i16:\n    let one: i16[1] = [7]\n    total(&one)\n    let many: vec[i16] = [1, 2]\n    return many[1]\n",
    );
    let text = listing_on(&parsed(&source), "main", &level("O2"), "486");
    let main = between(&text, "_main proc far", "_main endp");
    assert!(!main.contains("ss:[bx"), "{main}");
}

/// A bounds check's panic block ends in an escape with no source bytes:
/// "'NoneType' object has no attribute 'op'" in jump threading.
#[test]
fn test_a_panic_path_reaches_the_object() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "checked.nib",
        "fn at(values: &[i16], i: u16) -> i16:\n    return values[i]\n\nfn main() -> i16:\n    let v: i16[3] = [1, 2, 3]\n    return at(&v, 1)\n",
    );
    object_of(&parsed(&source), "main", &source, &level("O2"), llrm_core::backend::objbuild::CodeLayout::OneSegment).expect("writes an object");
}

#[test]
fn a_float_converts_to_every_integer_width() {
    // "i8: no floating storage format": no x87 store holds a byte, and a
    // `fistp word` cannot hold u16's top half.
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "convert.nib",
        "@export(\"pascal16\")\nfn convert(x: f64, y: f64) -> i16:\n    print(f\"{u8(x)} {i8(x - 300.0)} {u16(x * 200.0)} {u32(y)} {i32(x)}\")\n    return 0\n",
    );
    let text = listing_on(&parsed(&source), "convert", &level("Os"), llrm_target::Target::default_cpu(&llrm_x86_m16::M16));
    assert!(text.contains("fistp qword"), "{text}");
}

/// Parsing ran `cargo run --release` on this crate: after any edit, the first
/// test waited a minute for a release rebuild and every other one for its lock.
#[test]
fn test_parsing_runs_no_cargo() {
    if std::env::var_os("LLRM_NO_CARGO").is_some() {
        parsed(&fixture("fixed.nib"));
        return;
    }
    let name = concat!(module_path!(), "::test_parsing_runs_no_cargo").split_once("::").expect("a crate").1;
    let run = std::process::Command::new(std::env::current_exe().expect("the test binary"))
        .args(["--exact", name])
        .env("LLRM_NO_CARGO", "1")
        .env("PATH", "")
        .output()
        .expect("runs");
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stdout));
}

#[test]
fn a_pointer_loaded_from_a_local_descriptor_still_reaches_its_array() {
    // The data pointer read back from a slice descriptor had no exact cell
    // fact, so it reached nothing unescaped: the array's stores were dropped
    // and DOS printed stack garbage for 7, 8 and 9.
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "enumerate.nib",
        "fn main() -> i16:\n    let values: i16[3] = [7, 8, 9]\n    for (i, x) in enumerate(values):\n        print(f\"{i}: {x}\")\n    return 0\n",
    );
    let text = listing_on(&parsed(&source), "main", &level("Os"), llrm_target::Target::default_cpu(&llrm_x86_m16::M16));
    for value in [", 7", ", 8", ", 9"] {
        assert!(text.contains(value), "{value} is never stored:\n{text}");
    }
}

const HELPERS: &str = "\
fn twice(value: i16) -> i16:
    return value + value

fn scaled(value: i16) -> i16:
    return twice(value) + 1

fn main() -> i16:
    let mut total: i16 = 0
    for i in 0..10:
        total += scaled(i)
    return total
";

#[test]
fn a_private_one_line_helper_is_inlined_into_its_caller() {
    // Each function was optimized alone, so scaled kept
    // `push [bp+6] / call far ptr _twice` for a one-line add.
    let directory = tempfile::tempdir().expect("a directory");
    let text = listing(&parsed(&written(&directory, "helper.nib", HELPERS)), "main", &O2());
    assert!(!text.lines().any(|line| line.contains("call") && line.contains("_twice")), "{text}");
}

#[test]
fn a_call_inlined_away_leaves_no_extern() {
    // The call table outlived the inlined call: main declared
    // `extern _scaled:far` for a procedure the module no longer has.
    let directory = tempfile::tempdir().expect("a directory");
    let text = listing(&parsed(&written(&directory, "helper.nib", HELPERS)), "main", &O2());
    assert!(!text.contains("_scaled"), "{text}");
}

#[test]
fn test_each_procedure_has_a_code_segment_the_linker_may_drop() {
    // One segment held every procedure, so a program linked all of the
    // runtime even when it called one routine.
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(&directory, "two.nib", "@export(\"cdecl16\")\nfn unused(x: i16) -> i16:\n    return x + 1\n\nfn main() -> i16:\n    print(3)\n    return 0\n");
    let object = object_of(&parsed(&source), "main", &source, &level("O2"), llrm_core::backend::objbuild::CodeLayout::PerProcedure).expect("writes");
    let records = llrm_core::objectfile::omf::parse(&object).expect("parses");
    let segments = records.iter().filter(|one| one.r#type & 0xFE == llrm_core::objectfile::omf::SEGDEF).count();
    // Two procedures, and _DATA.
    assert_eq!(segments, 3);
}

#[test]
fn test_an_object_defines_each_segment_once_unless_asked_for_one_per_procedure() {
    // A segment per procedure, all of one name, was the default: Microsoft
    // LINK 3.69 read them as one and refused SORTLIB.OBJ with L1103.
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(&directory, "two.nib", "@export(\"cdecl16\")\nfn unused(x: i16) -> i16:\n    return x + 1\n\nfn main() -> i16:\n    print(3)\n    return 0\n");
    let object = object_of(&parsed(&source), "main", &source, &level("O2"), llrm_core::backend::objbuild::CodeLayout::OneSegment).expect("writes");
    let records = llrm_core::objectfile::omf::parse(&object).expect("parses");
    let segments = records.iter().filter(|one| one.r#type & 0xFE == llrm_core::objectfile::omf::SEGDEF).count();
    // The code, and _DATA.
    assert_eq!(segments, 2);
}

#[test]
fn test_a_computed_float_argument_is_passed_through_memory() {
    // x87 cannot push: "floating instruction has no allocation rule".
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(&directory, "pushed.nib", "fn half(x: f64) -> f64:\n    return x / 2.0\n\n@export(\"cdecl16\")\nfn quarter(x: f32, y: f64) -> f64:\n    print(x * 2.0)\n    return half(y) / 2.0\n\nfn main() -> i16:\n    return 0\n");
    object_of(&parsed(&source), "main", &source, &level("O2"), llrm_core::backend::objbuild::CodeLayout::OneSegment).expect("writes an object");
}

#[test]
/// Section 9.2: a far pointer comes back in dx:ax, where C and BASIC read
/// it. It came back in eax, so PDS's STRINGADDRESS result was misread.
fn test_a_far_pointer_result_travels_in_dx_ax() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "far.nib",
        "@extern(\"pascal16\")\nfn address(of: *near u8) -> *far u8\n\n@export(\"pascal16\")\nfn first(bytes: *far u8) -> *far u8:\n    return bytes\n\n@export(\"pascal16\")\nfn through(of: *near u8) -> u8:\n    unsafe:\n        let p = address(of)\n        return *p\n",
    );
    let text = listing_on(&parsed(&source), "main", &level("O2"), "486");
    let first = between(&text, "FIRST proc far", "FIRST endp");
    assert!(!first.contains("eax") && first.contains("mov dx,"), "{first}");
    let through = between(&text, "THROUGH proc far", "THROUGH endp");
    assert!(!through.contains("eax") && through.contains("es, dx"), "{through}");
}

#[test]
/// Section 15: a qb45 export takes BASIC's arguments first to last, each a
/// near pointer, and removes them; it links without the Nib runtime.
fn test_a_qb45_library_takes_basic_arguments_by_reference() {
    let source = root().join("examples/basic/sortlib.nib");
    let module = nib_compile::assembled(&parsed(&source), "main", &level("O2"), &crate::real_mode().os).expect("assembles");
    assert_eq!(module.publics, ["SORTSCORES", "UPPER", "AVERAGE", "ROWTOTAL", "INITIALS"]);
    let externs: Vec<&str> = module.externs.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(externs, ["B$SCPY", "MEAN"]);
    let text = masm::text(&module).expect("prints");
    let sort = between(&text, "SORTSCORES proc far", "SORTSCORES endp");
    // scores() is pushed first, so it is further from the return address than count.
    assert!(sort.contains("retf 4") && sort.contains("word ptr [bp+8]"), "{sort}");
    let upper = between(&text, "UPPER proc far", "UPPER endp");
    assert!(upper.contains("retf 2"), "{upper}");
    // Mean# takes two locals by reference and the DOUBLE's pointer, and returns that pointer.
    let average = between(&text, "AVERAGE proc far", "AVERAGE endp");
    assert!(average.matches("lea ").count() >= 3 && average.contains("call far ptr MEAN\n    mov bx, ax\n    fld qword ptr [bx]"), "{average}");
    // A rank-2 view reads both dimensions' counts: the descriptor's words at 14 and 18.
    let rows = between(&text, "ROWTOTAL proc far", "ROWTOTAL endp");
    let field = |offset: i64| Regex::new(&format!(r"word ptr \[\w+\+{offset}\]")).unwrap().is_match(rows);
    assert!(field(14) && field(18), "{rows}");
    let initials = between(&text, "INITIALS proc far", "INITIALS endp");
    // The result is copied into BASIC's string by B$SCPY, inline or through `abi.qb45.string_result`.
    assert!((initials.contains("call far ptr B$SCPY") || initials.contains("call far ptr _abi.qb45.string_result")) && initials.contains("retf 2"), "{initials}");
}

#[test]
/// Section 15: BASIC gives a SINGLE or DOUBLE function a near pointer, pushed
/// last, to store its result through, and reads the pointer back from ax.
fn test_a_basic_float_result_goes_through_its_hidden_pointer() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(&directory, "half.nib", "@export(\"qb45\")\nfn half(value: f64) -> f64:\n    return value / 2.0\n");
    let text = listing_on(&parsed(&source), "main", &level("O2"), "486");
    let half = between(&text, "HALF proc far", "HALF endp");
    assert!(half.contains("mov bx, word ptr [bp+6]") && half.contains("fstp qword ptr [bx]"), "{half}");
    assert!(half.contains("mov ax, bx") && half.contains("retf 10"), "{half}");
}

#[test]
/// Section 15: a PDS or VB-DOS string is far, and only its runtime's
/// STRINGADDRESS and STRINGLENGTH read the descriptor.
fn test_a_far_basic_string_is_read_through_its_runtime() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "count.nib",
        "import abi.pds71 as pds\n\n@export(\"pds71\")\nfn Spaces(text: pds.StringRef) -> i16:\n    let mut count: i16 = 0\n    for letter in text:\n        if letter == ' ':\n            count += 1\n    return count\n",
    );
    let module = nib_compile::assembled(&parsed(&source), "main", &level("O2"), &crate::real_mode().os).expect("assembles");
    let externs: Vec<&str> = module.externs.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(externs, ["STRINGADDRESS", "STRINGLENGTH"]);
    let text = masm::text(&module).expect("prints");
    assert!(between(&text, "SPACES proc far", "SPACES endp").contains("retf 2"), "{text}");
}

#[test]
/// An interrupt16 handler may interrupt anything: it saves every register,
/// runs with DGROUP in DS and ES and the direction flag clear, and leaves
/// by `iret`. Its name is its far address, a `dd` the linker fills.
fn test_an_interrupt_handler_saves_every_register_and_returns_with_iret() {
    let source = root().join("examples/ticker.nib");
    let program = parsed(&source);
    let text = listing_on(&program, "main", &level("O2"), "486");
    let lines: Vec<&str> = between(&text, "_tick proc far", "_tick endp").lines().map(str::trim).collect();
    // The frame, once (PUSHAD, then the segments), then DGROUP in DS and ES.
    let start = lines.iter().position(|one| *one == "pushad").expect("saves the registers first");
    assert_eq!(
        lines[start..start + 10],
        ["pushad", "push ds", "push es", "push fs", "push gs", "pushw DGROUP", "pop ds", "push ds", "pop es", "cld"],
        "{text}"
    );
    assert_eq!(lines[lines.len() - 6..], ["pop gs", "pop fs", "pop es", "pop ds", "popad", "iret"], "{text}");
    assert!(text.contains("dd _tick"), "{text}");
    object_of(&program, "main", &source, &level("O2"), llrm_core::backend::objbuild::CodeLayout::OneSegment).expect("encodes");
}

#[test]
/// A variable a handler names changes under the program. The busy wait
/// read it once, before the loop, and spun forever.
fn test_a_variable_a_handler_names_is_read_on_every_pass() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "wait.nib",
        "var ticks: u16 = 0\n\n@export(\"interrupt16\")\nfn tick() -> void:\n    ticks += 1\n\nfn main() -> i16:\n    while ticks < 36:\n        continue\n    return 0\n",
    );
    let text = listing(&parsed(&source), "main", &O2());
    let main = between(&text, "_main proc", "_main endp");
    assert!(Regex::new(r"(L\w+):\n    cmp word ptr \S+, 36\n    jb (L\w+)\n").unwrap().captures(main).is_some_and(|loop_| loop_[1] == loop_[2]), "{text}");
}

#[test]
/// Nothing calls a handler, and an interrupt passes it nothing.
fn test_an_interrupt_handler_takes_nothing_and_is_not_called() {
    let directory = tempfile::tempdir().expect("a directory");
    let taking = written(&directory, "taking.nib", "@export(\"interrupt16\")\nfn tick(n: i16) -> void:\n    return\n\nfn main() -> i16:\n    return 0\n");
    assert!(refused(&taking).contains("takes nothing and returns void"));
    let called = written(&directory, "called.nib", "@export(\"interrupt16\")\nfn tick() -> void:\n    return\n\nfn main() -> i16:\n    tick()\n    return 0\n");
    assert!(refused(&called).contains("only an interrupt enters it"));
}

#[test]
/// A byte parameter passed on to a call took a fresh value numbered as the
/// parameter was: "value#1 is defined 2 times".
fn test_a_byte_parameter_passed_on_to_a_call_compiles() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "byte.nib",
        "@extern(\"cdecl16\")\nfn put(number: u8) -> void\n\nfn set(number: u8) -> void:\n    unsafe:\n        put(number)\n\nfn main() -> i16:\n    set(28)\n    set(29)\n    return 0\n",
    );
    let text = listing(&parsed(&source), "main", &O2());
    let main = between(&text, "_main proc", "_main endp");
    assert_eq!(main.matches("call far ptr _put").count(), 2, "{text}");
    assert!(main.contains("movzx"), "{text}");
}

#[test]
/// Section 9.2: a far pointer comes back in dx:ax. The caller read eax,
/// so a DOS vector the runtime returned lost its segment.
fn test_a_far_pointer_result_comes_back_in_dx_ax() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "far.nib",
        "@extern(\"cdecl16\")\nfn get(number: u16) -> *far u8\n\nvar kept: *far u8 = 0\n\nfn main() -> i16:\n    unsafe:\n        kept = get(3)\n    return 0\n",
    );
    let text = listing(&parsed(&source), "main", &O2());
    assert!(Regex::new(r"mov word ptr \S+\+2, dx\n").unwrap().is_match(&text), "{text}");
}

/// An inline block is its bytes in place of a call, fed and read in the
/// registers it names: `a` goes to both cx and dx, `b` and 7 are packed into
/// ax, and `sum` and `high` come out of bx and ch.
#[test]
fn test_inline_assembly_is_its_bytes_between_its_register_constraints() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "blocks.nib",
        "fn mix(a: u16, b: u8) -> u16:\n    let mut high: u8 = 0\n    unsafe:\n        \
         asm(cx=a, dx=a, al=b, ah=7, out=(bx=let sum, ch=high), clobbers=[flags]):\n            \
         mov bx, cx\n            add bx, dx\n            add bl, al\n        return sum + a + u16(high) + (a ^ 77) * 3 + (a >> 1) * 5 + (a << 3)\n\n\
         fn five() -> u16:\n    return 5\n\n\
         fn main() -> i16:\n    return i16(mix(3, 4) + mix(five(), 9))\n",
    );
    let assembly = listing_on(&parsed(&source), "main", &O2(), "486");
    let mix = between(&assembly, "_mix proc near", "_mix endp");
    let pattern = r"(?s)or ax, 1792\n    mov dx, (\w+)\n    mov cx, (\w+)\n    db 089h,0cbh,001h,0d3h,000h,0c3h\n    mov ax, bx\n    shr cx, 8\n";
    let found = Regex::new(pattern).unwrap().captures(mix).unwrap_or_else(|| panic!("{mix}"));
    assert_eq!(found[1], found[2], "{mix}");
    assert!(!["ax", "bx", "cx", "dx"].contains(&&found[1]), "a is kept where the block leaves it: {mix}");
}

/// A block that declares `memory` reaches what its pointer inputs point to:
/// `bytes[3]` is read again after it, and only then.
#[test]
fn test_inline_assembly_declaring_memory_is_read_again_after() {
    let directory = tempfile::tempdir().expect("a directory");
    let text = "var bytes: u8[4] = [1, 2, 3, 4]\n\n\
        fn poke() -> u16:\n    let before = u16(bytes[3])\n    unsafe:\n        \
        let base: *near mut u8 = &mut bytes\n        asm(si=base, clobbers=[memory]):\n            \
        mov byte ptr [si+3], 7\n    return u16(bytes[3]) + before\n\n\
        fn main() -> i16:\n    return i16(poke())\n";
    let reads = |text: &str| {
        let assembly = listing(&parsed(&written(&directory, "poke.nib", text)), "main", &O2());
        // poke is inlined into main.
        between(&assembly, "db 0c6h,044h,003h,007h", "_main endp").contains("byte ptr $var_bytes+3")
    };
    assert!(reads(text));
    assert!(!reads(&text.replace("clobbers=[memory]", "clobbers=[]")));
}

/// Inputs were ordered by `Reg`'s name, not by its slots: `si=1, di=2`
/// loaded 2 into si and 1 into di.
#[test]
fn test_inline_assembly_inputs_reach_the_registers_they_name() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "pair.nib",
        "fn main() -> i16:\n    unsafe:\n        asm(si=1, di=2, ax=3, clobbers=[]):\n            cli\n    return 0\n",
    );
    let assembly = listing(&parsed(&source), "main", &O2());
    let before = between(&assembly, "_main proc far", "db 0fah");
    for set in ["mov si, 1", "mov di, 2", "mov ax, 3"] {
        assert!(before.contains(set), "{set}: {before}");
    }
}

/// Outputs were pinned by value, and a loop unrolled after physicalization
/// gave each copy new values: spin(3) read its `cx` output from ax.
#[test]
fn test_inline_assembly_outputs_survive_unrolling() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "spin.nib",
        "fn spin(n: u16) -> u16:\n    let mut total: u16 = 0\n    let mut i: u16 = 0\n    while i < n:\n        \
         unsafe:\n            asm(bx=i, out=(cx=let got), clobbers=[]):\n                mov cx, bx\n            \
         total += got\n        i += 1\n    return total\n\nfn main() -> i16:\n    return i16(spin(3))\n",
    );
    let assembly = listing(&parsed(&source), "main", &O2());
    let after: Vec<&str> = assembly.split("db 089h,0d9h\n").skip(1).map(|rest| rest.lines().next().unwrap_or("")).collect();
    assert_eq!(after.len(), 3, "{assembly}");
    assert!(after.iter().all(|line| line.ends_with(", cx")), "{assembly}");
}

/// `source` through the rich MIR, as masm.
fn rich(directory: &tempfile::TempDir, name: &str, source: &str) -> String {
    let program = parsed(&written(directory, name, source));
    let module = nib_compile::assembled(&program, "main", &llrm_driver::m16_options(nib_compile::machine(&llrm_x86_m16::M16, &crate::real_mode().os)), &crate::real_mode().os).unwrap_or_else(|error| panic!("{error}"));
    masm::text(&module).expect("prints")
}

/// The rich MIR refused every inline block as "HIR asm": examples/speaker.nib
/// did not compile. Its bytes stand where it is called, fed and read in the
/// registers it names: `a` goes to both cx and dx, `b` and 7 are packed into
/// ax, and `sum` and `high` come out of bx and ch.
#[test]
fn test_the_rich_mir_lays_an_inline_block_between_its_register_constraints() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = "fn mix(a: u16, b: u8) -> u16:\n    let mut high: u8 = 0\n    unsafe:\n        \
         asm(cx=a, dx=a, al=b, ah=7, out=(bx=let sum, ch=high), clobbers=[flags]):\n            \
         mov bx, cx\n            add bx, dx\n            add bl, al\n        return sum + a + u16(high) + (a ^ 77) * 3 + (a >> 1) * 5 + (a << 3)\n\n\
         fn five() -> u16:\n    return 5\n\n\
         fn main() -> i16:\n    return i16(mix(3, 4) + mix(five(), 9))\n";
    assert!(source.contains("asm("), "the shape that was refused");
    let assembly = rich(&directory, "blocks.nib", source);
    let mix = between(&assembly, "_mix proc near", "_mix endp");
    let pattern = r"(?s)or ax, 1792\n    mov dx, (\w+)\n    mov cx, (\w+)\n    db 089h,0cbh,001h,0d3h,000h,0c3h\n    mov ax, bx\n    shr cx, 8\n";
    let found = Regex::new(pattern).unwrap().captures(mix).unwrap_or_else(|| panic!("{mix}"));
    assert_eq!(found[1], found[2], "{mix}");
    assert!(!["ax", "bx", "cx", "dx"].contains(&&found[1]), "a is kept where the block leaves it: {mix}");
}

/// A block that declares `memory` reaches what its pointer inputs point to:
/// `bytes[3]` is read again after it, and only then.
#[test]
fn test_the_rich_mir_reads_memory_again_after_a_block_declaring_it() {
    let directory = tempfile::tempdir().expect("a directory");
    let text = "var bytes: u8[4] = [1, 2, 3, 4]\n\n\
        fn poke() -> u16:\n    let before = u16(bytes[3])\n    unsafe:\n        \
        let base: *near mut u8 = &mut bytes\n        asm(si=base, clobbers=[memory]):\n            \
        mov byte ptr [si+3], 7\n    return u16(bytes[3]) + before\n\n\
        fn main() -> i16:\n    return i16(poke())\n";
    let reads = |text: &str| between(&rich(&directory, "poke.nib", text), "db 0c6h,044h,003h,007h", "retf").contains("byte ptr $var_bytes+3");
    assert!(reads(text));
    assert!(!reads(&text.replace("clobbers=[memory]", "clobbers=[]")));
}

/// Inputs reach the registers they name, whatever order they are written in.
#[test]
fn test_the_rich_mir_loads_each_input_into_its_register() {
    let directory = tempfile::tempdir().expect("a directory");
    let assembly = rich(&directory, "pair.nib", "fn main() -> i16:\n    unsafe:\n        asm(si=1, di=2, ax=3, clobbers=[]):\n            cli\n    return 0\n");
    let before = between(&assembly, "_main proc far", "db 0fah");
    for set in ["mov si, 1", "mov di, 2", "mov ax, 3"] {
        assert!(before.contains(set), "{set}: {before}");
    }
}

/// A block's output is read from its register after a loop is unrolled: each
/// of the three copies reads cx.
#[test]
fn test_the_rich_mir_reads_a_block_output_from_its_register_when_unrolled() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = "fn spin(n: u16) -> u16:\n    let mut total: u16 = 0\n    let mut i: u16 = 0\n    while i < n:\n        \
         unsafe:\n            asm(bx=i, out=(cx=let got), clobbers=[]):\n                mov cx, bx\n            \
         total += got\n        i += 1\n    return total\n\nfn main() -> i16:\n    return i16(spin(3))\n";
    let assembly = rich(&directory, "spin.nib", source);
    let after: Vec<&str> = assembly.split("db 089h,0d9h\n").skip(1).map(|rest| rest.lines().next().unwrap_or("")).collect();
    assert_eq!(after.len(), 3, "{assembly}");
    assert!(after.iter().all(|line| line.trim_start().starts_with("mov ax, cx") || line.trim_start().starts_with("add ax, cx")), "{assembly}");
}

/// A raw pointer walk (#105): `p != e` of far pointers was refused "a ptr
/// addrspace(1) value", and a `&mut [T]` view had no raw address ("only a
/// scalar, struct, or sequence has a raw address") though a `&[T]` did.
#[test]
fn test_a_raw_pointer_walks_an_array_and_a_mutable_view() {
    let directory = tempfile::tempdir().expect("a directory");
    let walk = "var g: i16[10] = [0] * 10\n\nfn total(n: i16) -> i32:\n    let mut t: i32 = 0\n    let mut p: *far mut i16 = 0\n    let mut e: *far mut i16 = 0\n    unsafe:\n        p = (&mut g)\n        e = p.offset(n)\n    while p != e:\n        unsafe:\n            t += i32(p[0])\n            p = p.offset(1)\n    return t\n\nfn main() -> i16:\n    return i16(total(3))\n";
    let view = "fn fill(a: &mut [i16]) -> void:\n    let mut p: *far mut i16 = 0\n    unsafe:\n        p = &mut a\n    let mut i: u16 = 0\n    while i < a.len:\n        unsafe:\n            p[0] = i16(i)\n            p = p.offset(1)\n        i += 1\n\nfn main() -> i16:\n    let mut g: i16[3] = [0, 0, 0]\n    fill(&mut g)\n    return g[2]\n";
    assert!(walk.contains("while p != e") && view.contains("p = &mut a"), "the shapes that were refused");
    for (name, source) in [("walk.nib", walk), ("view.nib", view)] {
        rich(&directory, name, source);
    }
    let immutable = view.replace("a: &mut [i16]", "a: &[i16]").replace("fill(&mut g)", "fill(&g)");
    let refused = driver::parsed(&written(&directory, "immutable.nib", &immutable), &crate::real_mode(), None);
    assert!(refused.is_err(), "a raw &mut of a &[T] view is still refused");
}

#[test]
fn test_an_export_no_object_uses_is_dropped_with_what_only_it_calls() {
    // jwlink's `option eliminate` keeps a segment any other references, even
    // one it drops: every program carried the unused float printer, 2.8 KB.
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(&directory, "lib.nib", "fn helper(x: u16) -> u16:\n    let mut total: u16 = 0\n    for i in 0..x:\n        total += i * x\n    return total\n\n@export(\"cdecl16\", name=\"N$ZA\")\nfn a(x: u16) -> u16:\n    return helper(x) + 1\n\n@export(\"cdecl16\", name=\"N$ZB\")\nfn b(x: u16) -> u16:\n    return x * 2\n");
    let mut program = parsed(&source);
    nib_compile::keep_exports(&mut program, &["N$ZB".to_owned()].into_iter().collect());
    let module = nib_compile::assembled(&program, "main", &level("O2"), &crate::real_mode().os).expect("assembles");
    assert_eq!(module.publics, ["N$ZB"]);
    assert_eq!(module.procedures.len(), 1, "{:?}", module.procedures.iter().map(|one| &one.name).collect::<Vec<_>>());
}

#[test]
fn test_an_error_in_an_imported_module_names_that_module() {
    // Semantic errors carried no module: one in shapes.nib was reported at main.nib's line.
    let directory = tempfile::tempdir().expect("a directory");
    written(&directory, "shapes.nib", "pub fn area(w: i16, h: u16) -> i16:\n    return w * h\n");
    let main = written(&directory, "main.nib", "import shapes\n\nfn main() -> i16:\n    return shapes.area(2, 3)\n");
    let (path, error) = super::compile_file(&main, &crate::real_mode()).expect_err("refused");
    assert!(path.ends_with("shapes.nib"), "{} {}", path.display(), error.message);
    assert_eq!(error.span.line, 2);
}

#[test]
fn test_a_borrowed_fixed_array_is_a_far_pointer_with_no_descriptor() {
    // `&i16[4]` was passed as a view: a descriptor pointer, its length loaded at run time.
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "fixed_borrow.nib",
        "fn last(values: &i16[4]) -> i16:\n    return values[values.len - 1]\n\nfn main() -> i16:\n    let v: i16[4] = [1, 2, 3, 4]\n    return last(&v)\n",
    );
    let program = parsed(&source);
    let module = &program.modules[0];
    let function = function(&program, "last");
    let by_id = |id: i64| module.types.iter().find(|one| one.id == id).unwrap();
    let pointer = by_id(function.values[0].r#type);
    let target = by_id(pointer.element.unwrap());

    assert_eq!((pointer.kind, pointer.width), (model::TypeKind::Pointer, 4));
    assert_eq!((target.kind, target.rank, target.width), (model::TypeKind::Array, 1, 8));
    assert!(!instructions(function).any(|one| one.operands.iter().any(|operand| matches!(operand, model::Operand::DescriptorPlace(_)))));
}

/// An `@export` without an ABI is Nib's own convention, as the runtime's
/// routines are: a `&string` crosses it, where cdecl16 refuses one.
#[test]
fn test_an_export_without_an_abi_takes_what_a_nib_function_takes() {
    let directory = tempfile::tempdir().expect("a directory");
    let text = |decorator: &str| format!("{decorator}\nfn size(text: &string) -> u16:\n    return text.len\n\nfn main() -> i16:\n    return 0\n");
    let foreign = written(&directory, "foreign.nib", &text("@export(\"cdecl16\")"));
    assert!(refused(&foreign).contains("cannot cross a foreign ABI"), "{}", refused(&foreign));
    let native = written(&directory, "native.nib", &text("@export(name=\"N$SIZE\")"));
    driver::parsed(&native, &crate::real_mode(), None).expect("a native export takes a view");
}

/// Nib through the rich MIR: emitted, selected and assembled whole, the
/// entry public and each function by its object name. `twice`, folded into
/// its one call, was still assembled until GlobalDCE dropped what no root
/// reaches, as the old route does.
#[test]
fn test_a_program_compiles_through_the_rich_mir() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(&directory, "twice.nib", "fn twice(x: i16) -> i16:\n    return x + x\n\nfn main() -> i16:\n    return twice(21)\n");
    let module = nib_compile::assembled(&parsed(&source), "main", &llrm_driver::m16_options(nib_compile::machine(&llrm_x86_m16::M16, &crate::real_mode().os)), &crate::real_mode().os).expect("assembles");
    let text = masm::text(&module).expect("prints");
    let lines: Vec<&str> = text.lines().map(str::trim).filter(|line| !line.is_empty()).collect();
    assert_eq!(
        lines,
        [
            ".model medium", ".386", "public _main", ".data", ".code TWICE_TEXT",
            "_main proc far", "L0_0:", "mov ax, 42", "retf", "_main endp",
            "end",
        ]
    );
}

/// The rich route priced every CPU as a 486, so a 386's dearer far call
/// did not make evaluating `logic`'s calls pay. The body is one too dear to
/// evaluate on a 486 after a complemented boolean folds to one compare.
#[test]
fn test_the_rich_route_prices_the_configured_cpu() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(&directory, "logic.nib", "fn logic(a: i16, b: i16) -> bool:\n    return (a < b && a * 3 + b == 0) || (b == 7 && a + b * 5 == 2)\n\nfn main() -> i16:\n    print(f\"{i16(logic(1, 2))} {i16(logic(0, 2))} {i16(logic(3, 7))} {i16(logic(3, 2))}\")\n    return 0\n");
    let calls = |cpu: &'static str| {
        let module = nib_compile::assembled(&parsed(&source), "main", &llrm_driver::m16_options(llrm_core::abi::machine::Machine { cpu: cpu.to_owned(), ..nib_compile::machine(&llrm_x86_m16::M16, &crate::real_mode().os) }), &crate::real_mode().os).expect("assembles");
        masm::text(&module).expect("prints").lines().filter(|line| line.contains("call") && line.contains("_logic")).count()
    };
    assert_eq!((calls("486"), calls("386")), (4, 0));
}

/// runtime.nib's floats.assign: affine rewrote `at * 2 + copy(a)`, its own
/// form, into a fresh copy each round, strength folded it back, and the
/// runtime did not compile: "MIR optimization did not converge".
#[test]
fn test_a_loop_through_a_copied_pointer_converges() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "assign.nib",
        "const LIMBS = 72\n\npub fn assign(a: *near mut u16, value: u16) -> void:\n    unsafe:\n        for at in 0..LIMBS:\n            a[at] = 0\n        a[0] = value\n",
    );
    nib_compile::assembled(&parsed(&source), "assign", &level("O2"), &crate::real_mode().os).expect("assembles");
}

/// A borrowed view's descriptor is the caller's, never written in the
/// call: stated as LLVM's `noalias readonly dereferenceable`, for LICM to
/// hoist its loads, and `nocapture`, as `multiply` keeps none of them.
#[test]
fn test_a_borrowed_view_states_facts_of_its_descriptor() {
    use llrm_mir::facts::Fact;
    let program = parsed(&fixture("matmul8.nib"));
    let multiply = function(&program, "multiply");
    let stated = |index: usize| -> Vec<Fact> {
        program.modules[0]
            .facts
            .iter()
            .filter(|one| matches!(one.subject, llrm_core::hir::facts::Subject::Param { function, index: at } if function == multiply.id && at == index as i64))
            .map(|one| one.fact)
            .collect()
    };
    for index in 0..multiply.parameters.len() {
        assert_eq!(stated(index), vec![Fact::NoAlias, Fact::ReadOnly, Fact::Dereferenceable(10), Fact::NoCapture], "parameter {index}");
    }
}

/// A reference is not null and points at all it borrows; a shared one is
/// read only; `bump` keeps neither beyond its call. Nothing else reaches
/// them (the checker refuses a lend `bump` could write), so `noalias`.
#[test]
fn test_a_reference_states_what_the_language_guarantees_and_no_more() {
    use llrm_mir::facts::Fact;
    let source = "struct Pt:\n    mut x: i16\n    y: i16\n\nvar g: Pt = Pt(x=1, y=2)\nvar h: Pt = Pt(x=3, y=4)\n\nfn bump(p: &mut Pt, q: &Pt) -> void:\n    p.x += q.y\n\nfn main() -> i16:\n    bump(g, h)\n    return g.x\n";
    let directory = tempfile::tempdir().unwrap();
    let program = parsed(&written(&directory, "refs.nib", source));
    let bump = function(&program, "bump");
    let stated = |index: i64| -> Vec<Fact> {
        program.modules[0]
            .facts
            .iter()
            .filter(|one| matches!(one.subject, llrm_core::hir::facts::Subject::Param { function, index: at } if function == bump.id && at == index))
            .map(|one| one.fact)
            .collect()
    };
    assert_eq!(stated(0), vec![Fact::NonNull, Fact::Dereferenceable(4), Fact::NoAlias, Fact::NoCapture]);
    assert_eq!(stated(1), vec![Fact::NonNull, Fact::Dereferenceable(4), Fact::ReadOnly, Fact::NoAlias, Fact::NoCapture]);
}

/// `for i in 0..n` adds one to a counter that is below `n`: it cannot wrap,
/// signed or unsigned. Stated, `nsw` or `nuw` on that add is what makes a
/// variable bound a counted loop; unstated, the trip count was unknown.
#[test]
fn test_a_range_loops_counter_does_not_wrap() {
    use llrm_mir::facts::Fact;
    let directory = tempfile::tempdir().unwrap();
    let stated = |type_name: &str| -> Vec<Fact> {
        let source = format!("fn total(n: {type_name}) -> {type_name}:\n    let mut s: {type_name} = 0\n    for i in 0..n:\n        s += i\n    return s\n\nfn main() -> i16:\n    return 0\n");
        let program = parsed(&written(&directory, &format!("{type_name}.nib"), &source));
        let total = function(&program, "total");
        program.modules[0]
            .facts
            .iter()
            .filter(|one| matches!(one.subject, llrm_core::hir::facts::Subject::Instruction { function, .. } if function == total.id))
            .map(|one| one.fact)
            .collect()
    };
    assert_eq!(stated("i16"), vec![Fact::NoSignedWrap]);
    assert_eq!(stated("u16"), vec![Fact::NoUnsignedWrap]);
    // It is the counter's own add of one, not the body's `s += i`.
    let source = "fn total(n: i16) -> i16:\n    let mut s: i16 = 0\n    for i in 0..n:\n        s += i\n    return s\n\nfn main() -> i16:\n    return 0\n";
    let program = parsed(&written(&directory, "which.nib", source));
    let total = function(&program, "total");
    let ids: Vec<i64> = program.modules[0].facts.iter().filter_map(|one| match one.subject {
        llrm_core::hir::facts::Subject::Instruction { function, id } if function == total.id => Some(id),
        _ => None,
    }).collect();
    assert_eq!(ids.len(), 1);
    let stated = total.blocks.iter().flat_map(|block| &block.instructions).find(|one| one.id == ids[0]).expect("the instruction");
    assert_eq!(stated.op, llrm_core::hir::model::Op::Add);
    assert!(matches!(stated.operands[1], llrm_core::hir::model::Operand::Constant(_)), "{:?}", stated.operands);
}

/// A shared reference is not null and points at all of its struct, so the
/// load of its field is hoisted above the loop's guard and no register is
/// saved to hold it: 15 instructions where 17 saved and restored `si`.
#[test]
fn test_a_reference_lets_its_field_load_leave_the_loop() {
    let directory = tempfile::tempdir().unwrap();
    let source = "struct V:\n    mut a: i16\n    b: i16\n\nfn sum(v: &V, n: i16) -> i16:\n    let mut s: i16 = 0\n    for i in 0..n:\n        s += v.b\n    return s\n\nfn main() -> i16:\n    return 0\n";
    let program = parsed(&written(&directory, "refsum.nib", source));
    let module = nib_compile::assembled(&program, "sum", &llrm_driver::m16_options(nib_compile::machine(&llrm_x86_m16::M16, &crate::real_mode().os)), &crate::real_mode().os).expect("assembles");
    let asm = masm::text(&module).expect("prints");
    let from = asm.find("_sum proc").expect("the function");
    let body: Vec<&str> = asm[from..].lines().skip(1).map(str::trim).take_while(|one| !one.ends_with("endp")).filter(|one| !one.ends_with(':')).collect();
    assert_eq!(body.len(), 15, "{body:?}");
    assert!(!body.contains(&"push si"), "{body:?}");
}

/// A loop whose body always returns leaves its counter's increment
/// unreachable; the fact stated of it named an instruction the function no
/// longer had, and the program was refused as invalid HIR.
#[test]
fn test_a_fact_of_a_pruned_instruction_goes_with_it() {
    let directory = tempfile::tempdir().unwrap();
    let source = "fn first(n: i16) -> i16:\n    for i in 0..n:\n        return i\n    return -1\n\nfn main() -> i16:\n    return 0\n";
    let program = parsed(&written(&directory, "first.nib", source));
    let first = function(&program, "first");
    assert!(program.modules[0].facts.iter().all(|one| !matches!(one.subject, llrm_core::hir::facts::Subject::Instruction { function, .. } if function == first.id)));
}

/// The rich route ran -O2 whatever `-O` said: `-Os` copied dice's loops as
/// -O2 does, an object as large.
#[test]
fn test_the_level_reaches_the_rich_route() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = root().join("examples/dice.nib");
    let object = |level: &str| {
        let output = directory.path().join(format!("dice{level}.obj"));
        let argv = [source.display().to_string(), level.to_owned(), "-o".to_owned(), output.display().to_string()];
        assert_eq!(crate::cli::main(&argv), 0);
        std::fs::read(output).expect("the object").len()
    };
    assert!(object("-Os") < object("-O2"));
}

/// `dst: &mut P` and `src: &P` are stated noalias, so `src.y` is loaded
/// once, before the loop. Unstated, the loop reloaded it after every store
/// to `dst.x`.
#[test]
fn test_a_noalias_parameter_keeps_its_loads_out_of_a_loop_that_stores_another() {
    use llrm_mir::facts::Fact;
    let source = "struct P:\n    mut x: i16\n    y: i16\n\nfn bump(dst: &mut P, src: &P, n: i16) -> void:\n    for i in 0..n:\n        dst.x += src.y\n\nfn main() -> i16:\n    let mut a = P(x=0, y=0)\n    let b = P(x=0, y=3)\n    bump(a, b, 4)\n    return a.x\n";
    let directory = tempfile::tempdir().unwrap();
    let program = parsed(&written(&directory, "bump.nib", source));
    let bump = function(&program, "bump");
    let unaliased = |index: i64| program.modules[0].facts.iter().any(|one| one.fact == Fact::NoAlias && matches!(one.subject, llrm_core::hir::facts::Subject::Param { function, index: at } if function == bump.id && at == index));
    assert!(unaliased(0) && unaliased(1), "the premise: both are stated noalias");
    // The memory operands of `bump`'s loop: from the label its backward
    // jump names to that jump.
    let looped = |assembly: &str| -> usize {
        let body = between(assembly, "_bump proc near\n", "_bump endp");
        let jump = Regex::new(r"\n    j\w+ (L\d+_\d+)\n").unwrap();
        let (head, end) = jump
            .captures_iter(body)
            .map(|one| (one[1].to_owned(), one.get(0).unwrap().start()))
            .find(|(label, at)| body[..*at].contains(&format!("{label}:\n")))
            .expect("a loop");
        between(&body[..end], &format!("{head}:\n"), "\0").matches("ptr").count()
    };
    // Unrolled, the 4-trip loop is gone and there is nothing to count.
    let pipeline = llrm_transforms::pipeline::Options { inline: llrm_transforms::inline::Threshold::new(0), unroll: false, peel: false, ..Default::default() };
    let options = llrm_core::driver::Options { pipeline, ..llrm_driver::m16_options(nib_compile::machine(&llrm_x86_m16::M16, &crate::real_mode().os)) };
    let module = nib_compile::assembled(&program, "main", &options, &crate::real_mode().os).expect("assembles");
    assert_eq!(looped(&masm::text(&module).expect("prints")), 1);
}

/// examples/league.nib's `main` was refused after #127 made every spiller
/// product unspillable: "value#23 cannot be spilled and no register is free".
#[test]
fn test_league_compiles_when_a_long_spiller_product_must_be_spilled() {
    let program = parsed(&fixture("league.nib"));
    let result = nib_compile::assembled(&program, "main", &Options { machine: llrm_core::abi::machine::Machine { cpu: "386".to_owned(), ..nib_compile::machine(&llrm_x86_m16::M16, &crate::real_mode().os) }, ..level("O2") }, &crate::real_mode().os);
    assert!(result.is_ok(), "{:?}", result.err());
}

/// MIR infers each defined function's memory effects, per location, from
/// its body and what its callees are stated to do (#113: Nib states effects
/// only where no body shows them, the runtime's routines): a function with
/// no `&mut` and no module-variable write reads memory and writes none, one
/// that writes through a `&mut` writes only through it, and one of scalars
/// touches none.
#[test]
fn test_mir_infers_what_a_nib_function_touches() {
    use llrm_mir::{GlobalKind, Attribute};
    let source = "@extern(\"cdecl16\")\nfn keep(x: i16) -> i16\n\nvar g: i16 = 3\n\nfn scalars(a: i16, b: i16) -> i16:\n    return a * b\n\nfn reads(p: &i16) -> i16:\n    return p + g\n\nfn writes(p: &mut i16) -> void:\n    p = 1\n\nfn main() -> i16:\n    unsafe:\n        let mut x: i16 = keep(2)\n        writes(x)\n        print(scalars(keep(x), reads(x)))\n    return 0\n";
    let directory = tempfile::tempdir().unwrap();
    let mut program = parsed(&written(&directory, "effects.nib", source));
    // As the compile does: each function by its name, `main` the entry that keeps the rest alive.
    for function in &mut program.modules[0].functions {
        function.symbol = Some(function.name.clone());
        if function.name == "main" {
            function.linkage = llrm_core::hir::model::FunctionLinkage::External;
        }
    }
    let pipeline = llrm_transforms::pipeline::Options { inline: llrm_transforms::inline::Threshold::new(0), ..Default::default() };
    let options = llrm_core::driver::Options { pipeline, ..llrm_driver::m16_options(nib_compile::machine(&llrm_x86_m16::M16, &crate::real_mode().os)) };
    let (mut mir, _) = llrm_core::driver::emitted(&program, &options).expect("emits");
    llrm_core::driver::optimized(&mut mir, &options).expect("optimizes");
    let module = &mir.modules[0];
    let memory = |name: &str| -> Vec<(Option<String>, String)> {
        let GlobalKind::Function(function) = &module.global(module.named(name).unwrap_or_else(|| panic!("{name} among {:?}", module.globals.iter().filter_map(|one| one.name.clone()).collect::<Vec<_>>()))).kind else { panic!("{name} is no function") };
        function.attrs.iter().find_map(|one| if let Attribute::Memory(locations) = one { Some(locations.clone()) } else { None }).unwrap_or_default()
    };
    assert_eq!(memory("scalars"), [(None, "none".to_owned())]);
    assert!(memory("reads").iter().all(|(_, access)| access != "write" && access != "readwrite"), "{:?}", memory("reads"));
    assert_eq!(memory("writes"), [(Some("argmem".to_owned()), "write".to_owned())]);
}

/// A range loop's counter cannot wrap (`nsw`), so its trip count is `n` and
/// the loop counts down to zero, testing the flags `dec` leaves: no `cmp`
/// in the loop.
#[test]
fn test_a_range_loop_with_a_variable_bound_counts_to_zero() {
    let source = "fn total(values: &[i16], n: i16) -> i16:\n    let mut s: i16 = 0\n    for i in 0..n:\n        s += values[0]\n    return s\n\nfn main() -> i16:\n    let a: i16[2] = [1, 2]\n    print(total(a, 5))\n    return 0\n";
    let directory = tempfile::tempdir().unwrap();
    let program = parsed(&written(&directory, "trip.nib", source));
    let pipeline = llrm_transforms::pipeline::Options { inline: llrm_transforms::inline::Threshold::new(0), ..Default::default() };
    let options = llrm_core::driver::Options { pipeline, ..llrm_driver::m16_options(nib_compile::machine(&llrm_x86_m16::M16, &crate::real_mode().os)) };
    let module = nib_compile::assembled(&program, "main", &options, &crate::real_mode().os).expect("assembles");
    let assembly = masm::text(&module).expect("prints");
    let body = between(&assembly, "_total proc near\n", "_total endp");
    // The loop: from the label its backward jump names to that jump.
    let jump = Regex::new(r"\n    j\w+ (L\d+_\d+)\n").unwrap();
    let (head, end) = jump
        .captures_iter(body)
        .map(|one| (one[1].to_owned(), one.get(0).unwrap().start()))
        .find(|(label, at)| body[..*at].contains(&format!("{label}:\n")))
        .expect("a loop");
    let looped = between(&body[..end], &format!("{head}:\n"), "\0");
    assert!(!looped.contains("cmp"), "{looped}");
}

/// Nib frames are not zeroed, but the program claimed they were: the MIR
/// stored zero into every local at entry (`mov dword ptr [bp-4], 0` before
/// the struct's own stores) and left each to dead-store elimination, which
/// -O0 does not run.
#[test]
fn test_a_nib_program_does_not_claim_zeroed_frames() {
    let source = "struct P:\n    mut x: i16\n    mut y: i16\n\nfn f(n: i16) -> i16:\n    let mut p = P(x=n, y=2)\n    p.x += 1\n    return p.x + p.y\n\nfn main() -> i16:\n    print(f(1))\n    return 0\n";
    let directory = tempfile::tempdir().unwrap();
    let program = parsed(&written(&directory, "zeroed.nib", source));
    assert!(!program.zeroed_locals, "the premise: the program says its frames are not zeroed");
    let pipeline = llrm_transforms::pipeline::Options { optimize: false, ..Default::default() };
    let options = llrm_core::driver::Options { pipeline, ..llrm_driver::m16_options(nib_compile::machine(&llrm_x86_m16::M16, &crate::real_mode().os)) };
    let module = nib_compile::assembled(&program, "main", &options, &crate::real_mode().os).expect("assembles");
    let assembly = masm::text(&module).expect("prints");
    let body = between(&assembly, "_f proc near\n", "_f endp");
    assert!(!body.contains(", 0\n"), "{body}");
}

/// Nib's `true` is one, as C's. It was -1, so a bool C handed over (1)
/// was not equal to a Nib `true`: `b == t` compared the bytes. Run on the
/// HIR executor and on the optimized MIR at -O0 and -O2, each a bit of
/// the score.
#[test]
fn test_a_bool_is_equal_to_any_other_true_whoever_stored_it() {
    use llrm_mir::interpret::{self, Val};
    let source = "\
struct S:
    mut flag: bool

fn main() -> i16:
    let mut b: bool = false
    let t: bool = true
    let mut s = S(flag=false)
    unsafe:
        let p: *far mut bool = &mut b
        let r = p.cast[u8]()
        r[0] = 1
        let q: *far mut bool = &mut s.flag
        let u = q.cast[u8]()
        u[0] = 1
    let c: bool = 3 > 2
    let n: bool = !b
    let mut score: i16 = 0
    if b == t:
        score += 1
    if b == true:
        score += 2
    if b != false:
        score += 4
    if s.flag == t:
        score += 8
    if b == c:
        score += 16
    if n == false:
        score += 32
    if !n == b:
        score += 64
    return score
";
    let directory = tempfile::tempdir().unwrap();
    let mut program = parsed(&written(&directory, "booleans.nib", source));
    let held = llrm_core::hir::execute::run(&program, "main", &[]).expect("runs").value;
    assert_eq!(held, Some(llrm_core::hir::model::Number::Int(127)), "the HIR executor");
    program.modules[0].functions.iter_mut().for_each(|function| function.symbol = Some(function.name.clone()));
    program.modules[0].functions.iter_mut().find(|function| function.name == "main").unwrap().linkage = llrm_core::hir::model::FunctionLinkage::External;
    for optimize in [false, true] {
        let pipeline = llrm_transforms::pipeline::Options { optimize, ..Default::default() };
        let options = llrm_core::driver::Options { pipeline, ..llrm_driver::m16_options(nib_compile::machine(&llrm_x86_m16::M16, &crate::real_mode().os)) };
        let (mut mir, _) = llrm_core::driver::emitted(&program, &options).expect("emits");
        llrm_core::driver::optimized(&mut mir, &options).expect("optimizes");
        let score = interpret::run(&mir.modules[0], "main", vec![], 1_000_000).unwrap_or_else(|trap| panic!("{trap:?}"));
        assert!(matches!(score, Val::Int { bits: 127, .. }), "optimize {optimize}: {score:?}");
    }
}

/// A view's descriptor holds the data's pointer. The optimizer took a call
/// of `first_even(values)` to read only the descriptor, and dropped the
/// array's stores before it: the digits example printed "first even 0" on
/// DOS at -O1 and -O2 (the callee reads the array through the pointer the
/// descriptor holds, which `nocapture` says nothing of).
#[test]
fn test_a_call_reads_the_array_behind_the_view_it_is_given() {
    use llrm_mir::interpret::{self, Val};
    let source = "\
fn first_even(values: &[i16]) -> i16:
    for value in values:
        if value % 2 == 0:
            return value
    return 0

fn main() -> i16:
    let values: i16[4] = [3, 7, 8, 9]
    return first_even(values)
";
    let directory = tempfile::tempdir().unwrap();
    let mut program = parsed(&written(&directory, "views.nib", source));
    program.modules[0].functions.iter_mut().for_each(|function| function.symbol = Some(function.name.clone()));
    program.modules[0].functions.iter_mut().find(|function| function.name == "main").unwrap().linkage = llrm_core::hir::model::FunctionLinkage::External;
    let pipeline = llrm_transforms::pipeline::Options { inline: llrm_transforms::inline::Threshold::new(0), ..Default::default() };
    let options = llrm_core::driver::Options { pipeline, ..llrm_driver::m16_options(nib_compile::machine(&llrm_x86_m16::M16, &crate::real_mode().os)) };
    let (mut mir, _) = llrm_core::driver::emitted(&program, &options).expect("emits");
    llrm_core::driver::optimized(&mut mir, &options).expect("optimizes");
    let result = interpret::run(&mir.modules[0], "main", vec![], 1_000_000);
    assert!(matches!(result, Ok(Val::Int { bits: 8, .. })), "{result:?}");
}

/// A payload-less variant stored only its tag, so the other bytes of the
/// enum were undefined where the whole value then flowed as one integer:
/// returned, compared, copied into a struct compared bytewise. The MIR
/// interpreter reads an unwritten byte as poison, which a copy of the whole
/// value carries to the tag read after it.
#[test]
fn test_an_enum_value_is_written_whole() {
    use llrm_mir::interpret::{self, Val};
    let source = "\
fn first_even(a: i16, b: i16) -> Option[i16]:
    if a % 2 == 0:
        return .some(a)
    if b % 2 == 0:
        return .some(b)
    return .none

fn main() -> i16:
    let x = first_even(1, 3)
    let y = x
    match y:
        .some(n):
            return n
        .none:
            return 7
";
    let directory = tempfile::tempdir().unwrap();
    let mut program = parsed(&written(&directory, "enum.nib", source));
    program.modules[0].functions.iter_mut().for_each(|function| function.symbol = Some(function.name.clone()));
    program.modules[0].functions.iter_mut().find(|function| function.name == "main").unwrap().linkage = llrm_core::hir::model::FunctionLinkage::External;
    let pipeline = llrm_transforms::pipeline::Options { optimize: false, ..Default::default() };
    let options = llrm_core::driver::Options { pipeline, ..llrm_driver::m16_options(nib_compile::machine(&llrm_x86_m16::M16, &crate::real_mode().os)) };
    let (mut mir, _) = llrm_core::driver::emitted(&program, &options).expect("emits");
    llrm_core::driver::optimized(&mut mir, &options).expect("optimizes");
    let result = interpret::run(&mir.modules[0], "main", vec![], 1_000_000);
    assert!(matches!(result, Ok(Val::Int { bits: 7, .. })), "{result:?}");
}

/// An enum too large for a register moves by memory copy and a match reads
/// the tag first, so its `.none` leaves the payload bytes unwritten: it
/// copies and matches right, and costs no zero-fill.
#[test]
fn test_a_large_enum_value_copied_and_matched_stays_correct() {
    use llrm_mir::interpret::{self, Val};
    let source = "\
enum Box:
    empty
    full(a: i16, b: i16, c: i16)

fn make(n: i16) -> Box:
    if n > 0:
        return .full(n, n, n)
    return .empty

fn main() -> i16:
    let x = make(0)
    let y = x
    match y:
        .full(a, b, c):
            return a + b + c
        .empty:
            return 7
";
    let directory = tempfile::tempdir().unwrap();
    let mut program = parsed(&written(&directory, "large.nib", source));
    program.modules[0].functions.iter_mut().for_each(|function| function.symbol = Some(function.name.clone()));
    program.modules[0].functions.iter_mut().find(|function| function.name == "main").unwrap().linkage = llrm_core::hir::model::FunctionLinkage::External;
    let pipeline = llrm_transforms::pipeline::Options { optimize: false, ..Default::default() };
    let options = llrm_core::driver::Options { pipeline, ..llrm_driver::m16_options(nib_compile::machine(&llrm_x86_m16::M16, &crate::real_mode().os)) };
    let (mut mir, _) = llrm_core::driver::emitted(&program, &options).expect("emits");
    llrm_core::driver::optimized(&mut mir, &options).expect("optimizes");
    let result = interpret::run(&mir.modules[0], "main", vec![], 1_000_000);
    assert!(matches!(result, Ok(Val::Int { bits: 7, .. })), "{result:?}");
}

/// `small`'s byte sits at offset 2 and byte 3 is never written, yet an enum
/// is copied as raw i16 words: the word at offset 2 is poison, and so is the
/// payload read from the copy.
#[test]
fn test_a_small_payload_of_a_large_enum_survives_a_copy() {
    use llrm_mir::interpret::{self, Val};
    let source = "\
enum E:
    small(a: u8)
    big(a: i16, b: i16, c: i16)

fn make() -> E:
    return .small(7)

fn main() -> i16:
    let x = make()
    let y = x
    match y:
        .small(a):
            if a == 7:
                return 7
            return 1
        .big(a, b, c):
            return 2
";
    let directory = tempfile::tempdir().unwrap();
    let mut program = parsed(&written(&directory, "small.nib", source));
    program.modules[0].functions.iter_mut().for_each(|function| function.symbol = Some(function.name.clone()));
    program.modules[0].functions.iter_mut().find(|function| function.name == "main").unwrap().linkage = llrm_core::hir::model::FunctionLinkage::External;
    let pipeline = llrm_transforms::pipeline::Options { optimize: false, ..Default::default() };
    let options = llrm_core::driver::Options { pipeline, ..llrm_driver::m16_options(nib_compile::machine(&llrm_x86_m16::M16, &crate::real_mode().os)) };
    let (mut mir, _) = llrm_core::driver::emitted(&program, &options).expect("emits");
    llrm_core::driver::optimized(&mut mir, &options).expect("optimizes");
    let result = interpret::run(&mir.modules[0], "main", vec![], 1_000_000);
    assert!(matches!(result, Ok(Val::Int { bits: 7, .. })), "{result:?}");
}

/// A struct copies its fields, and its enum field's bytes were two words of
/// a payload `small` never wrote: the copied tag read poison.
#[test]
fn test_a_struct_holding_an_enum_copies_it_byte_for_byte() {
    use llrm_mir::interpret::{self, Val};
    let source = "\
enum E:
    small(a: u8)
    big(a: i16, b: i16, c: i16)

struct Holder:
    tag: u8
    value: E

fn make() -> Holder:
    return Holder(tag=1, value=.small(7))

fn main() -> i16:
    let x = make()
    let y = x
    match y.value:
        .small(a):
            if a == 7:
                return 7
            return 1
        .big(a, b, c):
            return 2
";
    let directory = tempfile::tempdir().unwrap();
    let mut program = parsed(&written(&directory, "test_a_struct_holding_an_enum_copies_it_byte_for_byte.nib", source));
    program.modules[0].functions.iter_mut().for_each(|function| function.symbol = Some(function.name.clone()));
    program.modules[0].functions.iter_mut().find(|function| function.name == "main").unwrap().linkage = llrm_core::hir::model::FunctionLinkage::External;
    let pipeline = llrm_transforms::pipeline::Options { optimize: false, ..Default::default() };
    let options = llrm_core::driver::Options { pipeline, ..llrm_driver::m16_options(nib_compile::machine(&llrm_x86_m16::M16, &crate::real_mode().os)) };
    let (mut mir, _) = llrm_core::driver::emitted(&program, &options).expect("emits");
    llrm_core::driver::optimized(&mut mir, &options).expect("optimizes");
    let result = interpret::run(&mir.modules[0], "main", vec![], 1_000_000);
    assert!(matches!(result, Ok(Val::Int { bits: 7, .. })), "{result:?}");
}

/// An enum of 40 bytes was copied as words past the byte-copy limit, and its
/// `small` variant's byte shared a word with one never written.
#[test]
fn test_an_enum_past_the_unrolled_copy_keeps_its_small_payload() {
    use llrm_mir::interpret::{self, Val};
    let source = "\
enum E:
    small(a: u8)
    big(a: i16, b: i16, c: i16, d: i16, e: i16, f: i16, g: i16, h: i16, i: i16, j: i16, k: i16, l: i16, m: i16, n: i16, o: i16, p: i16, q: i16, r: i16, s: i16)

fn make() -> E:
    return .small(7)

fn main() -> i16:
    let x = make()
    let y = x
    match y:
        .small(a):
            if a == 7:
                return 7
            return 1
        .big(a, b, c, d, e, f, g, h, i, j, k, l, m, n, o, p, q, r, s):
            return 2
";
    let directory = tempfile::tempdir().unwrap();
    let mut program = parsed(&written(&directory, "test_an_enum_past_the_unrolled_copy_keeps_its_small_payload.nib", source));
    program.modules[0].functions.iter_mut().for_each(|function| function.symbol = Some(function.name.clone()));
    program.modules[0].functions.iter_mut().find(|function| function.name == "main").unwrap().linkage = llrm_core::hir::model::FunctionLinkage::External;
    let pipeline = llrm_transforms::pipeline::Options { optimize: false, ..Default::default() };
    let options = llrm_core::driver::Options { pipeline, ..llrm_driver::m16_options(nib_compile::machine(&llrm_x86_m16::M16, &crate::real_mode().os)) };
    let (mut mir, _) = llrm_core::driver::emitted(&program, &options).expect("emits");
    llrm_core::driver::optimized(&mut mir, &options).expect("optimizes");
    let result = interpret::run(&mir.modules[0], "main", vec![], 1_000_000);
    assert!(matches!(result, Ok(Val::Int { bits: 7, .. })), "{result:?}");
}

/// A far pointer is its segment and offset words: a store through
/// `screen[2]`, `screen` from the literal 0xB8000000, writes B800:0002. The
/// MIR interpreter models no video memory, so it stops at the access and
/// names its address. The literal was refused: "expected *far pointer, found i16".
#[test]
fn test_a_far_pointer_literal_stores_at_its_segment_and_offset() {
    use llrm_mir::interpret::{self, Trap};
    let source = "fn poke() -> void:\n    unsafe:\n        let screen: *far mut u8 = 0xB8000000\n        screen[2] = 7\n";
    let directory = tempfile::tempdir().unwrap();
    let mut program = parsed(&written(&directory, "poke.nib", source));
    program.modules[0].functions.iter_mut().for_each(|function| function.symbol = Some(function.name.clone()));
    program.modules[0].functions[0].linkage = llrm_core::hir::model::FunctionLinkage::External;
    let options = llrm_driver::m16_options(nib_compile::machine(&llrm_x86_m16::M16, &crate::real_mode().os));
    let (mut mir, _) = llrm_core::driver::emitted(&program, &options).expect("emits");
    llrm_core::driver::optimized(&mut mir, &options).expect("optimizes");
    let result = interpret::run(&mir.modules[0], "poke", vec![], 1_000);
    assert_eq!(result, Err(Trap::Undefined("an access of 1 bytes at 0xb8000002, outside every object".to_owned())));
}

/// TEXTFILL in Nib compiles at -O2: its screen is the literal 0xB8000000.
#[test]
fn test_textfill_compiles_at_o2() {
    let program = parsed(&fixture("textfill.nib"));
    for entry in ["fill", "checksum"] {
        let text = listing(&program, entry, &level("O2"));
        assert!(text.contains("byte ptr es:["), "{text}");
    }
}

/// A `huge var` array past 64K is C's `__huge` global: one far object, each
/// element reached through a huge pointer at an i32 offset. A module array
/// had no way past 64K (#362).
#[test]
fn test_a_huge_module_array_is_a_far_object_indexed_through_a_huge_pointer() {
    let source = "huge var a: i32[30000] = [0] * 30000\n\nfn fill() -> void:\n    for i in 0..30000:\n        a[i] = i32(i) + 5\n";
    let directory = tempfile::tempdir().unwrap();
    let text = emitted_text(&parsed(&written(&directory, "long1d.nib", source)));
    assert!(text.contains("@$var_a = internal addrspace(1) global [120000 x i8] zeroinitializer"), "{text}");
    let fill = defined(&text, "fill");
    let cast = Regex::new(r"(%\d+) = addrspacecast ptr addrspace\(1\) @\$var_a to ptr addrspace\(3\)").unwrap();
    let base = &cast.captures(fill).unwrap_or_else(|| panic!("{fill}"))[1];
    assert!(fill.contains(&format!("getelementptr inbounds i32, ptr addrspace(3) {base}, i32 %")), "{fill}");
}

/// A plain `var` past 64K does not fit DGROUP.
#[test]
fn test_a_module_array_past_64k_needs_huge() {
    let directory = tempfile::tempdir().unwrap();
    let error = refused(&written(&directory, "big.nib", "var a: i32[30000] = [0] * 30000\n"));
    assert!(error.contains("huge var"), "{error}");
}

/// The most a segment holds was the compiler's own 64K (65535 in the view check, a word's reach in
/// the static check): a target whose segments hold 32K got an array past it accepted.
#[test]
fn a_target_states_how_much_a_segment_holds() {
    let source = "var a: i16[20000] = [0] * 20000\n";
    let compile = |frontend: &crate::Frontend| crate::compile_module(crate::parse(crate::lex(source).unwrap()).unwrap(), "m", frontend);
    let mut frontend = crate::real_mode();
    assert!(compile(&frontend).is_ok());
    frontend.layout.spaces.roles.segment_bytes = Some(32768);
    let error = compile(&frontend).unwrap_err().to_string();
    assert!(error.contains("past DGROUP's 32768 bytes"), "{error}");
}

/// A view of a huge array was a far pointer, whose 16-bit offset wraps at
/// 64K: the callee read the wrong elements.
#[test]
fn test_a_huge_module_array_is_not_borrowed() {
    let source = "huge var a: i32[30000] = [0] * 30000\n\nfn first(xs: &[i32]) -> i32:\n    return xs[0]\n\nfn main() -> i16:\n    print(first(a))\n    return 0\n";
    let directory = tempfile::tempdir().unwrap();
    let error = refused(&written(&directory, "view.nib", source));
    assert!(error.contains("only indexed"), "{error}");
}

const RECURSIVE: &str = "fn down(n: i16) -> i16:\n    if n == 0:\n        return 0\n    return down(n - 1) + n\n\n@export(\"cdecl16\")\nfn up(n: i16) -> i16:\n    return down(n)\n\nfn main() -> i16:\n    print(up(3))\n    return 0\n";

#[test]
fn test_an_internal_function_only_called_directly_is_entered_by_a_near_call() {
    // Every Nib procedure was far: a 6-byte frame offset, retf, and a segment pushed per call.
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(&directory, "near.nib", RECURSIVE);
    let text = listing(&parsed(&source), "main", &level("O2"));
    assert!(text.contains("_down proc near"), "{text}");
    assert!(text.contains("_up proc far"), "{text}");
    assert!(!between(&text, "_down proc near", "_down endp").contains("retf"), "{text}");
}

#[test]
fn test_a_near_call_reaches_a_procedure_in_another_code_segment_of_the_object() {
    // With a segment per procedure, a near call between them was refused: "a near call to _down in another code segment".
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(&directory, "near.nib", RECURSIVE);
    object_of(&parsed(&source), "main", &source, &level("O2"), llrm_core::backend::objbuild::CodeLayout::PerProcedure).expect("writes");
}
