//! `-fsanitize=stack`: the limit and the handler are what the runtime's description says.

use llrm_core::hir::model::{Program, StackCheck};

use super::driver as qb_driver;
use super::test_hir::{between, listing, written};

const RUNTIMES: [&str; 3] = ["qb45", "pds71", "vbdos"];
const SOURCE: &str = "SUB s (x AS INTEGER)\nx = x + 1\nEND SUB\n";
/// A local STRING needs the runtime's frame.
const STRING: &str = "SUB s\nDIM t AS STRING\nt = \"x\"\nPRINT t\nEND SUB\n";

fn program(runtime: &str, checked: bool, source: &str) -> Program {
    let directory = tempfile::tempdir().expect("creates a directory");
    let path = written(&directory, "stack.bas", source.as_bytes());
    let frontend = qb_driver::Frontend { checked_stack: checked, ..qb_driver::Frontend::new(runtime, runtime) };
    qb_driver::parsed(&path, &frontend, None).unwrap_or_else(|error| panic!("{runtime}: {error}"))
}

fn procedure(program: &Program) -> String {
    between(&listing(program), "S proc", "S endp").to_owned()
}

/// A pass that named `b$pendchk` itself would not follow a runtime that keeps its limit elsewhere:
/// each runtime's row is what the program carries, and whatever names it states reach the listing.
#[test]
fn the_limit_and_handler_come_from_the_runtime_description() {
    for runtime in RUNTIMES {
        let mut checked = program(runtime, true, SOURCE);
        assert_eq!(checked.stack_check, llrm_core::abi::runtime::semantics::stack(runtime), "{runtime}");
        checked.stack_check = Some(StackCheck { limit: "FOO".into(), handler: "BAR".into(), far: true, red_zone: 0, entry: Some("BAZ".into()) });
        let own = procedure(&checked);
        assert!(own.contains("cmp sp, word ptr FOO") && own.contains("call far ptr BAR"), "{runtime}: {own}");
        assert!(!own.contains("pendchk") && !own.contains("ERR_OSS"), "{runtime}: {own}");
        // Framed by the runtime, it enters through the entry the description names.
        let mut framed = program(runtime, true, STRING);
        framed.stack_check = checked.stack_check.clone();
        let entered = procedure(&framed);
        assert!(entered.contains("call far ptr BAZ") && !entered.contains("B$ENRA") && !entered.contains("cmp sp"), "{runtime}: {entered}");
        // Whichever entry it is, it builds the frame: a shell `push bp` before it made the first
        // argument read as garbage (29281 for 5) under the checking B$ENRD, and a run past it.
        assert!(!entered.contains("push bp") && !entered.contains("push si"), "{runtime}: {entered}");
    }
}

/// Never on by default: no check in the HIR program, in MIR or in the listing.
#[test]
fn the_default_build_checks_nothing() {
    for runtime in RUNTIMES {
        for source in [SOURCE, STRING] {
            let plain = program(runtime, false, source);
            assert_eq!(plain.stack_check, None);
            let text = listing(&plain);
            assert!(!text.contains("pendchk") && !text.contains("ERR_OSS") && !text.contains("B$ENRD"), "{runtime}: {text}");
            let mir = llrm_core::hir::mir::emit(&plain, &llrm_x86_code16::layout());
            assert!(mir.iter().all(|one| !format!("{:?}", one.module.globals).contains("stackcheck")), "{runtime}");
        }
        let marked = llrm_core::hir::mir::emit(&program(runtime, true, SOURCE), &llrm_x86_code16::layout());
        assert!(marked.iter().any(|one| format!("{:?}", one.module.globals).contains("stackcheck")), "{runtime}");
    }
}
