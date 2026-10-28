//! Optimizer regressions that need QB source to reach their shape.

/// deedlines failed "value#5479 is read but never defined": counting `i` itself
/// to zero rebased `i + 512` onto a new held offset but left it out of `uses`,
/// so dead deleted its definition.
#[test]
fn test_a_constant_offset_rebased_onto_the_counter_stays_defined() {
    use std::path::Path;

    use crate::{compile as qb_compile, driver as qb_driver};

    let directory = tempfile::TempDir::new().unwrap();
    let basic = directory.path().join("MOD.BAS");
    let lines = ["DIM SHARED m%(-168 TO 168)", "FOR i% = -168 TO 168", "m%(i%) = ((i% + 512) MOD 256) \\ 2", "NEXT i%"];
    std::fs::write(&basic, format!("{}\r\n", lines.join("\r\n"))).unwrap();
    let program = qb_driver::parsed(&basic, &qb_driver::Frontend::new("qb45", "qb45"), None).unwrap();
    qb_compile::object_bytes(
        &program,
        Path::new("MOD.BAS"),
        None,
        &llrm_driver::m16_options_old(llrm_x86_m16::machine::BASIC.clone()),
    )
    .unwrap();
}

/// A string comparison in a program with ON ERROR was refused, "a call that
/// may raise an error where ON ERROR is on, which is no invoke to the pad":
/// B$SCMP had no row, so it was taken to raise. Any `IF a$ < b$` in such a
/// program failed to compile.
#[test]
fn test_a_string_comparison_compiles_where_on_error_is_on() {
    use std::path::Path;

    use crate::{compile as qb_compile, driver as qb_driver};

    let directory = tempfile::TempDir::new().unwrap();
    let basic = directory.path().join("CMP.BAS");
    let lines = [
        "DEFINT A-Z",
        "DIM a AS STRING, b AS STRING",
        "a = \"x\": b = \"y\"",
        "ON ERROR GOTO oops",
        "IF a < b THEN k = 1",
        "END",
        "oops:",
        "RESUME NEXT",
    ];
    std::fs::write(&basic, format!("{}\r\n", lines.join("\r\n"))).unwrap();
    for runtime in ["qb45", "llrm"] {
        let program = qb_driver::parsed(&basic, &qb_driver::Frontend::new("qb45", runtime), None).unwrap();
        qb_compile::object_bytes(
            &program,
            Path::new("CMP.BAS"),
            None,
            &llrm_driver::m16_options_old(llrm_x86_m16::machine::BASIC.clone()),
        )
        .unwrap_or_else(|error| panic!("{runtime}: {error}"));
    }
}

mod decided_tests {

    use crate::driver as qb_driver;

