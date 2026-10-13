//! DEF SEG where one address space has no segments.

use qbfront::semantic::{compile_with_warnings, Options};
use qbfront::{parse, Dialect};

fn compiled(
    source: &str,
    options: &Options,
) -> Result<Vec<String>, String> {
    let module = parse(source, Dialect::QuickBasic45).expect("parses");
    compile_with_warnings(&module, "t", Dialect::QuickBasic45, "llrm", options)
        .map(|(_, warnings)| warnings)
        .map_err(|e| format!("{e:?}"))
}

fn flat() -> Options {
    Options { near_bytes: 4, far_bytes: 4, ..Options::default() }
}

fn real() -> Options {
    Options { near_bytes: 2, far_bytes: 4, ..Options::default() }
}

/// qbdemo's fracline did `DEF SEG = VARSEG(array(i))` and POKEd the interrupt
/// table at -m32: it died with no message, and a DEF SEG = &HB800 program wrote
/// the first kilobytes of memory instead of the screen. Every DEF SEG says it
/// is ignored, at the line.
#[test]
fn every_def_seg_warns_where_there_is_one_address_space_and_real_mode_does_not() {
    let source = "DIM a(100) AS INTEGER\nDEF SEG = VARSEG(a(0))\nDEF SEG = &HB800\nDEF SEG\nPOKE 5, 1\n";
    let found = compiled(source, &flat()).expect("compiles");
    assert_eq!(found.len(), 3, "{found:?}");
    for (at, line) in found.iter().zip([2, 3, 4]) {
        assert!(at.starts_with(&format!("line {line}: warning: DEF SEG is ignored at one address space")), "{at}");
        assert!(at.contains("vram& = &HB8000") && at.contains("docs/frontends/qb/memory-model.md"), "{at}");
    }
    assert!(compiled(source, &real()).expect("compiles").is_empty());
}

/// The image's segment and offset are 16-bit words: a flat program names the
/// address, so BLOAD with none is refused.
#[test]
fn bload_without_an_address_is_refused_where_there_is_one_address_space() {
    assert!(compiled("BLOAD \"A.BIN\"\n", &real()).is_ok());
    let error = compiled("BLOAD \"A.BIN\"\n", &flat()).expect_err("refused");
    assert!(error.contains("BLOAD without an offset"), "{error}");
    assert!(compiled("BLOAD \"A.BIN\", 4096\n", &flat()).is_ok());
}
