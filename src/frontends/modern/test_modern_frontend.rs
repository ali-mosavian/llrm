//! Port of `tests/test_modern_frontend.py`.
//!
//! skipped: `execute.run` assertions (`qbopt/hir/execute.py` is tools-only),
//! and the tests made of nothing else; test_dos_bootstrap_enters_the_runtime_before_language_main
//! (reads runtime sources, no compiler).

use crate::abi::modern as rt;
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
    // The x87 evaluates in extended precision and rounds on store, as DOS C does.
    assert_eq!(types["f32"].evaluation, model::FloatEvaluation::Extended80);
    assert_eq!(types["f64"].evaluation, model::FloatEvaluation::Extended80);
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
        instructions(fixed_literals).filter(|one| one.callee.as_deref() == Some(rt::PRINT_Q4)).collect();
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
    assert!(strings.iter().all(|place| place.offset == 6));
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
    mut value: i32
    delta: i32

fn update() -> i32:
    let mut samples: sample[5] = [
        sample(tag=0, value=1, delta=2),
        sample(tag=0, value=2, delta=3),
        sample(tag=0, value=3, delta=4),
        sample(tag=0, value=4, delta=5),
        sample(tag=0, value=5, delta=6),
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
    // The end-relative recurrence is an affine-loop rule, !a body/16 rule.
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
        "fn use(left: &mut [u16], right: &[u16]) -> void:\n    left[0] += right[0]\nfn bad() -> void:\n    let mut values: u16[1] = [1]\n    use(&mut values, &values)\n",
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
    // A stride-two offset repeats after 32768 word updates && cannot control a longer loop.
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
fn test_scoped_array_range_is_one_descriptor_pointer_and_executes() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "slice.mod",
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
        "range_slice.mod",
        "fn sum(values: &[i16]) -> i16:\n    return values[0]\nfn main() -> i16:\n    let values: i16[4] = [1, 2, 3, 4]\n    return sum(&values[1..3])\n",
    );

    refused(&source);
}

#[test]
fn test_data_is_an_explicit_pointer_escape_hatch() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "data.mod",
        "fn data(values: &[i16]) -> addr:\n    return values.data()\nfn main() -> i16:\n    let values: i16[2] = [4, 9]\n    data(&values)\n    return 0\n",
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
        "fn main() -> i16:\n    let values: i16[4] = [1, 2, 3, 4]\n    let doubled = [value * 2 for value in values]\n    let mut total: i16 = 0\n    for value in (item + 1 for item in doubled):\n        total += value\n    return total\n",
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
        "fn main() -> i16:\n    let values: i16[4] = [1, 2, 1, 3]\n    let table = {item: item * 10 for item in values}\n    return table.get(1, 0) + table.get(3, 0) + table.get(9, 5)\nfn count() -> u16:\n    let values: i16[4] = [1, 2, 1, 3]\n    let table = {item: item * 10 for item in values}\n    return table.len\n",
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
        "fn value(k: i16) -> i32:\n    let mut a: i32[64] = [0] * 64\n    unsafe:\n        a[k] = 5\n        return a[k] + a[k + 1]\nfn main() -> i16:\n    return i16(value(3))\n",
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
        "field_runs.mod",
        "struct File:\n    handle: i16\n    mut buffer: u8[128]\n    mut start: u16\n\nfn opened(h: i16) -> File:\n    return File(handle=h, buffer=[0] * 128, start=0)\n\nfn relay(h: i16) -> File:\n    let f = opened(h)\n    return f\n\nfn main() -> i16:\n    let f = relay(3)\n    return f.handle + i16(f.buffer[5])\n",
    );
    let assembly = listing(&parsed(&source), "main", &O2());
    let opened = between(&assembly, "_opened proc", "_opened endp");
    let relay = between(&assembly, "_relay proc", "_relay endp");

    assert!(opened.contains("rep stos"), "{opened}");
    assert!(relay.contains("rep movs") || Regex::new(r"\bj\w+\s").unwrap().is_match(relay), "{relay}");
    for body in [opened, relay] {
        assert!(body.matches("byte ptr").count() < 8, "{body}");
    }
}

