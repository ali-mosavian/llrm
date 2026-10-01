//! Optimizer regressions that need QB source to reach their shape.

/// deedlines failed "value#5479 is read but never defined": counting `i` itself
/// to zero rebased `i + 512` onto a new held offset but left it out of `uses`,
/// so dead deleted its definition.
#[test]
fn test_a_constant_offset_rebased_onto_the_counter_stays_defined() {
    use std::path::Path;

    use crate::{compile as qb_compile, driver as qb_driver};
    use llrm_core::model::passes::O2;

    let directory = tempfile::TempDir::new().unwrap();
    let basic = directory.path().join("MOD.BAS");
    let lines = [
        "DIM SHARED m%(-168 TO 168)",
        "FOR i% = -168 TO 168",
        "m%(i%) = ((i% + 512) MOD 256) \\ 2",
        "NEXT i%",
    ];
    std::fs::write(&basic, format!("{}\r\n", lines.join("\r\n"))).unwrap();
    let program =
        qb_driver::parsed(&basic, &qb_driver::Frontend::new("qb45", "qb45"), None).unwrap();
    qb_compile::object_bytes(&program, Path::new("MOD.BAS"), None, &O2()).unwrap();
}

#[test]
fn test_intervals_are_not_built_per_block_per_loop() {
    // sunk_stores built an interval map for every block, per loop: ten
    // loops here. Python's per-op copies took deedlines past 7 GB.
    use std::path::Path;

    use llrm_core::optimize::loopmotion::{MAPPED, SEEN};
    use crate::{compile as qb_compile, driver as qb_driver};
    use llrm_core::model::passes::O2;

    let directory = tempfile::TempDir::new().unwrap();
    let basic = directory.path().join("LOOPS.BAS");
    let mut lines = vec!["DEFINT A-Z".to_owned()];
    for k in 0..10 {
        lines.extend([format!("x{k} = {k}"), format!("FOR i = 1 TO 10: s{k} = s{k} + i: NEXT")]);
    }
    lines.push(format!("PRINT {}", (0..10).map(|k| format!("s{k} + x{k}")).collect::<Vec<_>>().join(" + ")));
    std::fs::write(&basic, format!("{}\r\n", lines.join("\r\n"))).unwrap();
    let program =
        qb_driver::parsed(&basic, &qb_driver::Frontend::new("qb45", "qb45"), None).unwrap();
    MAPPED.with(|mapped| mapped.set(0));
    SEEN.with(|seen| seen.set(0));
    qb_compile::object_bytes(&program, Path::new("LOOPS.BAS"), None, &O2()).unwrap();
    let (mapped, seen) = (MAPPED.with(std::cell::Cell::get), SEEN.with(std::cell::Cell::get));
    assert!(seen > 0 && mapped <= seen, "{mapped} maps for {seen} blocks");
}

mod decided_tests {
    use std::collections::BTreeSet;

    use crate::{compile as qb_compile, driver as qb_driver};
    use llrm_core::model::passes::O2;

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
        let program =
            qb_driver::parsed(&basic, &qb_driver::Frontend::new("qb45", "qb45"), None)
                .unwrap();
        let lowered = llrm_core::hir::lower::lower(&program).unwrap();
        let (function, body) = program.modules[0]
            .functions
            .iter()
            .zip(&lowered)
            .find(|(function, _)| function.name.ends_with('T'))
            .expect("SUB t");
        let body = qb_compile::optimized(&program, function, body, &O2()).unwrap().body;
        let defined: BTreeSet<_> = body
            .blocks
            .iter()
            .flat_map(|block| block.phis.iter().map(|phi| phi.result).chain(block.ops.iter().flat_map(|op| op.defines.clone())))
            .collect();
        let read: BTreeSet<_> = body
            .blocks
            .iter()
            .flat_map(|block| {
                block.phis.iter().flat_map(|phi| phi.incoming.values().copied()).chain(block.ops.iter().flat_map(|op| op.uses.clone()))
            })
            .collect();
        assert_eq!(read.difference(&defined).collect::<Vec<_>>(), Vec::<&llrm_core::model::mir::Value>::new());
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
    let lines = ["DECLARE SUB s (a() AS INTEGER, x AS LONG, BYVAL y AS INTEGER)", "SUB s (a() AS INTEGER, x AS LONG, BYVAL y AS INTEGER)", "x = a(y)", "END SUB"];
    std::fs::write(&basic, format!("{}\r\n", lines.join("\r\n"))).unwrap();
    let program = qb_driver::parsed(&basic, &qb_driver::Frontend::new("qb45", "qb45"), None).unwrap();
    let emitted = llrm_core::hir::mir::emit(&program);
    let text = llrm_mir::print::module(&emitted[0].module);
    let define = text.lines().find(|line| line.starts_with("define") && line.contains("@S(")).expect("SUB s");
    assert!(define.contains("ptr dereferenceable(18) %0, ptr dereferenceable(4) %1, i16 %2"), "{define}");
}