    /// deedlines hung in SPHEREMAPLASMA: deciding a constant zero-trip guard
    /// dropped every op sharing the guard's source address, which included the
    /// countdown's seeds, so the fade loop counted from an unwritten slot.
    #[test]
    fn test_deciding_a_guard_keeps_the_work_sharing_its_address() {
        let directory = tempfile::TempDir::new().unwrap();
        let basic = directory.path().join("FADE.BAS");
        let lines = [
            "DECLARE SUB t ()",
            "'$DYNAMIC",
            "DIM SHARED r%(0 TO 255), g%(0 TO 255), B%(0 TO 255)",
            "'$STATIC",
            "t",
            "SUB t",
            "s% = 0: t% = 26",
            "s1% = 0: t1% = 26",
            "fps% = 0",
            "DO",
            "fps% = fps% + 1",
            "IF fps% = 128 THEN fadeout% = 1",
            "IF s% < t% THEN s% = s% + 1 ELSE GOTO endfadepal",
            "OUT &H3C8, 0",
            "FOR n% = 0 TO 255 STEP 10",
            "OUT &H3C9, 63 + s% * ((r%(n%) - 63) / t%)",
            "OUT &H3C9, 63 + s% * ((g%(n%) - 63) / t%)",
            "OUT &H3C9, 63 + s% * ((B%(n%) - 63) / t%)",
            "NEXT n%",
            "endfadepal:",
            "IF fadeout% = 0 THEN GOTO nofadeout",
            "IF s1% < t1% THEN s1% = s1% + 1 ELSE GOTO gout",
            "OUT &H3C8, 0",
            "FOR n% = 0 TO 255 STEP 10",
            "OUT &H3C9, r%(n%) + s1% * ((0 - r%(n%)) / t1%)",
            "OUT &H3C9, g%(n%) + s1% * ((0 - g%(n%)) / t1%)",
            "OUT &H3C9, B%(n%) + s1% * ((0 - B%(n%)) / t1%)",
            "NEXT n%",
            "nofadeout:",
            "LOOP",
            "gout:",
            "END SUB",
        ];
        std::fs::write(&basic, format!("{}\r\n", lines.join("\r\n"))).unwrap();
        let program = qb_driver::parsed(&basic, &qb_driver::Frontend::new("qb45", "qb45"), None).unwrap();
        // The pipeline verifies its result: every value read is defined.
        let optimized = crate::test_hir::optimized_mir(&program);
        assert!(optimized.contains("define cc 1000 void @T("), "{optimized}");
    }
}

/// A reference parameter names a whole object: an array's descriptor, of
/// rank one at least, or a variable of its type. Unpromised, sum_three
/// reloaded its descriptors every trip, as a load may fault.
#[test]
fn test_a_reference_parameter_is_dereferenceable() {
    use crate::driver as qb_driver;

    let directory = tempfile::TempDir::new().unwrap();
    let basic = directory.path().join("REF.BAS");
    let lines = [
        "DECLARE SUB s (a() AS INTEGER, x AS LONG, BYVAL y AS INTEGER)",
        "SUB s (a() AS INTEGER, x AS LONG, BYVAL y AS INTEGER)",
        "x = a(y)",
        "END SUB",
    ];
    std::fs::write(&basic, format!("{}\r\n", lines.join("\r\n"))).unwrap();
    let program = qb_driver::parsed(&basic, &qb_driver::Frontend::new("vbdos", "vbdos"), None).unwrap();
    let emitted = llrm_codegen::hir::mir::emit(&program, &llrm_x86_m16::layout());
    let text = llrm_mir::print::module(&emitted[0].module);
    let define = text.lines().find(|line| line.starts_with("define") && line.contains("@S(")).expect("SUB s");
    assert!(define.contains("ptr dereferenceable(18) %0, ptr dereferenceable(4) %1, i16 %2"), "{define}");
}

/// B$SCMP's result is the machine flags, read by comparisons alone: combine
/// turned `sext (icmp slt v, 0)` into `ashr v, 15`, and every string
/// comparison of the BC runtime failed to compile at -O1 ("flags read other
/// than by a comparison").
#[test]
fn a_string_comparison_stays_a_comparison_of_the_flags_when_optimized() {
    use crate::driver as qb_driver;

    let directory = tempfile::TempDir::new().unwrap();
    let basic = directory.path().join("SCMP.BAS");
    std::fs::write(&basic, "PRINT \"\" < \"a\"\r\n").unwrap();
    let program = qb_driver::parsed(&basic, &qb_driver::Frontend::new("qb45", "qb45"), None).unwrap();
    let optimized = crate::test_hir::optimized_mir(&program);
    assert!(
        optimized.contains("@llrm.qb.B$SCMP(ptr, ptr) addrspace(1) threeway nocallback nounwind flagsresult"),
        "{optimized}"
    );
    assert!(optimized.contains("icmp slt") && !optimized.contains("ashr"), "{optimized}");
}