#[test]
fn test_a_ranked_repeat_literal_at_os_is_one_string_fill() {
    // `[0; 8, 8]` at -Os was an 8-trip loop around an 8-cell loop: its count was unproved and nested fills never merged.
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "nested_fill.mod",
        "fn value(k: i16) -> i32:\n    let mut a: i32[8, 8] = [[0] * 8] * 8\n    a[k, 1] = 5\n    return a[k, 2]\nfn main() -> i16:\n    return i16(value(3))\n",
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
fn test_a_negative_index_is_out_of_bounds() {
    // The check compared signed, so `i < 0` proved `i < 8` and the optimizer deleted the panic.
    let directory = tempfile::tempdir().expect("a directory");
    let text = "fn value(i: i16) -> i16:\n    let a: i16[8] = [1] * 8\n    if i < 0:\n        return a[i]\n    return 0\n";
    let body = _settled(&directory, text, &O2());
    let panics = body.blocks.iter().flat_map(|block| &block.ops).filter(|op| op.name == rt::ERROR_BOUNDS).count();

    assert_eq!(panics, 1);
}

#[test]
fn test_a_fill_count_that_is_a_number_is_written_as_one() {
    // A merged fill's count stayed a held 64, which pricing read as an unknown ten cells.
    let directory = tempfile::tempdir().expect("a directory");
    let text = "fn value(k: i16) -> i32:\n    let mut a: i32[8, 8] = [[0] * 8] * 8\n    a[k, 1] = 5\n    return a[k, 2]\n";
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
    // Every loop but the one asked about was priced at ten trips, so an 8-trip outer loop cost 25% too much;
    // a bounds check's exit into its panic did the same.
    let directory = tempfile::tempdir().expect("a directory");
    let text = concat!(
        "fn value(v: &[i16]) -> i16:\n",
        "    let mut total: i16 = 0\n",
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
            "    let mut sum: i32 = 0\n",
            "    for one in &samples:\n",
            "        sum += one.value\n",
            "    return sum\n",
            "fn main() -> i16:\n",
            "    let s: sample[2] = [sample(tag=0, value=1, delta=2), sample(tag=0, value=2, delta=3)]\n",
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
        "fn value(k: i16, j: i16) -> i32:\n    let mut a: i32[8, 8] = [[0] * 8] * 8\n    a[k, 1] = 5\n    return a[k, j]\nfn main() -> i16:\n    return i16(value(3, 2) + value(4, 1))\n",
    );
    // The 486 unrolls it: a dword store is one clock, `rep stosd` 7+4n. Two calls keep `value` unspecialized.
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
        let source = written(&directory, "rejected.mod", &format!("fn value() -> i16:\n{body}"));
        refused(&source);
    }
}

#[test]
fn test_not_is_not_an_operand_of_a_tighter_operator() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(&directory, "not.mod", "fn value() -> bool:\n    return true == !false\n");
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
        "struct point:\n    mut x: i16\n    y: i16\nfn nudge(point: &mut point) -> void:\n    point.x += point.y\nfn update(points: &mut [point]) -> void:\n    for point in &mut points:\n        nudge(&mut point)\nfn calculate() -> i16:\n    let mut points: point[2] = [point(1, 2), point(10, 20)]\n    update(&mut points)\n    return points[0].x + points[1].x\n",
    );

    let assembly = listing(&parsed(&source), "calculate", &O2());
    assert!(assembly.contains("call far ptr _update"));
    assert!(assembly.contains("call far ptr _nudge"));
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
        let text = format!("fn value(k: i16) -> i16:\n    let mut total: i16 = k\n    for i in 0..{trips}:\n        total = total + i\n    return total\n");
        let body = _settled(&directory, &text, &O2());
        !loops::loops(&body.blocks, Some(body.entry)).is_empty()
    };
    assert!(!rolled(16), "within the cap the loop is copied out");
    assert!(rolled(17));
}

