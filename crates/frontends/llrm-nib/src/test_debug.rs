//! `-g` on the Nib path: what CodeView reads of a compiled module.

use std::path::Path;
use std::rc::Rc;

use llrm_core::backend::objbuild::CodeLayout;
use llrm_core::objectfile::{cv4info, omf};

const SOURCE: &str = "var counter: i16 = 5
var total: u32 = 70000

struct point:
    x: i16
    y: i32

fn scale(p: point, factor: i16) -> i32:
    let doubled: i32 = p.y * 2
    return doubled + factor

fn main() -> i16:
    let mut values: i16[4] = [1, 2, 3, 4]
    let origin = point(3, 40000)
    let small: u8 = 200
    let ratio: f32 = 1.5
    counter += values[2]
    let result = scale(origin, counter)
    if result > 0:
        print(\"ok\")
    return counter
";

fn object() -> Vec<Rc<omf::Record>> {
    compiled(false)
}

fn compiled(inlined: bool) -> Vec<Rc<omf::Record>> {
    let directory = tempfile::tempdir().expect("creates a directory");
    let path = directory.path().join("probe.nib");
    std::fs::write(&path, SOURCE).expect("writes");
    let frontend = crate::Frontend { debug: true, ..crate::real_mode() };
    let program = crate::driver::parsed(&path, &frontend, None).expect("parses");
    // Unless asked, not inlined: `scale` is a symbol and its lines are statements to read.
    let threshold = if inlined { llrm_transforms::inline::Threshold::default() } else { llrm_transforms::inline::Threshold::none() };
    let pipeline = llrm_transforms::pipeline::Options { inline: threshold, ..Default::default() };
    let options = llrm_core::driver::Options { pipeline, ..llrm_driver::m16_options(crate::compile::machine(&llrm_x86_m16::M16, &crate::real_mode().os)) };
    let module = crate::compile::assembled(&program, "main", &options, &crate::real_mode().os).expect("compiles");
    omf::parse(&crate::compile::object(&module, Path::new("probe.nib"), CodeLayout::OneSegment, llrm_target::object::Format::Omf).expect("writes")).expect("parses")
}

/// Each source local and module variable, and each parameter (a stack one an argument, a register one a cell of the frame), with its Nib type; no
/// compiler temporary.
#[test]
fn nib_symbols_read_with_their_types() {
    assert_eq!(
        cv4info::shape(&object()),
        [
            "DATA counter: SHORT",
            "DATA total: UNSIGNED LONG",
            "LOCAL main.origin: struct point {x +0 SHORT, y +2 LONG}",
            "LOCAL main.ratio: REAL32",
            "LOCAL main.result: LONG",
            "LOCAL main.small: UNSIGNED CHAR",
            "LOCAL main.values: 8 BYTES OF SHORT",
            "LOCAL scale.doubled: LONG",
            "LOCAL scale.factor: SHORT",
            // `scale`'s parameters arrive in registers (regparm3), stored to a cell at the entry (#883): locals of the frame. A struct
            // passed by value travels as a far pointer to it.
            "LOCAL scale.p: FAR * struct point {x +0 SHORT, y +2 LONG}",
            "PROC main far () -> SHORT",
            "PROC scale near (FAR * struct point {x +0 SHORT, y +2 LONG}, SHORT) -> LONG",
            "UDT point: struct point {x +0 SHORT, y +2 LONG}",
        ]
    );
}

/// Each statement's line.
#[test]
fn nib_lines_are_its_statements() {
    let lines: Vec<u16> = object().iter().filter(|one| one.r#type == omf::LINNUM).flat_map(|one| omf::lines(one).1).map(|(line, _)| line).collect();
    assert_eq!(lines, [9, 10, 13, 14, 15, 16, 17, 18, 19, 20, 21]);
}

/// `scale` inlined into `main` has no symbols of its own; its statements,
/// lines 9 and 10, are numbered where its code now is, between `main`'s.
#[test]
fn nib_inlined_code_keeps_its_lines_and_loses_its_symbols() {
    let object = compiled(true);
    let lines: Vec<u16> = object.iter().filter(|one| one.r#type == omf::LINNUM).flat_map(|one| omf::lines(one).1).map(|(line, _)| line).collect();
    assert_eq!(lines, [13, 14, 15, 16, 17, 9, 10, 18, 19, 20, 21]);
    let shape = cv4info::shape(&object);
    assert!(shape.iter().all(|one| !one.contains("scale")), "{shape:?}");
    assert!(shape.contains(&"PROC main far () -> SHORT".to_owned()), "{shape:?}");
}

/// The model of the Nib `source` compiled for real mode, nothing inlined.
fn model_of(source: &str) -> llrm_object::debug::Info {
    let directory = tempfile::tempdir().expect("creates a directory");
    let path = directory.path().join("probe.nib");
    std::fs::write(&path, source).expect("writes");
    let frontend = crate::Frontend { debug: true, ..crate::real_mode() };
    let program = crate::driver::parsed(&path, &frontend, None).expect("parses");
    let pipeline = llrm_transforms::pipeline::Options { inline: llrm_transforms::inline::Threshold::none(), ..Default::default() };
    let options = llrm_core::driver::Options { pipeline, ..llrm_driver::m16_options(crate::compile::machine(&llrm_x86_m16::M16, &crate::real_mode().os)) };
    let module = crate::compile::assembled(&program, "main", &options, &crate::real_mode().os).expect("compiles");
    llrm_core::backend::objbuild::built(&module, "probe.nib", CodeLayout::OneSegment).expect("builds").debug.expect("-g's information")
}

/// `gcd` is only ever called with two constants, so the optimiser takes both parameters out of it. They were
/// dropped from the debug information with nothing said: a debugger had no `x` or `y` to show, not even to say
/// they are optimized out. They are in the model with no location.
#[test]
fn a_parameter_the_optimiser_removed_is_in_the_model_with_no_location() {
    use llrm_object::debug::{Kind, Location};
    const GCD: &str = "fn gcd(x: i16, y: i16) -> i16:
    let mut a = x
    let mut b = y
    while b != 0:
        let t = a % b
        a = b
        b = t
    return a

fn main() -> i16:
    print(gcd(1071, 462))
    return 0
";
    let info = model_of(GCD);
    let gcd = info.functions.iter().find(|one| one.name == "gcd").expect("gcd");
    for name in ["x", "y"] {
        let parameter = gcd.variables.iter().find(|one| one.name == name).unwrap_or_else(|| panic!("no parameter {name}: {:?}", gcd.variables));
        assert_eq!((parameter.kind, &parameter.location), (Kind::Parameter, &Location::List(Vec::new())), "{name}");
    }
    // The locals a, b and t, in cells as before.
    assert!(["a", "b", "t"].iter().all(|name| gcd.variables.iter().any(|one| one.name == *name && matches!(one.location, Location::Frame { .. }))));
}

const TWO: &str = "fn add(a: i16, b: i32) -> i32:\n    return i32(a) + b\n\nfn main() -> i16:\n    print(add(2, 3) + add(4, 5))\n    return 0\n";

/// The model of the Nib `source` compiled for real mode, nothing inlined, with `flags` (`procedure_segments`: a code segment each).
fn segmented_model(source: &str, procedure_segments: bool) -> llrm_object::debug::Info {
    let directory = tempfile::tempdir().expect("creates a directory");
    let path = directory.path().join("probe.nib");
    std::fs::write(&path, source).expect("writes");
    let frontend = crate::Frontend { debug: true, ..crate::real_mode() };
    let program = crate::driver::parsed(&path, &frontend, None).expect("parses");
    let pipeline = llrm_transforms::pipeline::Options { inline: llrm_transforms::inline::Threshold::none(), ..Default::default() };
    let options = llrm_core::driver::Options { pipeline, ..llrm_driver::m16_options(crate::compile::machine(&llrm_x86_m16::M16, &crate::real_mode().os)) };
    let module = crate::compile::assembled(&program, "main", &options, &crate::real_mode().os).expect("compiles");
    let layout = if procedure_segments { CodeLayout::PerProcedure } else { CodeLayout::OneSegment };
    llrm_core::backend::objbuild::built(&module, "probe.nib", layout).expect("builds").debug.expect("-g's information")
}

/// `-g` with a code segment per procedure was refused, for the one-segment form of the BASIC dialect; CodeView 4 has the segment in
/// each address. Each function is in the segment its code is, and the unit's code is each segment's.
#[test]
fn cv4_describes_a_program_with_a_code_segment_per_procedure() {
    let info = segmented_model(TWO, true);
    let sections: Vec<(&str, usize)> = info.functions.iter().map(|one| (one.name.as_str(), one.ranges[0].section)).collect();
    assert!(sections.windows(2).all(|pair| pair[0].1 != pair[1].1), "a segment each: {sections:?}");
    assert!(info.code.len() >= 2, "{:?}", info.code);
    let one = segmented_model(TWO, false);
    // The same functions with the same variables, in one segment.
    let shape = |info: &llrm_object::debug::Info| info.functions.iter().map(|one| (one.name.clone(), one.variables.len())).collect::<Vec<_>>();
    assert_eq!(shape(&info), shape(&one));
}

const ADD: &str = "fn add(a: i16, b: i32) -> i32:
    return i32(a) + b

fn main() -> i16:
    print(add(2, 3) + add(4, 5))
    return 0
";

/// The model of `ADD` under `-mabi=regparm3`, its debug format `format`.
fn regparm_model(format: llrm_object::debug::Format) -> (llrm_object::debug::Info, Vec<u8>) {
    let directory = tempfile::tempdir().expect("creates a directory");
    let path = directory.path().join("probe.nib");
    std::fs::write(&path, ADD).expect("writes");
    let frontend = crate::Frontend { debug: true, native_name: "regparm3".into(), ..crate::real_mode() };
    let program = crate::driver::parsed(&path, &frontend, None).expect("parses");
    let pipeline = llrm_transforms::pipeline::Options { inline: llrm_transforms::inline::Threshold::none(), ..Default::default() };
    let options = llrm_core::driver::Options { pipeline, abi: Some("regparm3".into()), debug_format: format, ..llrm_driver::m16_options(crate::compile::machine(&llrm_x86_m16::M16, &crate::real_mode().os)) };
    let module = crate::compile::assembled(&program, "main", &options, &crate::real_mode().os).expect("compiles");
    let object = llrm_core::backend::objbuild::built(&module, "probe.nib", CodeLayout::OneSegment).expect("builds");
    let code = object.sections.iter().find(|one| one.role == llrm_object::Role::Text).expect("code").image.clone();
    (object.debug.expect("-g's information"), code)
}

/// Under `-mabi=regparm3` a parameter arrives in a register, and CodeView 4's BASIC-era records (and Turbo Debugger's) name one place for
/// a whole scope: `add`'s `a` and `b` were in no CodeView at all. The backend stores each to a cell at the entry and describes the
/// cell, so the debugger stopped at the first line reads the value there; the first line starts after the stores.
#[test]
fn a_register_parameter_is_stored_to_a_cell_at_the_entry_where_the_format_names_one_place() {
    use llrm_object::debug::{Kind, Location};
    let (info, code) = regparm_model(llrm_object::debug::Format::Default);
    let add = info.functions.iter().find(|one| one.name == "add").expect("add");
    let cells: Vec<(&str, Kind, &Location)> = add.variables.iter().map(|one| (one.name.as_str(), one.kind, &one.location)).collect();
    assert_eq!(cells, [("a", Kind::Parameter, &Location::Frame { disp: -2 }), ("b", Kind::Parameter, &Location::Frame { disp: -6 })], "{cells:?}");
    // `mov [bp-2], ax` is 89 46 FE: it ends before the body begins.
    let at = code.windows(3).position(|bytes| bytes == [0x89, 0x46, 0xFE]).expect("the store of a");
    assert!(at + 3 <= add.body.expect("a body").0, "the store at {at} comes before the body at {:?}", add.body);
    // The reader sees both, as locals: it tells a parameter from a local by the sign of the offset from BP, and the cells are below it.
    let shape = cv4info::shape(&compiled_regparm());
    assert!(shape.contains(&"LOCAL add.a: SHORT".to_owned()) && shape.contains(&"LOCAL add.b: LONG".to_owned()), "{shape:#?}");
}

fn compiled_regparm() -> Vec<Rc<omf::Record>> {
    let directory = tempfile::tempdir().expect("creates a directory");
    let path = directory.path().join("probe.nib");
    std::fs::write(&path, ADD).expect("writes");
    let frontend = crate::Frontend { debug: true, native_name: "regparm3".into(), ..crate::real_mode() };
    let program = crate::driver::parsed(&path, &frontend, None).expect("parses");
    let pipeline = llrm_transforms::pipeline::Options { inline: llrm_transforms::inline::Threshold::none(), ..Default::default() };
    let options = llrm_core::driver::Options { pipeline, abi: Some("regparm3".into()), ..llrm_driver::m16_options(crate::compile::machine(&llrm_x86_m16::M16, &crate::real_mode().os)) };
    let module = crate::compile::assembled(&program, "main", &options, &crate::real_mode().os).expect("compiles");
    omf::parse(&crate::compile::object(&module, Path::new("probe.nib"), CodeLayout::OneSegment, llrm_target::object::Format::Omf).expect("writes")).expect("parses")
}

/// Where the debug format can say it (DWARF's location lists), the parameter stays in its register until the code stores it: no cell
/// is made for the debugger, and the code is the code of a build without `-g`.
#[test]
fn where_the_format_says_ranges_a_register_parameter_stays_in_its_register() {
    use llrm_object::debug::Location;
    let (info, _) = regparm_model(llrm_object::debug::Format::Dwarf { version: 5 });
    let add = info.functions.iter().find(|one| one.name == "add").expect("add");
    assert!(add.variables.iter().all(|one| matches!(&one.location, Location::List(entries) if matches!(entries[..], [(_, Location::Register(_))]))), "{:?}", add.variables);
}
