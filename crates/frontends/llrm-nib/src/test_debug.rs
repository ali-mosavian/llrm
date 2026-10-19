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
    // Unless asked, not inlined: `scale` is a symbol and its lines are
    // statements to read.
    let threshold = if inlined {
        llrm_transforms::inline::Threshold::default()
    } else {
        llrm_transforms::inline::Threshold::none()
    };
    // Not optimised unless it inlines: a variable the optimiser keeps in no
    // cell is left out of CodeView 4, which is what these read.
    let pipeline = llrm_transforms::pipeline::Options { inline: threshold, optimize: inlined, ..Default::default() };
    let options = llrm_core::driver::Options {
        pipeline,
        ..llrm_driver::m16_options(crate::compile::machine(&llrm_x86_m16::M16, &crate::real_mode().os))
    };
    let module = crate::compile::assembled(&program, "main", &options, &crate::real_mode().os).expect("compiles");
    omf::parse(
        &crate::compile::object(
            &module,
            Path::new("probe.nib"),
            CodeLayout::OneSegment,
            llrm_target::object::Format::Omf,
        )
        .expect("writes"),
    )
    .expect("parses")
}

/// Each source local and module variable, and each parameter (a stack one an
/// argument, a register one a cell of the frame), with its Nib type; no
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
            // `scale`'s second parameter arrives in a register (regparm3),
            // which CodeView 4 cannot say for a whole scope, and `-g` stores
            // nothing to a cell for it: it is left out. The first is on the
            // stack, an argument.
            "PARAM scale.p: FAR * struct point {x +0 SHORT, y +2 LONG}",
            "PROC main far () -> SHORT",
            "PROC scale near (FAR * struct point {x +0 SHORT, y +2 LONG}, SHORT) -> LONG",
            "UDT point: struct point {x +0 SHORT, y +2 LONG}",
        ]
    );
}

/// Each statement's line.
#[test]
fn nib_lines_are_its_statements() {
    let lines: Vec<u16> = object()
        .iter()
        .filter(|one| one.r#type == omf::LINNUM)
        .flat_map(|one| omf::lines(one).1)
        .map(|(line, _)| line)
        .collect();
    assert_eq!(lines, [9, 10, 13, 14, 15, 16, 17, 18, 19, 20, 21]);
}

/// `scale` inlined into `main` has no symbols of its own; its statements,
/// lines 9 and 10, are numbered where its code now is, between `main`'s.
#[test]
fn nib_inlined_code_keeps_its_lines_and_loses_its_symbols() {
    let object = compiled(true);
    let lines: Vec<u16> = object
        .iter()
        .filter(|one| one.r#type == omf::LINNUM)
        .flat_map(|one| omf::lines(one).1)
        .map(|(line, _)| line)
        .collect();
    // Optimised, a statement whose code is gone has no line. `counter` is 5 and
    // `values[2]` is 3, so `result > 0` is known: the call's `return doubled +
    // factor` (10) and the `if` (19) fold away and `print` follows
    // unconditionally; what is left is `counter += 3` (17), `print` (20) and
    // the `return` (21). Lines 10 and 19 had code while `result > 0` stayed a
    // compare and a branch.
    assert_eq!(lines, [17, 20, 21]);
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
    let pipeline =
        llrm_transforms::pipeline::Options { inline: llrm_transforms::inline::Threshold::none(), ..Default::default() };
    let options = llrm_core::driver::Options {
        pipeline,
        ..llrm_driver::m16_options(crate::compile::machine(&llrm_x86_m16::M16, &crate::real_mode().os))
    };
    let module = crate::compile::assembled(&program, "main", &options, &crate::real_mode().os).expect("compiles");
    llrm_core::backend::objbuild::built(&module, "probe.nib", CodeLayout::OneSegment)
        .expect("builds")
        .debug
        .expect("-g's information")
}

/// `gcd` is only ever called with two constants, so the optimiser takes both
/// parameters out of it. They were dropped from the debug information with
/// nothing said: a debugger had no `x` or `y` to show, not even to say they are
/// optimized out. They are in the model with no location.
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
        let parameter = gcd
            .variables
            .iter()
            .find(|one| one.name == name)
            .unwrap_or_else(|| panic!("no parameter {name}: {:?}", gcd.variables));
        assert_eq!((parameter.kind, &parameter.location), (Kind::Parameter, &Location::List(Vec::new())), "{name}");
    }
    // The locals a, b and t were kept in cells for a debugger; the optimiser
    // keeps them in registers, which CodeView 4 cannot say, so they are
    // left out.
    assert!(["a", "b", "t"].iter().all(|name| {
        !gcd.variables.iter().any(|one| one.name == *name && matches!(one.location, Location::Frame { .. }))
    }));
}

const TWO: &str = "fn add(a: i16, b: i32) -> i32:\n    return i32(a) + b\n\nfn main() -> i16:\n    print(add(2, 3) + add(4, 5))\n    return 0\n";