#[test]
fn test_a_byte_argument_is_pushed_as_a_word() {
    // A u8 or char argument reached the push as `push al`, which the assembler rejects.
    // digit is exported so that the call is not inlined.
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "byte_argument.mod",
        "export \"cdecl16\":\n    fn digit(c: char) -> u8:\n        return u8(c) - u8('0')\nfn main() -> i16:\n    let c: char = '7'\n    return i16(digit(c))\n",
    );
    let assembly = listing(&parsed(&source), "main", &O2());
    assert!(assembly.contains("call far ptr _digit"));
    assert!(!Regex::new(r"push [abcd]l\b").unwrap().is_match(&assembly));
}

/// `while true:` branched on a constant, which lowering refused: "branch condition must be a value".
#[test]
fn test_a_branch_on_a_constant_lowers_as_a_jump() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "forever.mod",
        "fn main() -> i16:\n    let mut n = 3\n    while true:\n        if n == 0:\n            return 7\n        n -= 1\n    return 0\n",
    );
    lowered_named(&parsed(&source), "forever.main");
}

/// A vec borrowed as `&[T]` takes DGROUP's selector for its far data
/// pointer, which the object writer could not name: "KeyError: (grp, 0)".
#[test]
fn test_a_vec_view_names_dgroup_in_the_object() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "view.mod",
        "fn total(values: &[i16]) -> i16:\n    let mut sum = 0\n    for value in values:\n        sum += value\n    return sum\n\nfn main() -> i16:\n    let values = [x * x for x in [1, 2, 3]]\n    return total(values)\n",
    );
    modern_compile::written(&parsed(&source), "main", &source, &level("O2"))
        .expect("writes an object");
}

/// `v[0].bump()` passed the element's near pointer where `&mut T` is far:
/// the host ran it, DOS bumped whatever the stale segment pointed at.
#[test]
fn test_a_method_on_a_vec_element_takes_a_far_pointer() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "bump.mod",
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
        "each.mod",
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
        "interop.mod",
        "extern \"cdecl16\":\n    @link_name(\"_sum_all\")\n    fn total(values: *far i16, count: u16) -> i32\n\n\
         export \"cdecl16\":\n    fn weight(value: i16) -> i16:\n        return value * 2\n\n\
         fn main() -> i16:\n    let values: i16[2] = [1, 2]\n    unsafe:\n        return i16(total(&values, 2))\n",
    );
    let module = modern_compile::assembled(
        &parsed(&source),
        "main",
        ProfileOrName::Name("486"),
        &level("O2"),
    )
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
        "floats.mod",
        "fn unused(x: f32) -> i16:\n    return 1\n\nfn above(x: f32) -> i16:\n    if x > 1.0:\n        return i16(x)\n    return 0\n\nfn main() -> i16:\n    return above(2.5) + unused(1.5)\n",
    );
    modern_compile::written(&parsed(&source), "main", &source, &level("O2")).expect("writes an object");
}

#[test]
/// pascal16 pushes the first argument first, names symbols in upper case, and
/// the callee removes the arguments with `retf n`.
fn test_pascal_functions_push_in_order_and_clean_up_after_themselves() {
    let source = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/examples/pascal/levels.mod"));
    let module =
        modern_compile::assembled(&parsed(&source), "main", ProfileOrName::Name("486"), &level("O2")).expect("assembles");
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
        "half.mod",
        "export \"pascal16\":\n    fn half(value: f32, out: *near f32) -> f32:\n        return value / 2.0\n\nfn main() -> i16:\n    return 0\n",
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
        "scaled.mod",
        "extern \"pascal16\":\n    fn scale(value: f32) -> f32\n\nexport \"pascal16\":\n    fn twice(value: f32) -> f32:\n        unsafe:\n            return scale(value) * 2.0\n",
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
        "near.mod",
        "@repr(\"c16\", pack=1)\nstruct Pair:\n    low: u16\n    high: u16\n\nvar pair: Pair = Pair(low=1, high=2)\n\n\
         extern \"pascal16\":\n    fn take(pair: *near Pair) -> u16\n\nexport \"pascal16\":\n    fn give() -> u16:\n        unsafe:\n            return take(&pair)\n",
    );
    let text = listing_on(&parsed(&source), "main", &level("O2"), "486");
    assert!(between(&text, "GIVE proc far", "GIVE endp").contains("push offset"), "{text}");
}