/// The MIR of `lines` compiled as a program, whole.
fn mir_of(lines: &[&str]) -> String {
    use crate::driver as qb_driver;

    let directory = tempfile::TempDir::new().unwrap();
    let basic = directory.path().join("POLL.BAS");
    std::fs::write(&basic, format!("{}\r\n", lines.join("\r\n"))).unwrap();
    let program = qb_driver::parsed(&basic, &qb_driver::Frontend::new("qb45", "qb45"), None).unwrap();
    llrm_mir::print::module(&llrm_core::hir::mir::emit(&program)[0].module)
}

/// What a loop waiting on another agent reads, it reads each trip. The
/// default route hoisted the load and the loop spun on a register for ever.
#[test]
fn test_a_volatile_variable_a_loop_polls_is_read_each_trip() {
    let byref = mir_of(&["DECLARE SUB WaitFor (x AS INTEGER)", "DIM k AS INTEGER", "WaitFor k", "SUB WaitFor (x AS VOLATILE INTEGER)", "DO WHILE x = 0", "LOOP", "END SUB"]);
    let shared = mir_of(&["DIM SHARED k AS VOLATILE INTEGER", "SUB WaitShared", "DO WHILE k = 0", "LOOP", "END SUB", "WaitShared"]);
    let local = mir_of(&["SUB WaitLocal", "DIM f AS VOLATILE INTEGER", "DO WHILE f = 0", "LOOP", "END SUB", "WaitLocal"]);
    for (name, text, routine) in [("BYREF", byref, "@WAITFOR("), ("SHARED", shared, "@WAITSHARED("), ("local", local, "@WAITLOCAL(")] {
        let at = text.find(routine).unwrap_or_else(|| panic!("{name}: no {routine}\n{text}"));
        assert!(text[at..].contains("load volatile i16"), "{name}\n{text}");
    }
}

/// Nothing is assumed volatile: a BYREF no one declared so is an ordinary
/// variable, and the same loop on it has an ordinary load.
#[test]
fn test_an_unannotated_byref_is_not_assumed_volatile() {
    let text = mir_of(&["DECLARE SUB Sum (x AS INTEGER)", "DIM k AS INTEGER", "Sum k", "SUB Sum (x AS INTEGER)", "DO WHILE x = 0", "LOOP", "END SUB"]);
    let at = text.find("@SUM(").expect("the SUB");
    assert!(!text[at..].contains("volatile"), "{text}");
}

/// A field of a BYREF record is the record's: ordered when it is declared
/// `VOLATILE`, ordinary when it is not. It was ordered always (qmove's
/// `vel.x`), which the rich route ignored and the legacy route did not.
#[test]
fn test_a_field_of_a_byref_record_follows_the_records_declaration() {
    let sub = |declared: &str| {
        mir_of(&["TYPE V", "x AS INTEGER", "END TYPE", "DECLARE SUB S (v AS V)", "DIM r AS V", "S r", &format!("SUB S (v {declared} V)"), "DO WHILE v.x = 0", "LOOP", "END SUB"])
    };
    let plain = sub("AS");
    let volatile = sub("AS VOLATILE");
    let body = |text: &str| text[text.find("@S(").expect("the SUB")..].to_owned();
    assert!(!body(&plain).contains("volatile"), "{plain}");
    assert!(body(&volatile).contains("load volatile i16"), "{volatile}");
}