/// The model of the Nib `source` compiled for real mode, nothing inlined, with
/// `flags` (`procedure_segments`: a code segment each).
fn segmented_model(
    source: &str,
    procedure_segments: bool,
) -> llrm_object::debug::Info {
    let directory = tempfile::tempdir().expect("creates a directory");
    let path = directory.path().join("probe.nib");
    std::fs::write(&path, source).expect("writes");
    let frontend = crate::Frontend { debug: true, ..crate::real_mode() };
    let program = crate::driver::parsed(&path, &frontend, None).expect("parses");
    let pipeline =
        llrm_transforms::pipeline::Options { inline: llrm_transforms::inline::Threshold::none(), ..Default::default() };
    let options = llrm_core::driver::Options {
        pipeline,
        ..llrm_driver::m16_options(crate::compile::machine(&llrm_x86_m16::M16, &crate::real_mode().os))
    };
    let module = crate::compile::assembled(&program, "main", &options, &crate::real_mode().os).expect("compiles");
    let layout = if procedure_segments { CodeLayout::PerProcedure } else { CodeLayout::OneSegment };
    llrm_core::backend::objbuild::built(&module, "probe.nib", layout).expect("builds").debug.expect("-g's information")
}

/// `-g` with a code segment per procedure was refused, for the one-segment form
/// of the BASIC dialect; CodeView 4 has the segment in each address. Each
/// function is in the segment its code is, and the unit's code is each
/// segment's.
#[test]
fn cv4_describes_a_program_with_a_code_segment_per_procedure() {
    let info = segmented_model(TWO, true);
    let sections: Vec<(&str, usize)> =
        info.functions.iter().map(|one| (one.name.as_str(), one.ranges[0].section)).collect();
    assert!(sections.windows(2).all(|pair| pair[0].1 != pair[1].1), "a segment each: {sections:?}");
    assert!(info.code.len() >= 2, "{:?}", info.code);
    let one = segmented_model(TWO, false);
    // The same functions with the same variables, in one segment.
    let shape = |info: &llrm_object::debug::Info| {
        info.functions.iter().map(|one| (one.name.clone(), one.variables.len())).collect::<Vec<_>>()
    };
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
    regparm_model_of(ADD, format)
}

/// The model of the Nib `source` under `-mabi=regparm3`, and its code.
fn regparm_model_of(
    source: &str,
    format: llrm_object::debug::Format,
) -> (llrm_object::debug::Info, Vec<u8>) {
    let directory = tempfile::tempdir().expect("creates a directory");
    let path = directory.path().join("probe.nib");
    std::fs::write(&path, source).expect("writes");
    let frontend = crate::Frontend { debug: true, native_name: "regparm3".into(), ..crate::real_mode() };
    let program = crate::driver::parsed(&path, &frontend, None).expect("parses");
    let pipeline =
        llrm_transforms::pipeline::Options { inline: llrm_transforms::inline::Threshold::none(), ..Default::default() };
    let options = llrm_core::driver::Options {
        pipeline,
        abi: Some("regparm3".into()),
        debug_format: format,
        ..llrm_driver::m16_options(crate::compile::machine(&llrm_x86_m16::M16, &crate::real_mode().os))
    };
    let module = crate::compile::assembled(&program, "main", &options, &crate::real_mode().os).expect("compiles");
    let object = llrm_core::backend::objbuild::built(&module, "probe.nib", CodeLayout::OneSegment).expect("builds");
    let code = object.sections.iter().find(|one| one.role == llrm_object::Role::Text).expect("code").image.clone();
    (object.debug.expect("-g's information"), code)
}

/// Under `-mabi=regparm3` a parameter arrives in a register. A format that
/// names one place for a whole scope (CodeView 4, Turbo Debugger) says it where
/// the code keeps it from its first value to the body's end: `add`'s `b` stays
/// in EDX the whole function, `a` is overwritten by the first instruction and
/// has no such place. The entry said only that each was in its register until
/// the body starts (one byte), so `b` was in no CodeView; the tracking that
/// follows a variable the code moves says for how long now. `-g` changes no
/// code: nothing stores either to a cell.
#[test]
fn a_register_parameter_is_said_where_the_code_keeps_it_for_the_whole_body() {
    use llrm_object::debug::{Kind, Location};
    let (info, code) = regparm_model(llrm_object::debug::Format::Default);
    let add = info.functions.iter().find(|one| one.name == "add").expect("add");
    let places: Vec<(&str, Kind, &Location)> =
        add.variables.iter().map(|one| (one.name.as_str(), one.kind, &one.location)).collect();
    let whole = add.ranges[0];
    assert!(
        matches!(&places[..], [("a", Kind::Parameter, Location::List(a)), ("b", Kind::Parameter, Location::List(b))]
            if a.is_empty() && matches!(&b[..], [(range, Location::Register(register))] if *range == whole && register == "edx")),
        "{places:?}"
    );
    // `mov [bp-2], ax` is 89 46 FE: nothing stores it.
    assert!(!code.windows(3).any(|bytes| bytes == [0x89, 0x46, 0xFE]), "the store of a is in the code");
    let shape = cv4info::shape(&compiled_regparm());
    assert!(shape.iter().any(|one| one == "REGISTER add.b: LONG in register 19"), "{shape:#?}");
    assert!(!shape.iter().any(|one| one.contains("add.a")), "{shape:#?}");
}