#[test]
/// Any integer converts to a float: a parameter, a temporary, a constant, of
/// any width or sign. Only one already in memory in an x87 format did;
/// `f64(high)` of an i16 parameter was "integer-to-float conversion needs a place".
fn test_any_integer_operand_converts_to_a_float() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "floats.mod",
        "export \"pascal16\":\n    fn mixed(small: i8, byte: u8, word: u16, long: u32, high: i16) -> f64:\n        \
         return f64(high) + f64(small) + f64(byte) + f64(word) + f64(long) + f64(high + 1) + f64(u16(7))\n",
    );
    let text = listing_on(&parsed(&source), "main", &level("O2"), "486");
    let mixed = between(&text, "MIXED proc far", "MIXED endp");
    // u32 is loaded as a signed qword, u16 widened to a signed dword.
    assert!(mixed.contains("fild qword ptr") && mixed.contains("movzx eax, cx"), "{mixed}");
}

#[test]
/// Section 9.2: an aggregate of 4 bytes or less comes back in registers,
/// with no hidden slot pointer; a larger one still takes the slot.
fn test_small_aggregates_return_in_registers() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "r.mod",
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
    modern_compile::written(&program, "main", &source, &level("O2")).expect("writes an object");
}

#[test]
/// Points-to read a call's pointer result as the contents of the escaped
/// cells the call reads: a new vec's buffer took the frame array a view had
/// published, and its elements were written through SS, not DS.
fn test_a_call_result_does_not_point_into_a_frame_the_call_can_read() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "grow.mod",
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
        "checked.mod",
        "fn at(values: &[i16], i: u16) -> i16:\n    return values[i]\n\nfn main() -> i16:\n    let v: i16[3] = [1, 2, 3]\n    return at(&v, 1)\n",
    );
    modern_compile::written(&parsed(&source), "main", &source, &level("O2")).expect("writes an object");
}

#[test]
fn a_float_converts_to_every_integer_width() {
    // "i8: no floating storage format": no x87 store holds a byte, and a
    // `fistp word` cannot hold u16's top half.
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "convert.mod",
        "fn main() -> i16:\n    let x: f64 = 250.5\n    let y: f64 = 4000000000.0\n    print(f\"{u8(x)} {i8(x - 300.0)} {u16(x * 200.0)} {u32(y)} {i32(x)}\")\n    return 0\n",
    );
    let text = listing_on(&parsed(&source), "main", &level("Os"), crate::frontends::modern::compile::CPU);
    assert!(text.contains("fistp qword"), "{text}");
}

/// Parsing ran `cargo run --release` on this crate: after any edit, the first
/// test waited a minute for a release rebuild and every other one for its lock.
#[test]
fn test_parsing_runs_no_cargo() {
    if std::env::var_os("LLRM_NO_CARGO").is_some() {
        parsed(&fixture("fixed.mod"));
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
        "enumerate.mod",
        "fn main() -> i16:\n    let values: i16[3] = [7, 8, 9]\n    for (i, x) in enumerate(values):\n        print(f\"{i}: {x}\")\n    return 0\n",
    );
    let text = listing_on(&parsed(&source), "main", &level("Os"), crate::frontends::modern::compile::CPU);
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
    let text = listing(&parsed(&written(&directory, "helper.mod", HELPERS)), "main", &O2());
    assert!(!text.lines().any(|line| line.contains("call") && line.contains("_twice")), "{text}");
}

#[test]
fn a_call_inlined_away_leaves_no_extern() {
    // The call table outlived the inlined call: main declared
    // `extern _scaled:far` for a procedure the module no longer has.
    let directory = tempfile::tempdir().expect("a directory");
    let text = listing(&parsed(&written(&directory, "helper.mod", HELPERS)), "main", &O2());
    assert!(!text.contains("_scaled"), "{text}");
}

