//! A procedure frames itself, not through B$ENRA/B$EXSA, where the runtime
//! needs no frame of its own; `--runtime-frames` gives every one the runtime's.

use llrm_core::hir::execute;
use llrm_core::hir::model::Program;

use super::driver as qb_driver;
use super::test_hir::{between, listing, written};

/// Each dialect, on its runtime.
const DIALECTS: [(&str, &str); 4] = [("qbasic11", "qb45"), ("qb45", "qb45"), ("pds71", "pds71"), ("vbdos", "vbdos")];

fn program(
    source: &str,
    dialect: &str,
    runtime: &str,
    own_frames: bool,
) -> Program {
    let directory = tempfile::tempdir().expect("creates a directory");
    let path = written(&directory, "frames.bas", source.as_bytes());
    let frontend = qb_driver::Frontend { runtime_frames: !own_frames, ..qb_driver::Frontend::new(dialect, runtime) };
    qb_driver::parsed(&path, &frontend, None).unwrap_or_else(|error| panic!("{dialect}: {error}"))
}

/// Procedure `name`'s listing on the rich route.
fn procedure(
    source: &str,
    dialect: &str,
    runtime: &str,
    own_frames: bool,
    name: &str,
) -> String {
    let listing = listing(&program(source, dialect, runtime, own_frames));
    between(&listing, &format!("{name} proc"), &format!("{name} endp")).to_owned()
}

fn runtime_framed(listing: &str) -> bool {
    listing.contains("B$ENRA") || listing.contains("B$EXSA")
}

#[test]
/// QB 4.5, PDS 7.1 and VBDOS called B$ENRA in every procedure by default: fib
/// took 14.9 ms against 5.0 and each call 95 instructions more.
fn own_frames_are_the_default_in_every_dialect() {
    let source = "SUB s (x AS INTEGER)\nDIM a AS LONG, b AS DOUBLE\na = x\nb = a\nx = b\nEND SUB\n";
    for (dialect, runtime) in DIALECTS {
        assert!(runtime_framed(&procedure(source, dialect, runtime, false, "S")), "{dialect} with --runtime-frames");
        let own = procedure(source, dialect, runtime, true, "S");
        assert!(!runtime_framed(&own), "{dialect}: {own}");
    }
}

#[test]
fn own_frames_keep_the_runtime_frame_where_the_runtime_needs_it() {
    // A local STRING's handle, and a module handler the runtime reaches
    // through the procedure's frame.
    let string = "SUB s\nDIM t AS STRING\nt = \"x\"\nPRINT t\nEND SUB\n";
    let module_handler = "ON ERROR GOTO h\ns\nEND\nh:\nRESUME NEXT\nSUB s\nDIM i AS INTEGER\ni = 1\nPRINT i\nEND SUB\n";
    for (dialect, runtime) in DIALECTS {
        for source in [string, module_handler] {
            assert!(runtime_framed(&procedure(source, dialect, runtime, true, "S")), "{dialect}: {source}");
        }
    }
    let local_handler = "SUB s\nDIM i AS INTEGER\nON LOCAL ERROR GOTO h\ni = 1\nEXIT SUB\nh:\nRESUME NEXT\nEND SUB\n";
    for (dialect, runtime) in [("pds71", "pds71"), ("vbdos", "vbdos")] {
        assert!(runtime_framed(&procedure(local_handler, dialect, runtime, true, "S")), "{dialect}");
    }
}

#[test]
fn own_frames_locals_still_start_at_zero() {
    // A frame the procedure makes itself holds garbage in the interpreter:
    // only the HIR's own stores zero these.
    let source = "TYPE P\nx AS INTEGER\ny AS DOUBLE\nEND TYPE\n\
        s\ns\nEND\n\
        SUB s\nDIM a AS LONG, d AS DOUBLE, f AS SINGLE, p AS P, e(2) AS INTEGER, i AS INTEGER\n\
        FOR i = 1 TO 2\na = a + i\nNEXT\n\
        PRINT a; d; f; p.x; p.y; e(1)\nd = 5: f = 6: p.x = 7: e(1) = 8\nEND SUB\n";
    let expected = " 3  0  0  0  0  0 \n 3  0  0  0  0  0 \n";
    for (dialect, runtime) in DIALECTS {
        for own_frames in [false, true] {
            let executed = execute::run(&program(source, dialect, runtime, own_frames), "__main", &[]).expect("runs");
            assert_eq!(executed.panic, None, "{}", executed.output);
            assert_eq!(executed.output, expected, "{dialect}, own frames {own_frames}");
        }
    }
}

