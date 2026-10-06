//! `-g` on the C path: what CodeView reads of a unit wccq recorded under -d2.

use std::path::Path;
use std::rc::Rc;

use llrm_core::backend::objbuild;
use llrm_core::objectfile::{cvinfo, omf};

/// tests/fixtures/c/debug.cgs, recorded from debug.c (and its debug.h)
/// with -d2, compiled as the CLI compiles it.
fn object() -> Vec<Rc<omf::Record>> {
    object_of("debug")
}

/// tests/fixtures/c/`name`.cgs compiled as the CLI compiles it.
fn object_of(name: &str) -> Vec<Rc<omf::Record>> {
    let text = std::fs::read_to_string(Path::new(env!("LLRM_ROOT")).join(format!("tests/fixtures/c/{name}.cgs"))).expect("reads");
    let machine = llrm_core::abi::machine::Machine { cpu: "386".to_owned(), ..llrm_core::abi::machine::BUILT_IN.clone() };
    // Not inlined: `twice` is a symbol to read.
    let pipeline = llrm_transforms::pipeline::Options { inline: llrm_transforms::inline::Threshold::new(0), ..Default::default() };
    let options = llrm_core::driver::Options { pipeline, ..llrm_driver::m16_options(machine) };
    let built = super::compile::selected(&text, name, None, &options).expect("compiles");
    omf::parse(&objbuild::written(&built, &format!("{name}.c")).expect("writes")).expect("parses")
}

/// Every parameter, local, static and global with its C type; `(void)`
/// is no parameter, and its only one was `void`.
#[test]
fn c_symbols_read_with_their_types() {
    let shape = cvinfo::parse(&object()).shape();
    assert_eq!(
        shape,
        [
            "DATA ga: 20 BYTES OF INTEGER",
            "DATA gfp: BYREF CHAR",
            "DATA ghp: BYREF INTEGER",
            "DATA gp: TYPE pt {x +0 INTEGER, y +2 LONG}",
            "DATA gu: TYPE mix {b +0 UNSIGNED CHAR, w +0 UNSIGNED SHORT}",
            "DATA gul: UNSIGNED LONG",
            "DATA st: INTEGER",
            "LOCAL f.a: INTEGER",
            "LOCAL f.b: BYREF TYPE pt",
            "LOCAL f.c: CHAR",
            "LOCAL f.l: INTEGER",
            "LOCAL twice.x: INTEGER",
            "PROC f flags 4 (INTEGER, BYREF TYPE pt, CHAR) -> LONG",
            // Static: near.
            "PROC twice flags 0 (INTEGER) -> INTEGER",
            "PROC v flags 4 () -> STRING",
        ]
    );
}

/// Only the main file's lines: twice's, debug.h's, are none of debug.c's.
#[test]
fn c_lines_are_the_main_files() {
    let lines: Vec<u16> = object().iter().filter(|one| one.r#type == omf::LINNUM).flat_map(|one| omf::lines(one).1).map(|(line, _)| line).collect();
    assert_eq!(lines, [13, 15, 16, 17]);
}

/// HIR's codec writes a member's bit keys only for a bit field, so a program
/// without one encodes as before, and a bit field's round-trips.
#[test]
fn bit_field_members_round_trip_through_the_codec_and_others_are_unchanged() {
    let hir = |name: &str| {
        let text = std::fs::read_to_string(Path::new(env!("LLRM_ROOT")).join(format!("tests/fixtures/c/{name}.cgs"))).expect("reads");
        super::translate::program(&super::hir::unit(&super::stream::parse(&text)).unwrap(), name, llrm_target::Target::calling(&llrm_x86_m16::M16).named("cdecl16").unwrap()).unwrap()
    };
    let plain = llrm_core::hir::codec::encode(&hir("debug"), None).unwrap();
    assert!(!plain.contains("bit_start") && plain.contains("\"members\""), "premise: debug members, none a bit field");
    let fields = hir("debugbf");
    let encoded = llrm_core::hir::codec::encode(&fields, None).unwrap();
    assert!(encoded.contains("\"bit_start\""), "premise: a bit field member");
    assert_eq!(llrm_core::hir::codec::decode(&encoded).unwrap(), fields);
}

/// A bit field read as its whole base type: `DBBitField`'s first bit and
/// width were dropped, and a debugger showed `b` as the int at offset 0.
/// Each is now QuickC's bitfield record of its width, sign and first bit.
#[test]
fn c_bit_fields_read_with_their_width_and_first_bit() {
    let shape = cvinfo::parse(&object_of("debugbf")).shape();
    assert!(
        shape.contains(&"DATA gf: TYPE flags {a +0 BITFIELD 3 UNSIGNED @0, b +0 BITFIELD 5 SIGNED @3, c +1 BITFIELD 9 UNSIGNED @0}".to_owned()),
        "{shape:#?}"
    );
}

/// A member with a bit field's first bit but no width is no bit field and
/// no plain member: the verifier refuses it.
#[test]
fn a_debug_member_with_half_a_bit_field_is_refused() {
    let text = std::fs::read_to_string(Path::new(env!("LLRM_ROOT")).join("tests/fixtures/c/debugbf.cgs")).expect("reads");
    let mut program = super::translate::program(&super::hir::unit(&super::stream::parse(&text)).unwrap(), "debugbf", llrm_target::Target::calling(&llrm_x86_m16::M16).named("cdecl16").unwrap()).unwrap();
    assert!(llrm_core::hir::verify::verify(&program).is_ok(), "premise: valid as raised");
    let member = program.modules[0].debug.as_mut().unwrap().types.iter_mut().flat_map(|one| &mut one.members).find(|one| one.bit_width.is_some()).expect("premise: a bit field");
    member.bit_width = None;
    let why = llrm_core::hir::verify::verify(&program).unwrap_err();
    assert!(why.0.contains("start or width alone"), "{why:?}");
}
