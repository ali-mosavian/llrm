//! Port of `tests/test_modern_frontend.py`.
//!
//! Sources are parsed by `QBOPT_MODERNFRONT` when set, else by `cargo run`,
//! exactly as the driver does.
//!
//! skipped: `execute.run` assertions (`qbopt/hir/execute.py` is tools-only),
//! and the tests made of nothing else; test_dos_bootstrap_enters_the_runtime_before_language_main
//! (reads runtime sources, no compiler).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use num_bigint::BigInt;
use regex::Regex;

use super::compile as modern_compile;
use super::driver;
use crate::analysis::{induction, loops};
use crate::backend::cpu::{self as targets, ProfileOrName};
use crate::backend::{lower_int64, masm};
use crate::frontends::qb::abi::physicalize;
use crate::hir::{self, model};
use crate::model::mir::{self, Arg, Kind};
use crate::model::passes::{LEVELS, O2, Options};
use crate::optimize::profit;
use crate::support::pyjson::{self, Json};

pub(crate) fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

pub(crate) fn fixture(name: &str) -> PathBuf {
    root().join("fixtures/modern").join(name)
}

/// `driver.parsed(source)`.
pub(crate) fn parsed(source: &Path) -> model::Program {
    driver::parsed(source, None).unwrap_or_else(|error| panic!("{}: {error}", source.display()))
}

/// The `FrontendError` `driver.parsed(source)` raises.
fn refused(source: &Path) -> String {
    driver::parsed(source, None).expect_err("the frontend refuses").0
}

/// `tmp_path / name` holding `text`.
fn written(directory: &tempfile::TempDir, name: &str, text: &str) -> PathBuf {
    let path = directory.path().join(name);
    std::fs::write(&path, text).expect("writes the source");
    path
}

/// `masm.text(modern_compile.assembled(program, entry=entry, options=options))`.
pub(crate) fn listing(program: &model::Program, entry: &str, options: &Options) -> String {
    listing_on(program, entry, options, "386")
}

/// `listing` with `cpu=cpu`.
fn listing_on(program: &model::Program, entry: &str, options: &Options, cpu: &'static str) -> String {
    let module = modern_compile::assembled(program, entry, ProfileOrName::Name(cpu), options).expect("assembles");
    masm::text(&module).expect("prints")
}

/// `text.split(start, 1)[1].split(end, 1)[0]`.
fn between<'t>(text: &'t str, start: &str, end: &str) -> &'t str {
    let after = text.split_once(start).unwrap_or_else(|| panic!("{start:?} not in listing")).1;
    after.split_once(end).map_or(after, |(inside, _)| inside)
}

fn level(name: &str) -> Options {
    LEVELS()[name].clone()
}

fn types(program: &model::Program) -> std::collections::BTreeMap<&str, &model::Type> {
    program.modules[0].types.iter().map(|one| (one.name.as_str(), one)).collect()
}

fn function<'p>(program: &'p model::Program, name: &str) -> &'p model::Function {
    program.modules[0].functions.iter().find(|one| one.name == name).expect("the function exists")
}

fn lowered_named(program: &model::Program, name: &str) -> hir::Lowered {
    hir::lower(program).expect("lowers").into_iter().find(|one| one.name == name).expect("the body exists")
}

fn kinds(body: &mir::MirBody) -> BTreeSet<Kind> {
    body.blocks.iter().flat_map(|block| &block.ops).map(|op| op.kind).collect()
}

fn instructions(function: &model::Function) -> impl Iterator<Item = &model::Instruction> {
    function.blocks.iter().flat_map(|block| &block.instructions)
}

fn arg_width(arg: &Arg) -> u32 {
    match arg {
        Arg::Held(one) => one.width,
        Arg::Const(one) => one.width,
        Arg::Symbol(one) => one.width,
        Arg::FrameAddress(one) => one.width,
        Arg::FrameSelector(one) => one.width,
        Arg::Cell(one) => one.r#ref.width,
        Arg::Opaque(_) => unreachable!("no width"),
    }
}

/// `re.search(prefix + tail)` where `tail` names a group of `prefix` by `(?P=...)`.
fn search_back(text: &str, prefix: &str, tail: impl Fn(&regex::Captures<'_>) -> String) -> Option<String> {
    for found in Regex::new(prefix).expect("a pattern").captures_iter(text) {
        let end = found.get(0).expect("a match").end();
        let rest = Regex::new(&format!("^(?:{})", tail(&found))).expect("a pattern");
        if let Some(more) = rest.find(&text[end..]) {
            return Some(text[found.get(0).expect("a match").start()..end + more.end()].to_owned());
        }
    }
    None
}

fn program() -> model::Program {
    parsed(&fixture("control.mod"))
}

#[test]
fn test_frontend_document_crosses_the_strict_common_hir_boundary() {
    let program = program();
    assert_eq!(program.dialect, model::Dialect::Modern);
    assert_eq!(program.runtime, model::RuntimeProfile::Freestanding);
    let names: Vec<&str> = program.modules[0].functions.iter().map(|one| one.name.as_str()).collect();
    assert_eq!(names, ["step", "count"]);
    assert_eq!(hir::decode(&hir::encode(&program, None).expect("encodes")).expect("decodes"), program);
}

#[test]
fn test_frontend_lowers_control_flow_and_calls_to_existing_mir() {
    let program = program();
    let count = kinds(&lowered_named(&program, "control.count").body);
    let step = kinds(&lowered_named(&program, "control.step").body);
    for kind in [Kind::Call, Kind::Branch, Kind::Load, Kind::Store] {
        assert!(count.contains(&kind), "{kind:?}");
    }
    assert!(step.contains(&Kind::Add));
}

#[test]
fn test_frontend_json_is_deterministic_and_replayable() {
    let directory = tempfile::tempdir().expect("a directory");
    let first = directory.path().join("first.json");
    let second = directory.path().join("second.json");
    driver::parsed(&fixture("control.mod"), Some(&first)).expect("parses");
    driver::parsed(&fixture("control.mod"), Some(&second)).expect("parses");
    let first = std::fs::read(first).expect("dumped");
    assert_eq!(first, std::fs::read(second).expect("dumped"));
    let Json::Dict(document) = pyjson::loads(&String::from_utf8(first).expect("utf-8")).expect("JSON") else {
        panic!("not an object");
    };
    assert_eq!(document.get("schema"), Some(&Json::Int(1)));
}

#[test]
fn test_type_error_is_reported_above_hir() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "wrong.mod",
        "fn wrong(value: i16) -> i16:\n    if value:\n        return 1\n    return 0\n",
    );
    assert!(refused(&source).contains("expected bool"));
}