/// Zeroed a word store at a time, qbdemo's PLASMA grew 38 stores for four
/// local array descriptors, and the demo 10% more code, where B$ENRA had
/// cleared the frame in one call.
#[test]
fn own_frames_clear_zeroed_aggregates_in_one_fill() {
    let source = "SUB s (n AS INTEGER)\nDIM a(n) AS INTEGER, b(n) AS LONG, c(n) AS SINGLE, d(n, n) AS INTEGER\n\
        a(0) = 1: b(0) = 2: c(0) = 3: d(0, 0) = 4\nPRINT a(0); b(0); c(0); d(0, 0)\nEND SUB\n";
    for (dialect, runtime) in DIALECTS {
        let own = procedure(source, dialect, runtime, true, "S");
        let entry = own.split("call far ptr").next().expect("a call");
        assert_eq!(entry.matches("rep stosd").count(), 1, "{dialect}: {own}");
        assert!(!entry.contains("mov word ptr [bp") && !entry.contains("mov dword ptr [bp"), "{dialect}: {own}");
    }
}

/// Each indexed frame cell's displacement, `[bp+si-30]`, from the listing's
/// first frame address: the REDIM array's descriptor.
fn indexed_from_descriptor(listing: &str) -> Vec<i64> {
    let number = |text: &str| text[..text.find(']').unwrap()].parse::<i64>().unwrap();
    let descriptor = listing
        .lines()
        .filter(|line| line.trim().starts_with("lea "))
        .find_map(|line| line.split("[bp").nth(1))
        .map(number)
        .expect("the descriptor's address");
    let indexed: Vec<i64> = listing
        .lines()
        .filter_map(|line| line.split("[bp+").nth(1))
        .filter(|rest| rest.starts_with(|first: char| first.is_ascii_alphabetic()))
        .map(|rest| number(&rest[2..]) - descriptor)
        .collect();
    assert!(!indexed.is_empty(), "no indexed frame cell: {listing}");
    indexed
}

/// UBOUND of a local REDIM array printed -1 on the runtime's frame: its
/// bounds, read through `[bp+si-30]`, stayed where they were while B$ENRA's
/// header moved the rest of the descriptor down (#80).
#[test]
fn indexed_frame_cells_move_with_the_runtime_frame() {
    // A dimension known only at run time keeps the bound's read indexed.
    let source = "SUB arrs (n AS INTEGER, d AS INTEGER)\nREDIM v(n) AS LONG\nv(n) = 7\nPRINT UBOUND(v, d); v(n)\nEND SUB\narrs 3, 1\n";
    for (dialect, runtime) in DIALECTS {
        let framed = procedure(source, dialect, runtime, false, "ARRS");
        assert!(runtime_framed(&framed), "{dialect}");
        let own = procedure(source, dialect, runtime, true, "ARRS");
        assert_eq!(indexed_from_descriptor(&framed), indexed_from_descriptor(&own), "{dialect}:\n{framed}\n{own}");
    }
}

/// Scripts passed `--own-frames`, which became the default: it must still parse
/// and change nothing, and `--runtime-frames` must reach the frontend.
#[test]
fn the_frame_switches_parse() {
    let parse = |flag: &str| {
        super::cli::parse_args(&["a.bas".to_owned(), flag.to_owned()]).expect("parses").frontend.runtime_frames
    };
    assert!(!parse("--own-frames"));
    assert!(parse("--runtime-frames"));
}

/// Under own frames, a procedure that keeps B$ENRA (here a local STRING) had
/// its frame emitted as holding garbage, so a local read before it was written
/// was no longer zero to the optimizer: deedlines' INITCROSFADEPICS went from
/// 263 071 to 4.5 million estimated instructions. Its frame is zeroed; a
/// self-framed one is not.
#[test]
fn a_runtime_framed_procedure_still_zeroes_its_frame_in_mir() {
    let source = "SUB kept (n AS INTEGER)\nDIM k AS INTEGER, t AS STRING\nt = \"x\"\nPRINT k + n; t\nEND SUB\n\
        SUB own (n AS INTEGER)\nDIM k AS INTEGER\nPRINT k + n\nEND SUB\n";
    for (dialect, runtime) in DIALECTS {
        let text = llrm_mir::print::module(
            &llrm_core::hir::mir::emit(&program(source, dialect, runtime, true), &llrm_x86_m16::layout())
                .swap_remove(0)
                .module,
        );
        let entry = |name: &str| {
            text.split(&format!("@{name}("))
                .nth(1)
                .and_then(|rest| rest.split("\n}").next())
                .unwrap_or_else(|| panic!("{name} in {text}"))
                .to_owned()
        };
        // The emitter's own zeroing carries no metadata; the frontend's stores
        // do.
        let zeroes = |name: &str| {
            entry(name).lines().any(|line| line.trim().starts_with("store i16 0, ptr %") && !line.contains('!'))
        };
        assert!(zeroes("KEPT"), "{dialect}: {}", entry("KEPT"));
        assert!(!zeroes("OWN"), "{dialect}: {}", entry("OWN"));
    }
}