#[test]
fn test_each_procedure_has_a_code_segment_the_linker_may_drop() {
    // One segment held every procedure, so a program linked all of the
    // runtime even when it called one routine.
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(&directory, "two.mod", "export \"cdecl16\":\n    fn unused(x: i16) -> i16:\n        return x + 1\n\nfn main() -> i16:\n    print(3)\n    return 0\n");
    let object = modern_compile::written_as(&parsed(&source), "main", &source, &level("O2"), crate::backend::omfwrite::CodeLayout::PerProcedure).expect("writes");
    let records = crate::objectfile::omf::parse(&object).expect("parses");
    let segments = records.iter().filter(|one| one.r#type & 0xFE == crate::objectfile::omf::SEGDEF).count();
    // Two procedures, and _DATA.
    assert_eq!(segments, 3);
}

#[test]
fn test_an_object_defines_each_segment_once_unless_asked_for_one_per_procedure() {
    // A segment per procedure, all of one name, was the default: Microsoft
    // LINK 3.69 read them as one and refused SORTLIB.OBJ with L1103.
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(&directory, "two.mod", "export \"cdecl16\":\n    fn unused(x: i16) -> i16:\n        return x + 1\n\nfn main() -> i16:\n    print(3)\n    return 0\n");
    let object = modern_compile::written(&parsed(&source), "main", &source, &level("O2")).expect("writes");
    let records = crate::objectfile::omf::parse(&object).expect("parses");
    let segments = records.iter().filter(|one| one.r#type & 0xFE == crate::objectfile::omf::SEGDEF).count();
    // The code, and _DATA.
    assert_eq!(segments, 2);
}

#[test]
fn test_a_computed_float_argument_is_passed_through_memory() {
    // x87 cannot push: "floating instruction has no allocation rule".
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(&directory, "pushed.mod", "fn half(x: f64) -> f64:\n    return x / 2.0\n\nexport \"cdecl16\":\n    fn quarter(x: f32, y: f64) -> f64:\n        print(x * 2.0)\n        return half(y) / 2.0\n\nfn main() -> i16:\n    return 0\n");
    modern_compile::written(&parsed(&source), "main", &source, &level("O2")).expect("writes an object");
}

#[test]
/// Section 9.2: a far pointer comes back in dx:ax, where C and BASIC read
/// it. It came back in eax, so PDS's STRINGADDRESS result was misread.
fn test_a_far_pointer_result_travels_in_dx_ax() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "far.mod",
        "extern \"pascal16\":\n    fn address(of: *near u8) -> *far u8\n\nexport \"pascal16\":\n    fn first(bytes: *far u8) -> *far u8:\n        return bytes\n\n    \
         fn through(of: *near u8) -> u8:\n        unsafe:\n            let p = address(of)\n            return *p\n",
    );
    let text = listing_on(&parsed(&source), "main", &level("O2"), "486");
    let first = between(&text, "FIRST proc far", "FIRST endp");
    assert!(!first.contains("eax") && first.contains("mov dx,"), "{first}");
    let through = between(&text, "THROUGH proc far", "THROUGH endp");
    assert!(!through.contains("eax") && through.contains("es, dx"), "{through}");
}

#[test]
/// Section 15: a qb45 export takes BASIC's arguments first to last, each a
/// near pointer, and removes them; it links without the modern runtime.
fn test_a_qb45_library_takes_basic_arguments_by_reference() {
    let source = root().join("docs/examples/basic/sortlib.mod");
    let module = modern_compile::assembled(&parsed(&source), "main", ProfileOrName::Name("486"), &level("O2")).expect("assembles");
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
    // A rank-2 view asks for both dimensions' counts.
    let rows = between(&text, "ROWTOTAL proc far", "ROWTOTAL endp");
    assert!(rows.contains("pushw 1") && rows.contains("imul"), "{rows}");
    let initials = between(&text, "INITIALS proc far", "INITIALS endp");
    assert!(initials.contains("call far ptr _abi.qb45.string_result") && initials.contains("retf 2"), "{initials}");
}