#[test]
fn test_all_primitive_types_cross_hir_with_their_exact_representation() {
    let program = parsed(&fixture("primitives.mod"));
    let types = types(&program);
    let names: BTreeSet<&str> = types.keys().copied().collect();
    assert_eq!(
        names,
        BTreeSet::from(["void", "bool", "char", "i8", "u8", "i16", "u16", "i32", "u32", "f32", "f64", "string", "addr"])
    );
    let integral: Vec<(i64, Option<bool>)> =
        ["char", "i8", "u8", "i16", "u16", "i32", "u32"].iter().map(|name| (types[name].width, types[name].signed)).collect();
    assert_eq!(
        integral,
        [(1, Some(false)), (1, Some(true)), (1, Some(false)), (2, Some(true)), (2, Some(false)), (4, Some(true)), (4, Some(false))]
    );
    assert_eq!((types["bool"].width, types["void"].width), (1, 0));
    assert_eq!(types["f32"].evaluation, model::FloatEvaluation::Binary32);
    assert_eq!(types["f64"].evaluation, model::FloatEvaluation::Binary64);
    let sizes: Vec<usize> = program.modules[0].data.iter().map(|one| one.bytes.len()).collect();
    assert_eq!(sizes, [4, 8]);
}

#[test]
fn test_unsigned_and_floating_operations_keep_their_semantics_in_mir() {
    let program = parsed(&fixture("primitives.mod"));
    let lowered = hir::lower(&program).expect("lowers");
    assert!(lowered.iter().all(|one| mir::verify(&one.body).is_empty()));
    let kinds_of = |name: &str| kinds(&lowered.iter().find(|one| one.name == format!("primitives.{name}")).unwrap().body);

    assert!(kinds_of("unsigned_divide").contains(&Kind::Udivmod));
    assert!(!kinds_of("unsigned_divide").contains(&Kind::Divmod));
    assert!(kinds_of("unsigned_remainder").contains(&Kind::Udivmod));
    assert!(kinds_of("float_product").contains(&Kind::Fmul));

    let less = &lowered.iter().find(|one| one.name == "primitives.unsigned_less").unwrap().body;
    let branch = less.blocks.iter().flat_map(|block| &block.ops).find(|op| op.kind == Kind::Branch).unwrap();
    assert_eq!(branch.test, Some(Kind::Below));
}

#[test]
fn test_fixed_point_types_scale_literals_and_keep_storage_width_in_mir() {
    let program = parsed(&fixture("fixed.mod"));
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
        instructions(fixed_literals).filter(|one| one.callee.as_deref() == Some("_pf4")).collect();
    assert_eq!(decimal_prints.len(), 2);
    for decimal_print in decimal_prints {
        let [raw, fraction] = decimal_print.operands.as_slice() else { panic!("two operands") };
        let model::Operand::ValueRef(raw) = raw else { panic!("a value") };
        let value_type = fixed_literals.values.iter().find(|one| one.id == raw.value).unwrap().r#type;
        assert_eq!(value_type, types["i32"].id);
        assert_eq!(fraction, &model::Operand::constant(types["u8"].id, 16));
    }

    let lowered = hir::lower(&program).expect("lowers");
    assert!(lowered.iter().all(|one| mir::verify(&one.body).is_empty()));
    let product = kinds(&lowered.iter().find(|one| one.name == "fixed.product").unwrap().body);
    let quotient = kinds(&lowered.iter().find(|one| one.name == "fixed.quotient").unwrap().body);
    // fixed8 uses a 32-bit intermediate; fixed16 is already based on i32 and
    // stays one semantic operation until target lowering selects EDX:EAX.
    assert!([Kind::SignExtend, Kind::Mul, Kind::Sar].iter().all(|one| product.contains(one)));
    assert!(quotient.contains(&Kind::FixedDiv));
    assert!(![Kind::SignExtend, Kind::Shl, Kind::Divmod].iter().any(|one| quotient.contains(one)));
}

/// `physicalize(program, nbody, lowered nbody.nbody)`.
fn nbody_physical() -> crate::frontends::qb::abi::Physicalized {
    let program = parsed(&fixture("nbody.mod"));
    let function = function(&program, "nbody");
    let lowered = lowered_named(&program, "nbody.nbody");
    physicalize(&program, function, &lowered).expect("physicalizes")
}

#[test]
fn test_fixed_i32_product_stays_a_storage_width_operation_through_physicalization() {
    // Native nbody used to route every Q23.9 product through generic i64 MIR.
    let physical = nbody_physical();
    let fixed: Vec<&mir::Op> = physical
        .lowered
        .body
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .filter(|op| matches!(op.kind, Kind::FixedMul | Kind::FixedDiv))
        .collect();

    assert!(!fixed.is_empty());
    assert!(fixed.iter().all(|op| arg_width(&op.results[0]) == 4));
    assert!(fixed.iter().all(|op| op.args.iter().all(|arg| arg_width(arg) <= 4)));
}

#[test]
fn test_fixed_i32_arithmetic_never_enters_generic_int64_legalization() {
    // nbody's Q23.9 inner loop expanded one division to 311 inline bytes.
    let physical = nbody_physical();
    let legalized = lower_int64::expanded(
        &physical.lowered.body,
        Some(&physical.calls),
        Some(&physical.contracts),
        Some(&physical.hints),
    )
    .expect("legalizes");

    assert!(legalized.inline.is_empty());
}