/// The MIR of `lines` compiled as a program, whole.
fn mir_of(lines: &[&str]) -> String {
    use crate::driver as qb_driver;

    let directory = tempfile::TempDir::new().unwrap();
    let basic = directory.path().join("POLL.BAS");
    std::fs::write(&basic, format!("{}\r\n", lines.join("\r\n"))).unwrap();
    let program = qb_driver::parsed(&basic, &qb_driver::Frontend::new("qb45", "qb45"), None).unwrap();
    llrm_mir::print::module(&llrm_codegen::hir::mir::emit(&program, &llrm_x86_m16::layout())[0].module)
}

/// What a loop waiting on another agent reads, it reads each trip. The
/// default route hoisted the load and the loop spun on a register for ever.
#[test]
fn test_a_volatile_variable_a_loop_polls_is_read_each_trip() {
    let byref = mir_of(&[
        "DECLARE SUB WaitFor (x AS INTEGER)",
        "DIM k AS INTEGER",
        "WaitFor k",
        "SUB WaitFor (x AS VOLATILE INTEGER)",
        "DO WHILE x = 0",
        "LOOP",
        "END SUB",
    ]);
    let shared = mir_of(&[
        "DIM SHARED k AS VOLATILE INTEGER",
        "SUB WaitShared",
        "DO WHILE k = 0",
        "LOOP",
        "END SUB",
        "WaitShared",
    ]);
    let local =
        mir_of(&["SUB WaitLocal", "DIM f AS VOLATILE INTEGER", "DO WHILE f = 0", "LOOP", "END SUB", "WaitLocal"]);
    for (name, text, routine) in
        [("BYREF", byref, "@WAITFOR("), ("SHARED", shared, "@WAITSHARED("), ("local", local, "@WAITLOCAL(")]
    {
        let at = text.find(routine).unwrap_or_else(|| panic!("{name}: no {routine}\n{text}"));
        assert!(text[at..].contains("load volatile i16"), "{name}\n{text}");
    }
}

/// Nothing is assumed volatile: a BYREF no one declared so is an ordinary
/// variable, and the same loop on it has an ordinary load.
#[test]
fn test_an_unannotated_byref_is_not_assumed_volatile() {
    let text = mir_of(&[
        "DECLARE SUB Sum (x AS INTEGER)",
        "DIM k AS INTEGER",
        "Sum k",
        "SUB Sum (x AS INTEGER)",
        "DO WHILE x = 0",
        "LOOP",
        "END SUB",
    ]);
    let at = text.find("@SUM(").expect("the SUB");
    assert!(!text[at..].contains("volatile"), "{text}");
}

/// A field of a BYREF record is the record's: ordered when it is declared
/// `VOLATILE`, ordinary when it is not. It was ordered always (qmove's
/// `vel.x`), which the rich route ignored and the legacy route did not.
#[test]
fn test_a_field_of_a_byref_record_follows_the_records_declaration() {
    let sub = |declared: &str| {
        mir_of(&[
            "TYPE V",
            "x AS INTEGER",
            "END TYPE",
            "DECLARE SUB S (v AS V)",
            "DIM r AS V",
            "S r",
            &format!("SUB S (v {declared} V)"),
            "DO WHILE v.x = 0",
            "LOOP",
            "END SUB",
        ])
    };
    let plain = sub("AS");
    let volatile = sub("AS VOLATILE");
    let body = |text: &str| text[text.find("@S(").expect("the SUB")..].to_owned();
    assert!(!body(&plain).contains("volatile"), "{plain}");
    assert!(body(&volatile).contains("load volatile i16"), "{volatile}");
}

/// An array of words sits on a word: the alignment the frontend states of
/// its data object reaches the global, as the field it replaced did.
#[test]
fn test_an_array_of_words_is_aligned_to_a_word() {
    let text = mir_of(&["DIM SHARED a(1 TO 4) AS INTEGER", "a(1) = 1"]);
    assert!(text.lines().any(|line| line.starts_with("@\"A%\"") && line.contains(", align 2")), "{text}");
}