#[test]
/// Section 15: BASIC gives a SINGLE or DOUBLE function a near pointer, pushed
/// last, to store its result through, and reads the pointer back from ax.
fn test_a_basic_float_result_goes_through_its_hidden_pointer() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(&directory, "half.mod", "export \"qb45\":\n    fn half(value: f64) -> f64:\n        return value / 2.0\n");
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
        "count.mod",
        "import abi.pds71 as pds\n\nexport \"pds71\":\n    fn Spaces(text: pds.StringRef) -> i16:\n        let mut count: i16 = 0\n        for letter in text:\n            if letter == ' ':\n                count += 1\n        return count\n",
    );
    let module = modern_compile::assembled(&parsed(&source), "main", ProfileOrName::Name("486"), &level("O2")).expect("assembles");
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
    let source = root().join("docs/examples/ticker.mod");
    let program = parsed(&source);
    let text = listing_on(&program, "main", &level("O2"), "486");
    let lines: Vec<&str> = between(&text, "_tick proc far", "_tick endp").lines().map(str::trim).collect();
    assert_eq!(
        lines[1..11],
        ["pushad", "push ds", "push es", "push fs", "push gs", "pushw DGROUP", "pop ds", "push ds", "pop es", "cld"],
        "{text}"
    );
    assert_eq!(lines[lines.len() - 6..], ["pop gs", "pop fs", "pop es", "pop ds", "popad", "iret"], "{text}");
    assert!(text.contains("dd _tick"), "{text}");
    modern_compile::written(&program, "main", &source, &level("O2")).expect("encodes");
}

#[test]
/// A variable a handler names changes under the program. The busy wait
/// read it once, before the loop, and spun forever.
fn test_a_variable_a_handler_names_is_read_on_every_pass() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "wait.mod",
        "var ticks: u16 = 0\n\nexport \"interrupt16\":\n    fn tick() -> void:\n        ticks += 1\n\n\
         fn main() -> i16:\n    while ticks < 36:\n        continue\n    return 0\n",
    );
    let text = listing(&parsed(&source), "main", &O2());
    assert!(between(&text, "_main proc", "_main endp").contains("cmp word ptr wait$D1, 36"), "{text}");
}

#[test]
/// Nothing calls a handler, and an interrupt passes it nothing.
fn test_an_interrupt_handler_takes_nothing_and_is_not_called() {
    let directory = tempfile::tempdir().expect("a directory");
    let taking = written(&directory, "taking.mod", "export \"interrupt16\":\n    fn tick(n: i16) -> void:\n        return\n\nfn main() -> i16:\n    return 0\n");
    assert!(refused(&taking).contains("takes nothing and returns void"));
    let called = written(&directory, "called.mod", "export \"interrupt16\":\n    fn tick() -> void:\n        return\n\nfn main() -> i16:\n    tick()\n    return 0\n");
    assert!(refused(&called).contains("only an interrupt enters it"));
}

#[test]
/// A byte parameter passed on to a call took a fresh value numbered as the
/// parameter was: "value#1 is defined 2 times".
fn test_a_byte_parameter_passed_on_to_a_call_compiles() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "byte.mod",
        "extern \"cdecl16\":\n    fn put(number: u8) -> void\n\nfn set(number: u8) -> void:\n    unsafe:\n        put(number)\n\n\
         fn main() -> i16:\n    set(28)\n    set(29)\n    return 0\n",
    );
    let text = listing(&parsed(&source), "main", &O2());
    assert!(between(&text, "_set proc", "_set endp").contains("movzx"), "{text}");
}

#[test]
/// Section 9.2: a far pointer comes back in dx:ax. The caller read eax,
/// so a DOS vector the runtime returned lost its segment.
fn test_a_far_pointer_result_comes_back_in_dx_ax() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "far.mod",
        "extern \"cdecl16\":\n    fn get(number: u16) -> *far u8\n\nvar kept: *far u8 = 0\n\n\
         fn main() -> i16:\n    unsafe:\n        kept = get(3)\n    return 0\n",
    );
    let text = listing(&parsed(&source), "main", &O2());
    assert!(text.contains("mov word ptr far$D1+2, dx"), "{text}");
}