#[test]
fn test_nbody_arrays_strings_and_print_cross_hir_and_verify_in_mir() {
    let program = parsed(&fixture("nbody.mod"));
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
        let length = literal.bytes[0] | literal.bytes[1] << 8;
        let capacity = literal.bytes[2] | literal.bytes[3] << 8;
        assert_eq!(length, capacity);
        assert_eq!(length, literal.bytes.len() as i64 - 5);
        assert_eq!(*literal.bytes.last().unwrap(), 0);
    }

    let callables: std::collections::BTreeMap<&str, &model::Callable> =
        module.callables.iter().map(|one| (one.name.as_str(), one)).collect();
    assert!(!callables["_pt"].defined);
    assert!(!callables["_pf4"].defined);
    assert_eq!(callables["_pf4"].parameter_types, [types["i32"].id, types["u8"].id]);
    assert!(!callables["_pn"].defined);
    assert!(module.functions[0].calls.iter().all(|call| call.distance == model::CallDistance::Far));
    let fixed_id = callables["_pf4"].id;
    assert!(module.functions[0].calls.iter().filter(|call| call.callee == Some(fixed_id)).all(|call| call.order == [1, 0]));

    let lowered = lowered_named(&program, "nbody.nbody");
    assert!(mir::verify(&lowered.body).is_empty());
    let kinds = kinds(&lowered.body);
    for kind in [Kind::Address, Kind::Branch, Kind::Call, Kind::FixedDiv, Kind::FixedMul, Kind::Load, Kind::Mul, Kind::Store]
    {
        assert!(kinds.contains(&kind), "{kind:?}");
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
    let program = parsed(&fixture("nbody.mod"));
    let strings: Vec<&model::Place> =
        program.modules[0].functions[0].places.iter().filter(|place| place.name.starts_with("$string")).collect();
    assert!(!strings.is_empty());
    assert!(strings.iter().all(|place| place.offset == 4));
}

#[test]
fn test_nbody_native_loops_eliminate_redundant_index_arithmetic() {
    // Modern nbody emitted 52 `sub index,0; shl index,4` address chains.
    let assembly = listing(&parsed(&fixture("nbody.mod")), "main", &O2());

    assert!(!assembly.contains("sub si, 0"));
    assert!(!assembly.contains("sub di, 0"));
    let scaled_indices = assembly.matches("shl si, 4").count() + assembly.matches("shl di, 4").count();
    assert!(scaled_indices <= 2);
}

#[test]
fn test_nbody_position_loop_uses_one_end_relative_byte_offset() {
    // -O2 unrolls the loop away.
    let assembly = listing(&parsed(&fixture("nbody.mod")), "main", &level("Os"));
    let loop_ = between(&assembly, "L0_18:\n", "    jne L0_18\n");

    assert!(!loop_.contains("mov si, ax"));
    assert!(!loop_.contains("shl si, 4"));
    assert!(!loop_.contains("lea di"));
    assert!(!loop_.contains("cmp ax, 6"));
    assert!(assembly.contains("mov si, 65440\nL0_18:")); // -96 in a word
    assert!(search_back(
        loop_,
        r"    mov (?P<x>e(?:ax|bx|cx|dx|si|di)), dword ptr \[bp\+si\+8\]\n",
        |found| format!(r"    add dword ptr \[bp\+si\], {}\n", &found["x"]),
    )
    .is_some());
    assert!(search_back(
        loop_,
        r"    mov (?P<y>e(?:ax|bx|cx|dx|si|di)), dword ptr \[bp\+si\+12\]\n",
        |found| format!(r"    add dword ptr \[bp\+si\+4\], {}\n", &found["y"]),
    )
    .is_some());
    assert!(loop_.contains("add si, 16\nL0_17:\n"));
    assert!(!loop_.contains("or si, si"));
    assert!(!loop_.contains("cmp si"));
}

#[test]
fn test_nbody_identity_uses_the_paired_byte_recurrences() {
    // -O2 unrolls the loop away.
    let assembly = listing(&parsed(&fixture("nbody.mod")), "main", &level("Os"));
    let interaction = between(&assembly, "L0_7:\n", "L0_2:\n");
    let force_loops = between(&assembly, "L0_3:\n", "L0_9:\n");

    assert!(!Regex::new(r"    shl (?:[sd]i|word ptr \[[^\]]+\]), 4\n").unwrap().is_match(interaction));
    assert!(!interaction.contains(", 96\n"));
    let recurrences = Regex::new(r"    add (?:[sd]i|word ptr \[[^\]]+\]), 16\nL\d+_\d+:\n    jne L\d+_\d+\n")
        .unwrap()
        .find_iter(force_loops)
        .count();
    assert_eq!(recurrences, 2);
    assert_eq!(Regex::new(r"    mov (?:[sd]i|word ptr \[[^\]]+\]), 65440\n").unwrap().find_iter(force_loops).count(), 2);
}

#[test]
fn test_nbody_velocity_fields_are_stored_once_per_update() {
    let assembly = listing(&parsed(&fixture("nbody.mod")), "main", &O2());
    let interaction = between(&assembly, "L0_7:\n", "L0_2:\n");

    for field in [8, 12] {
        let stores = Regex::new(&format!(r"dword ptr \[bp\+[sd]i\+{field}\], e(?:ax|bx|cx|dx|si|di)\n")).unwrap();
        let loads = Regex::new(&format!(r"e(?:ax|bx|cx|dx|si|di), dword ptr \[bp\+[sd]i\+{field}\]\n")).unwrap();
        assert_eq!(stores.find_iter(interaction).count(), 1);
        assert_eq!(loads.find_iter(interaction).count(), 1);
    }
}

const STRIDE: &str = "\
struct sample:
    tag: i16
    value: i32
    delta: i32