const LOOP: &str = "fn sum(n: i16, step: i16) -> i16:
    let mut total = 0
    let mut i = 0
    while i < n:
        total = total * 3 + step
        i = i + 1
    return total

fn main() -> i16:
    print(sum(3, 4) + sum(5, 6) + sum(1000, 7))
    return 0
";

/// A parameter kept in a register across a loop: `step` (DX) is there the whole
/// function, the one place a format of one place can say; `n` is copied out of
/// its register at the entry, so it has no place of one register, and a format
/// with ranges says each place it has.
#[test]
fn a_register_parameter_kept_across_a_loop_has_its_place_and_one_that_moves_has_its_ranges() {
    use llrm_object::debug::Location;
    let (info, _) = regparm_model_of(LOOP, llrm_object::debug::Format::Default);
    let sum = info.functions.iter().find(|one| one.name == "sum").expect("sum");
    let step = sum.variables.iter().find(|one| one.name == "step").expect("step");
    assert!(
        matches!(&step.location, Location::List(entries) if matches!(&entries[..], [(range, Location::Register(register))]
            if *range == sum.ranges[0] && register == "dx")),
        "{:?}",
        step.location
    );
    let n = sum.variables.iter().find(|one| one.name == "n").expect("n");
    assert!(matches!(&n.location, Location::List(entries) if entries.is_empty()), "{:?}", n.location);

    let (info, _) = regparm_model_of(LOOP, llrm_object::debug::Format::Dwarf { version: 5 });
    let sum = info.functions.iter().find(|one| one.name == "sum").expect("sum");
    let n = sum.variables.iter().find(|one| one.name == "n").expect("n");
    let Location::List(entries) = &n.location else { panic!("{:?}", n.location) };
    // The loop counts by another value (`neg ax` at the entry) and nothing
    // reads `n` inside it: ax is `n` up to that. On the path that skips the
    // loop the next instruction (`mov ax, bx`) overwrites ax at once, so that
    // path has no range of its own; in the loop it is optimized out, as a
    // debugger shows a value the code no longer holds.
    assert!(
        matches!(&entries[..], [(first, Location::Register(one))] if one == "ax" && first.offset == 0),
        "{entries:?}"
    );
    let end = entries.iter().map(|(range, _)| range.offset + range.length).max().expect("a range");
    assert!(end <= sum.ranges[0].offset + sum.ranges[0].length, "{entries:?}");
}

fn compiled_regparm() -> Vec<Rc<omf::Record>> {
    let directory = tempfile::tempdir().expect("creates a directory");
    let path = directory.path().join("probe.nib");
    std::fs::write(&path, ADD).expect("writes");
    let frontend = crate::Frontend { debug: true, native_name: "regparm3".into(), ..crate::real_mode() };
    let program = crate::driver::parsed(&path, &frontend, None).expect("parses");
    let pipeline =
        llrm_transforms::pipeline::Options { inline: llrm_transforms::inline::Threshold::none(), ..Default::default() };
    let options = llrm_core::driver::Options {
        pipeline,
        abi: Some("regparm3".into()),
        ..llrm_driver::m16_options(crate::compile::machine(&llrm_x86_m16::M16, &crate::real_mode().os))
    };
    let module = crate::compile::assembled(&program, "main", &options, &crate::real_mode().os).expect("compiles");
    omf::parse(
        &crate::compile::object(
            &module,
            Path::new("probe.nib"),
            CodeLayout::OneSegment,
            llrm_target::object::Format::Omf,
        )
        .expect("writes"),
    )
    .expect("parses")
}

/// Where the debug format can say it (DWARF's location lists), the parameter
/// stays in its register until the code stores it: no cell is made for the
/// debugger, and the code is the code of a build without `-g`.
#[test]
fn where_the_format_says_ranges_a_register_parameter_stays_in_its_register() {
    use llrm_object::debug::Location;
    let (info, _) = regparm_model(llrm_object::debug::Format::Dwarf { version: 5 });
    let add = info.functions.iter().find(|one| one.name == "add").expect("add");
    assert!(add.variables.iter().all(|one| matches!(&one.location, Location::List(entries) if matches!(entries[..], [(_, Location::Register(_))]))), "{:?}", add.variables);
}