/// An inline block is its bytes in place of a call, fed and read in the
/// registers it names: `a` goes to both cx and dx, `b` and 7 are packed into
/// ax, and `sum` and `high` come out of bx and ch.
#[test]
fn test_inline_assembly_is_its_bytes_between_its_register_constraints() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "blocks.mod",
        "fn mix(a: u16, b: u8) -> u16:\n    let mut high: u8 = 0\n    unsafe:\n        \
         asm(cx=a, dx=a, al=b, ah=7, out=(bx=let sum, ch=high), clobbers=[flags]):\n            \
         mov bx, cx\n            add bx, dx\n            add bl, al\n        return sum + a + u16(high)\n\n\
         fn five() -> u16:\n    return 5\n\n\
         fn main() -> i16:\n    return i16(mix(3, 4) + mix(five(), 9))\n",
    );
    let assembly = listing(&parsed(&source), "main", &O2());
    let mix = between(&assembly, "_mix proc far", "_mix endp");
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
        let assembly = listing(&parsed(&written(&directory, "poke.mod", text)), "main", &O2());
        between(&assembly, "db 0c6h,044h,003h,007h", "_poke endp").contains("byte ptr poke$D1+3")
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
        "pair.mod",
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
        "spin.mod",
        "fn spin(n: u16) -> u16:\n    let mut total: u16 = 0\n    let mut i: u16 = 0\n    while i < n:\n        \
         unsafe:\n            asm(bx=i, out=(cx=let got), clobbers=[]):\n                mov cx, bx\n            \
         total += got\n        i += 1\n    return total\n\nfn main() -> i16:\n    return i16(spin(3))\n",
    );
    let assembly = listing(&parsed(&source), "main", &O2());
    let after: Vec<&str> = assembly.split("db 089h,0d9h\n").skip(1).map(|rest| rest.lines().next().unwrap_or("")).collect();
    assert_eq!(after.len(), 3, "{assembly}");
    assert!(after.iter().all(|line| line.ends_with(", cx")), "{assembly}");
}

#[test]
fn test_an_export_no_object_uses_is_dropped_with_what_only_it_calls() {
    // jwlink's `option eliminate` keeps a segment any other references, even
    // one it drops: every program carried the unused float printer, 2.8 KB.
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(&directory, "lib.mod", "fn helper(x: u16) -> u16:\n    let mut total: u16 = 0\n    for i in range(0, x):\n        total += i * x\n    return total\n\nexport \"cdecl16\":\n    @link_name(\"M$ZA\")\n    fn a(x: u16) -> u16:\n        return helper(x) + 1\n\n    @link_name(\"M$ZB\")\n    fn b(x: u16) -> u16:\n        return x * 2\n");
    let mut program = parsed(&source);
    modern_compile::keep_exports(&mut program, &["M$ZB".to_owned()].into_iter().collect());
    let module = modern_compile::assembled(&program, "main", ProfileOrName::Name("486"), &level("O2")).expect("assembles");
    assert_eq!(module.publics, ["M$ZB"]);
    assert_eq!(module.procedures.len(), 1, "{:?}", module.procedures.iter().map(|one| &one.name).collect::<Vec<_>>());
}

#[test]
fn test_an_error_in_an_imported_module_names_that_module() {
    // Semantic errors carried no module: one in shapes.mod was reported at main.mod's line.
    let directory = tempfile::tempdir().expect("a directory");
    written(&directory, "shapes.mod", "pub fn area(w: i16, h: u16) -> i16:\n    return w * h\n");
    let main = written(&directory, "main.mod", "import shapes\n\nfn main() -> i16:\n    return shapes.area(2, 3)\n");
    let (path, error) = super::compile_file(&main).expect_err("refused");
    assert!(path.ends_with("shapes.mod"), "{} {}", path.display(), error.message);
    assert_eq!(error.span.line, 2);
}

#[test]
fn test_a_borrowed_fixed_array_is_a_far_pointer_with_no_descriptor() {
    // `&i16[4]` was passed as a view: a descriptor pointer, its length loaded at run time.
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "fixed_borrow.mod",
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