fn update() -> i32:
    var samples: [sample; 5] = [
        sample { tag: 0, value: 1, delta: 2 },
        sample { tag: 0, value: 2, delta: 3 },
        sample { tag: 0, value: 3, delta: 4 },
        sample { tag: 0, value: 4, delta: 5 },
        sample { tag: 0, value: 5, delta: 6 },
    ]
    for current in &mut samples:
        current.value += current.delta
    return samples[0].value + samples[4].value

fn main() -> i16:
    update()
    return 0
";

#[test]
fn test_counted_struct_loop_uses_its_record_width_as_the_byte_stride() {
    // The end-relative recurrence is an affine-loop rule, not a body/16 rule.
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(&directory, "stride.mod", STRIDE);

    // -O2 unrolls the loop away.
    let assembly = listing(&parsed(&source), "main", &level("Os"));

    let prefix = Regex::new(r"    mov (?P<offset>[sd]i), 65486\n(?P<label>L\d+_\d+):\n").unwrap();
    let found = prefix.captures_iter(&assembly).find_map(|found| {
        let end = found.get(0).unwrap().end();
        let tail = Regex::new(&format!(
            r"^(?P<body>(?:    .*\n)+?)    add {}, 10\nL\d+_\d+:\n    jne {}\n",
            &found["offset"], &found["label"]
        ))
        .unwrap();
        tail.captures(&assembly[end..]).map(|rest| (found["offset"].to_owned(), rest["body"].to_owned()))
    });
    let (offset, body) = found.expect("the loop"); // -5 * sizeof(sample), with sizeof(sample) == 10
    assert!(body.contains(&format!("dword ptr [bp+{offset}+2]")));
    assert!(body.contains(&format!("dword ptr [bp+{offset}+6]")));
    assert!(!assembly.contains(&format!("shl {offset}")));
}

#[test]
fn test_os_copies_no_loop_into_larger_code() {
    // -O2 unrolls the five-record update from 65 lines to 83.
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "stride.mod",
        "\
struct sample:
    tag: i16
    value: i32
    delta: i32

fn total(samples: &[sample]) -> i32:
    var sum: i32 = 0
    for one in &samples:
        sum += one.value
    return sum

fn update(v: &[i32]) -> i32:
    var samples: [sample; 5] = [
        sample { tag: 0, value: v[0], delta: v[1] },
        sample { tag: 0, value: v[1], delta: v[2] },
        sample { tag: 0, value: v[2], delta: v[3] },
        sample { tag: 0, value: v[3], delta: v[4] },
        sample { tag: 0, value: v[4], delta: v[5] },
    ]
    for current in &mut samples:
        current.value += current.delta
    return total(&samples)

fn main() -> i16:
    let v: [i32; 6] = [1, 2, 3, 4, 5, 6]
    update(&v)
    return 0
",
    );
    let size = |options: &Options| -> usize {
        listing(&parsed(&source), "main", options).lines().filter(|line| line.starts_with("    ")).count()
    };

    let uncopied = size(&Options { unroll: false, peel: false, ..Options::default() });
    assert!(size(&level("O2")) > uncopied);
    assert!(size(&level("Os")) <= uncopied);
}

#[test]
fn test_fixed_array_storage_has_a_prefix_descriptor() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "array_descriptor.mod",
        "fn main() -> i16:\n    var values: [i16; 3] = [10, 20, 30]\n    print(values.len())\n    return values[0]\n",
    );

    let program = parsed(&source);
    let function = &program.modules[0].functions[0];
    let values = function.places.iter().find(|place| place.name == "values").unwrap();
    let descriptor = |name: &str| function.places.iter().find(|place| place.name == name).unwrap();
    assert_eq!(values.offset, -6);
    assert_eq!(values.extent, Some(6));
    assert_eq!(descriptor("$values.length").offset, values.offset - 4);
    assert_eq!(descriptor("$values.capacity").offset, values.offset - 2);

    let assembly = listing(&parsed(&source), "main", &O2());
    assert!(assembly.contains("mov word ptr [bp-10], 3"));
    assert!(assembly.contains("mov word ptr [bp-8], 3"));
}

#[test]
fn test_borrowed_array_call_builds_one_view_from_the_direct_payload() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "array_borrow.mod",
        "fn bump(values: &mut [u16]) -> void:\n    values[1] += 3\nfn main() -> i16:\n    var values: [u16; 3] = [10, 20, 30]\n    bump(&mut values)\n    return 0\n",
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

    let assembly = listing(&program, "main", &O2());
    let bump = between(&assembly, "_bump proc far", "_bump endp");
    let main = between(&assembly, "_main proc far", "_main endp");
    assert!(Regex::new(r"    lea [a-z]+, (?:word ptr )?\[bp-6\]\n").unwrap().is_match(main));
    assert!(Regex::new(r"    mov [a-z]+, ss\n").unwrap().is_match(main));
    assert!(main.contains("call far ptr _bump"));
    assert!(main.contains("add sp, 4"));
    assert!(bump.contains("es:["));
    assert!(!modern_compile::written(&program, "main", &source, &O2()).expect("writes").is_empty());
}

#[test]
fn test_borrow_rules_reject_shared_mutation_and_aliasing_mutable_arguments() {
    let directory = tempfile::tempdir().expect("a directory");
    let shared = written(&directory, "shared.mod", "fn bad(values: &[u16]) -> void:\n    values[0] = 2\n");
    assert!(refused(&shared).contains("immutable"));

    let aliased = written(
        &directory,
        "aliased.mod",
        "fn use(left: &mut [u16], right: &[u16]) -> void:\n    left[0] += right[0]\nfn bad() -> void:\n    var values: [u16; 1] = [1]\n    use(&mut values, &values)\n",
    );
    assert!(refused(&aliased).contains("aliases a mutable argument"));
}

#[test]
fn test_readonly_array_borrow_keeps_payload_initialization_visible_to_callee() {
    // sum returned stack garbage after DSE erased every payload store before its read-only call.
    let assembly = listing(&parsed(&fixture("sum.mod")), "main", &O2());
    let main = between(&assembly, "_main proc far", "_main endp");

    assert!((1..7).all(|value| main.contains(&format!(", {value}"))));
}

#[test]
fn test_array_parameter_is_one_unsized_view_pointer() {
    let program = parsed(&fixture("sum.mod"));
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
    let assembly = listing(&parsed(&fixture("sum.mod")), "main", &O2());
    let function = between(&assembly, "_sum proc far", "_sum endp");
    let hot = between(function, "L0_3:", "L0_5:");

    assert!(!Regex::new(r"\b(?:imul|shl|lea)\b").unwrap().is_match(hot));
    assert!(function.contains("xor ax, ax"));
    assert!(!function.contains("dec "));
    assert!(Regex::new(r"\badd\s+(?:si|di|bx),\s*2\s*\n(?:L\w+:\n)?\s*jne\b").unwrap().is_match(hot));
}

#[test]
fn test_three_array_initializer_keeps_the_fixed_frame_address_component() {
    // sum_three wrote locals through EAX+SI after a secondary-base rewrite lost BP.
    let assembly = listing(&parsed(&fixture("sum_three.mod")), "main", &O2());
    let main = between(&assembly, "_main proc far", "call far ptr _sum_three");
    let cells: BTreeSet<String> =
        [-8, -20, -32].iter().flat_map(|payload| (0..4).map(move |index| format!("[bp{}]", payload + 2 * index))).collect();
    let initializers: BTreeSet<String> = Regex::new(r"mov word ptr (\[[^\]]+\]), \d+")
        .unwrap()
        .captures_iter(main)
        .map(|found| found[1].to_owned())
        .filter(|one| cells.contains(one))
        .collect();

    assert_eq!(initializers, cells);
}

/// `sum.sum`'s optimized physical body, from `semantic` in place of its semantic MIR.
fn sum_optimized_physical(program: &model::Program, semantic: &hir::Lowered) -> mir::MirBody {
    let function = function(program, "sum");
    let target = targets::profile("386").unwrap();
    let optimized = modern_compile::optimized(program, function, semantic, target, None, &O2()).unwrap();
    let physical = physicalize(program, function, &optimized).unwrap();
    modern_compile::optimized(program, function, &physical.lowered, target, Some(&physical.calls), &O2()).unwrap().body
}

#[test]
fn test_runtime_bounded_array_loop_has_a_symbolic_count_proof() {
    let program = parsed(&fixture("sum.mod"));
    let semantic =
        modern_compile::semantic_lowered(&program).unwrap().into_iter().find(|one| one.name == "sum.sum").unwrap();
    let length = semantic
        .body
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .flat_map(|op| &op.defines)
        .find(|value| semantic.body.integer_ranges.contains_key(*value))
        .copied()
        .unwrap();
    assert_eq!(
        semantic.body.integer_ranges.get(&length).unwrap().clone(),
        mir::IntegerRange { low: BigInt::from(0), high: BigInt::from(32768), width: 2 }
    );

    let body = sum_optimized_physical(&program, &semantic);
    let found = loops::loops(&body.blocks, Some(body.entry));
    let [loop_] = found.as_slice() else { panic!("one loop") };
    let recurrences = induction::basics(&body, loop_);
    assert_eq!(recurrences.len(), 1);
    assert_eq!(
        recurrences.values().next().unwrap().step,
        induction::AffineOperand::Const(mir::Const::new(2, 2))
    );

    let predecessors = loops::predecessors(&body.blocks);
    assert!(body.blocks.iter().all(|block| block.phis.iter().all(|phi| {
        phi.incoming.keys().copied().collect::<BTreeSet<i64>>() == predecessors[&block.at]
    })));
}

#[test]
fn test_runtime_bounded_array_control_respects_the_recurrence_period() {
    // A stride-two offset repeats after 32768 word updates and cannot control a longer loop.
    let program = parsed(&fixture("sum.mod"));
    let semantic =
        modern_compile::semantic_lowered(&program).unwrap().into_iter().find(|one| one.name == "sum.sum").unwrap();
    let [length] = semantic.body.integer_ranges.keys().copied().collect::<Vec<_>>()[..] else {
        panic!("one range")
    };
    let mut unsafe_ = semantic.clone();
    unsafe_.body.integer_ranges = mir::OrderedMap::from_iter([(
        length,
        mir::IntegerRange { low: BigInt::from(0), high: BigInt::from(32769), width: 2 },
    )]);

    let body = sum_optimized_physical(&program, &unsafe_);
    let found = loops::loops(&body.blocks, Some(body.entry));
    let [loop_] = found.as_slice() else { panic!("one loop") };
    let mut steps: Vec<BigInt> = induction::basics(&body, loop_)
        .values()
        .filter_map(|one| match &one.step {
            induction::AffineOperand::Const(step) => Some(step.n.clone()),
            induction::AffineOperand::Held(_) => None,
        })
        .collect();
    steps.sort();
    assert_eq!(steps, [BigInt::from(1), BigInt::from(2)]);
}

#[test]
fn test_borrowed_array_parameter_rejects_a_repeated_fixed_length() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(&directory, "sized_parameter.mod", "fn old(values: &[u16; 3]) -> void:\n    return\n");

    assert!(refused(&source).contains("omit the length"));
}

#[test]
fn test_scoped_array_range_is_one_descriptor_pointer_and_executes() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "slice.mod",
        "fn sum(values: &[i16]) -> i16:\n    var total: i16 = 0\n    for value in &values:\n        total += value\n    return total\nfn main() -> i16:\n    let values: [i16; 4] = [1, 2, 3, 4]\n    return sum(&values[1:3])\n",
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
        "range_slice.mod",
        "fn sum(values: &[i16]) -> i16:\n    return values[0]\nfn main() -> i16:\n    let values: [i16; 4] = [1, 2, 3, 4]\n    return sum(&values[1..3])\n",
    );

    refused(&source);
}

#[test]
fn test_data_is_an_explicit_pointer_escape_hatch() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "data.mod",
        "fn data(values: &[i16]) -> addr:\n    return values.data()\nfn main() -> i16:\n    let values: [i16; 2] = [4, 9]\n    data(&values)\n    return 0\n",
    );

    let program = parsed(&source);
    let types = types(&program);
    assert_eq!(types["addr"].kind, model::TypeKind::Pointer);
    assert_eq!((types["addr"].width, types["addr"].address), (4, model::AddressKind::Far));
    assert!(!modern_compile::written(&program, "main", &source, &O2()).expect("writes").is_empty());
}

#[test]
fn test_string_descriptor_methods_and_value_iteration_need_no_runtime() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "string_view.mod",
        "fn first(text: string) -> char:\n    for byte in text:\n        return byte\n    return '\\0'\nfn size(text: string) -> u16:\n    return text.len() + text.capacity()\nfn main() -> u16:\n    let text: string = \"abc\"\n    if first(text) == 'a':\n        return size(text)\n    return 0\n",
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
        "first.mod",
        "fn first(text: string) -> char:\n    for byte in text:\n        return byte\n    return '\\0'\nfn main() -> i16:\n    if first(\"metal\") == 'm':\n        print(\"ok\")\n        return 0\n    return 1\n",
    );

    let program = parsed(&source);
    assert!(!modern_compile::written(&program, "main", &source, &O2()).expect("writes").is_empty());
}

#[test]
fn test_bounded_comprehension_materializes_and_generator_fuses() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "comprehension.mod",
        "fn main() -> i16:\n    let values: [i16; 4] = [1, 2, 3, 4]\n    let doubled = [value * 2 for value in values]\n    var total: i16 = 0\n    for value in (item + 1 for item in doubled):\n        total += value\n    return total\n",
    );

    let program = parsed(&source);
    assert!(program.modules[0].callables.iter().all(|one| !["iter", "next", "collect", "append"].contains(&one.name.as_str())));
    assert!(!modern_compile::written(&program, "main", &source, &O2()).expect("writes").is_empty());
}

#[test]
fn test_dictionary_comprehension_deduplicates_and_has_explicit_lookup() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "dictionary.mod",
        "fn main() -> i16:\n    let values: [i16; 4] = [1, 2, 1, 3]\n    let table = {item: item * 10 for item in values}\n    return table.get(1, 0) + table.get(3, 0) + table.get(9, 5)\nfn count() -> u16:\n    let values: [i16; 4] = [1, 2, 1, 3]\n    let table = {item: item * 10 for item in values}\n    return table.len()\n",
    );

    let program = parsed(&source);
    assert!(!modern_compile::written(&program, "main", &source, &O2()).expect("writes").is_empty());
}

#[test]
fn test_fixed_point_arithmetic_has_a_price() {
    // Unpriced FIXED_MUL left nbody unpriceable, so every loop copy was built only to be refused.
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "fixed.mod",
        "type fixed16 = fixed i32, fraction=16\n\nfn scaled(left: fixed16, right: fixed16) -> fixed16:\n    return left * right / right\n\nfn main() -> i16:\n    scaled(1.5, 2.25)\n    return 0\n",
    );
    let costs = &targets::profile("386").unwrap().operations;
    let bodies: Vec<mir::MirBody> = hir::lower(&parsed(&source)).unwrap().into_iter().map(|one| one.body).collect();
    let kinds: BTreeSet<Kind> = bodies.iter().flat_map(kinds).collect();

    assert!(kinds.contains(&Kind::FixedMul) && kinds.contains(&Kind::FixedDiv));
    assert!(bodies.iter().all(|body| profit::r#static(body, costs).is_some()));
}

#[test]
fn test_a_repeat_literal_in_the_frame_is_one_string_fill() {
    // The fill loop stepped its byte address to zero under `!=`, which `fill` missed: 64 stores in a loop.
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "frame_fill.mod",
        "fn value(k: i16) -> i32:\n    var a: [i32; 64] = [0; 64]\n    a[k] = 5\n    return a[k] + a[k + 1]\nfn main() -> i16:\n    return i16(value(3))\n",
    );
    let assembly = listing(&parsed(&source), "main", &O2());
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
        "priced_fill.mod",
        "fn value(v: &[i16]) -> i32:\n    var a: [i32; 64] = [0; 64]\n    var total: i16 = 0\n    for i in 0..4:\n        total += v[i]\n    a[total] = 5\n    return a[1]\nfn main() -> i16:\n    let v: [i16; 4] = [1, 2, 3, 4]\n    return i16(value(&v))\n",
    );
    let assembly = listing(&parsed(&source), "main", &O2());
    let body = &assembly[assembly.find("_value proc").unwrap()..assembly.find("_value endp").unwrap()];

    assert!(body.contains("rep stosd"));
    assert!(!Regex::new(r"\bj\w+\s").unwrap().is_match(body));
}

#[test]
fn test_a_ranked_repeat_literal_at_os_is_one_string_fill() {
    // `[0; 8, 8]` at -Os was an 8-trip loop around an 8-cell loop: its count was unproved and nested fills never merged.
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "nested_fill.mod",
        "fn value(k: i16) -> i32:\n    var a: [i32; 8, 8] = [0; 8, 8]\n    a[k, 1] = 5\n    return a[k, 2]\nfn main() -> i16:\n    return i16(value(3))\n",
    );
    let assembly = listing(&parsed(&source), "main", &level("Os"));
    let body = &assembly[assembly.find("_value proc").unwrap()..assembly.find("_value endp").unwrap()];

    assert_eq!(body.matches("rep stosd").count(), 1);
    assert!(body.contains("mov cx, 64"));
    assert!(!Regex::new(r"\bj\w+\s").unwrap().is_match(body));
}

#[test]
fn test_unroll_is_priced_against_the_loop_as_optimized() {
    // Unroll compared its settled copy with the loop mid-round: `b`'s fill became eight at -Os, not one.
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "priced_unroll.mod",
        concat!(
            "type fix = fixed i32, fraction=8\n",
            "fn value(k: i16) -> fix:\n",
            "    var a: [fix; 8, 8] = [0; 8, 8]\n",
            "    var b: [fix; 8, 8] = [0; 8, 8]\n",
            "    for i in 0..8:\n",
            "        for j in 0..8:\n",
            "            a[i, j] = fix(i * 3 + j + 1) / 4\n",
            "            if i == j:\n",
            "                b[i, j] = 2\n",
            "            else:\n",
            "                b[i, j] = fix((i + j) % 3) / 2\n",
            "    return a[k, 1] + b[k, 2]\n",
            "fn main() -> i16:\n",
            "    value(3)\n",
            "    return 0\n",
        ),
    );
    let assembly = listing_on(&parsed(&source), "main", &level("Os"), "486");
    let body = &assembly[assembly.find("_value proc").unwrap()..assembly.find("_value endp").unwrap()];

    assert_eq!(body.matches("rep stosd").count(), 2);
    assert_eq!(body.matches("mov cx, 64").count(), 2);
}

fn _settled(directory: &tempfile::TempDir, text: &str, options: &Options) -> mir::MirBody {
    let source = written(directory, "settled.mod", text);
    let program = parsed(&source);
    let function = function(&program, "value");
    let semantic = modern_compile::semantic_lowered(&program)
        .expect("lowers")
        .into_iter()
        .find(|one| one.name.ends_with(".value"))
        .expect("the body exists");
    let target = targets::profile("486").unwrap();
    modern_compile::optimized(&program, function, &semantic, target, None, options).unwrap().body
}

#[test]
fn test_a_fill_count_that_is_a_number_is_written_as_one() {
    // A merged fill's count stayed a held 64, which pricing read as an unknown ten cells.
    let directory = tempfile::tempdir().expect("a directory");
    let text = "fn value(k: i16) -> i32:\n    var a: [i32; 8, 8] = [0; 8, 8]\n    a[k, 1] = 5\n    return a[k, 2]\n";
    let body = _settled(&directory, text, &level("Os"));
    let counts = body
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .filter(|op| op.kind == Kind::Fill)
        .map(|op| op.args[1].clone())
        .collect::<Vec<_>>();

    assert_eq!(counts, vec![Arg::Const(mir::Const::new(BigInt::from(64), 2))]);
}

#[test]
fn test_an_unnamed_loop_is_priced_at_its_proven_trip_count() {
    // Every loop but the one asked about was priced at ten trips, so an 8-trip outer loop cost 25% too much.
    let directory = tempfile::tempdir().expect("a directory");
    let text = concat!(
        "fn value(v: &[i16]) -> i16:\n",
        "    var total: i16 = 0\n",
        "    for i in 0..8:\n",
        "        total += v[i]\n",
        "    return total\n",
    );
    let options = Options { unroll: false, peel: false, ..Options::default() };
    let body = std::rc::Rc::new(_settled(&directory, text, &options));
    let found = loops::loops(&body.blocks, Some(body.entry));
    let [loop_] = found.as_slice() else { panic!("one loop, found {}", found.len()) };
    let frequency = profit::_frequencies(&body, None).expect("priced");

    assert_eq!(loop_.body.iter().map(|at| frequency[at]).collect::<BTreeSet<_>>(), BTreeSet::from([8]));
}

#[test]
fn test_a_new_counter_steps_where_no_condition_is_live() {
    // A rotated loop branches on flags its body set; the pointer step went between them: Unlowered.
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "struct_view.mod",
        concat!(
            "struct sample:\n",
            "    tag: i16\n",
            "    value: i32\n",
            "    delta: i32\n",
            "fn total(samples: &[sample]) -> i32:\n",
            "    var sum: i32 = 0\n",
            "    for one in &samples:\n",
            "        sum += one.value\n",
            "    return sum\n",
            "fn main() -> i16:\n",
            "    let s: [sample; 2] = [sample { tag: 0, value: 1, delta: 2 }, sample { tag: 0, value: 2, delta: 3 }]\n",
            "    return i16(total(&s))\n",
        ),
    );
    let assembly = listing(&parsed(&source), "main", &O2());
    let body = &assembly[assembly.find("_total proc").unwrap()..assembly.find("_total endp").unwrap()];

    assert!(
        Regex::new(r"add (?:si|di|bx), 10\n    add (?:si|di|bx|cx|dx|ax), 10\n(?:L\w+:\n)?    jne").unwrap().is_match(body)
    );
}

#[test]
fn test_an_unrolled_fill_stores_to_fixed_frame_cells() {
    // Each unrolled store of `[0; 8, 8]` loaded its constant offset into a register first: 64 extra movs.
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "unrolled_fill.mod",
        "fn value(k: i16) -> i32:\n    var a: [i32; 8, 8] = [0; 8, 8]\n    a[k, 1] = 5\n    return a[k, 2]\nfn main() -> i16:\n    return i16(value(3))\n",
    );
    // The 486 unrolls it: a dword store is one clock, `rep stosd` 7+4n.
    let assembly = listing_on(&parsed(&source), "main", &O2(), "486");
    let body = &assembly[assembly.find("_value proc").unwrap()..assembly.find("_value endp").unwrap()];
    let zeroes: Vec<String> =
        Regex::new(r"mov dword ptr \[(.*?)\], 0\n").unwrap().captures_iter(body).map(|one| one[1].to_owned()).collect();

    assert_eq!(zeroes.len(), 64);
    assert!(zeroes.iter().all(|one| Regex::new(r"^bp-\d+$").unwrap().is_match(one)));
}

#[test]
fn test_ill_formed_operators_conversions_and_repeats_are_rejected() {
    for body in [
        "    return i16(1 < 2 < 3)\n",
        "    return 1 << 16\n",
        "    return 1 << -1\n",
        "    let a: i16 = 1\n    let b: u16 = 1\n    return i16(a + b)\n",
        "    let a: i8 = -1\n    let b: u16 = 1\n    return i16(a < b)\n",
        "    return i16(bool(1))\n",
        "    return i16(u8(300))\n",
        "    return i16(f64(1) & f64(2))\n",
        "    let a: [i16; 4] = [0; 3]\n    return a[0]\n",
    ] {
        let directory = tempfile::tempdir().expect("a directory");
        let source = written(&directory, "rejected.mod", &format!("fn value() -> i16:\n{body}"));
        refused(&source);
    }
}

#[test]
fn test_not_is_not_an_operand_of_a_tighter_operator() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(&directory, "not.mod", "fn value() -> bool:\n    return true == not false\n");
    refused(&source);
}

#[test]
fn test_a_float_does_not_convert_to_fixed_point() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "float_fixed.mod",
        "type fix = fixed i32, fraction=8\nfn value() -> i16:\n    let x: f64 = 1.5\n    return i16(fix(x))\n",
    );
    refused(&source);
}

#[test]
fn test_ranked_arrays_index_fill_and_borrow_row_major() {
    let program = parsed(&fixture("ranked.mod"));
    assert_eq!(program.array_order, model::ArrayOrder::RowMajor);
}

/// From `tests/test_hir_execute.py`, less its `execute.run`.
#[test]
fn test_borrowed_struct_arrays_and_reborrows_keep_scoped_mutation() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "struct_array_borrow.mod",
        "struct point:\n    x: i16\n    y: i16\nfn nudge(point: &mut point) -> void:\n    point.x += point.y\nfn update(points: &mut [point]) -> void:\n    for point in &mut points:\n        nudge(&mut point)\nfn calculate() -> i16:\n    var points: [point; 2] = [{1, 2}, {10, 20}]\n    update(&mut points)\n    return points[0].x + points[1].x\n",
    );

    let assembly = listing(&parsed(&source), "calculate", &O2());
    assert!(assembly.contains("call far ptr _update"));
    assert!(assembly.contains("call far ptr _nudge"));
}

#[test]
fn test_ranked_arrays_reject_the_wrong_rank_or_shape() {
    for body in [
        "    let a: [i16; 2, 2] = [0; 2, 2]\n    return a[0]\n",
        "    let a: [i16; 2, 2, 2, 2, 2] = [0; 2, 2, 2, 2, 2]\n    return 0\n",
        "    let a: [i16; 2, 2] = [[1, 2], [3]]\n    return 0\n",
        "    let a: [i16; 2, 2] = [0; 2, 3]\n    return 0\n",
        "    let a: [i16; 2, 2] = [0; 2, 2]\n    return a.dim(2)\n",
        "    let a: [i16; 2, 2] = [0; 2, 2]\n    return first(&a)\n",
        "    let a: [i16; 2, 2] = [0; 2, 2]\n    var t: i16 = 0\n    for x in a:\n        t += x\n    return t\n",
    ] {
        let directory = tempfile::tempdir().expect("a directory");
        let source = written(
            &directory,
            "ranked_rejected.mod",
            &format!("fn first(values: &[i16]) -> i16:\n    return values[0]\nfn value() -> i16:\n{body}"),
        );
        refused(&source);
    }
}

#[test]
fn test_a_loop_past_max_completely_peel_times_stays_rolled() {
    // Copies were built for any trip count the simulation priced as folding: deedlines'
    // 16384-trip loops became 360K operations. GCC refuses past 16 before looking.
    let directory = tempfile::tempdir().expect("a directory");
    let rolled = |trips: i16| {
        let text = format!("fn value(k: i16) -> i16:\n    var total: i16 = k\n    for i in 0..{trips}:\n        total = total + i\n    return total\n");
        let body = _settled(&directory, &text, &O2());
        !loops::loops(&body.blocks, Some(body.entry)).is_empty()
    };
    assert!(!rolled(16), "within the cap the loop is copied out");
    assert!(rolled(17));
}

/// `_sum_three proc far` .. `endp` for the 486.
fn sum_three_on_486() -> String {
    let assembly = listing_on(&parsed(&fixture("sum_three.mod")), "main", &O2(), "486");
    between(&assembly, "_sum_three proc far", "_sum_three endp").to_owned()
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

#[test]
fn test_three_views_past_the_index_pairs_step_their_own_pointers() {
    // sum_three indexed three bases off one counter, and word base+index holds two: two reloads a trip.
    let function = sum_three_on_486();
    let loop_ = closed_on_jne(&function).unwrap_or_else(|| panic!("no loop closes on jne:\n{function}"));
    assert!(!loop_.contains("[bp"), "{loop_}");
    let added = Regex::new(r"\badd\s+\w+,\s*word ptr \w+:\[(?:si|di|bx)\]").unwrap();
    assert_eq!(added.find_iter(&loop_).count(), 3, "{loop_}");
}

const COLUMN: &str = "\
fn column(m: &[i32, 2], j: i16) -> i32:
    var total: i32 = 0
    for k in 0..m.dim(0):
        total += m[k, j]
    return total

fn main() -> i16:
    var m: [i32; 4, 4] = [1; 4, 4]
    column(&m, 1)
    return 0
";

fn column_loop() -> String {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(&directory, "column.mod", COLUMN);
    let assembly = listing_on(&parsed(&source), "main", &O2(), "486");
    let function = between(&assembly, "_column proc far", "_column endp");
    let start = function.find("L0_3:").unwrap_or_else(|| panic!("no L0_3:\n{function}"));
    let end = function.find("L0_5:").unwrap_or_else(|| panic!("no L0_5:\n{function}"));
    function[start..end].to_owned()
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
    assert!(Regex::new(r"\bjne L0_3\n").unwrap().is_match(&loop_), "{loop_}");
    assert!(!Regex::new(r"\b(?:jmp|xchg)\b|mov \w\w, \w\w\n").unwrap().is_match(&loop_), "{loop_}");
}
