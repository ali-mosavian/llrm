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
/// the first kilobytes of memory instead of the screen; far3.bas (a string
/// flipped in case through DEF SEG = VARSEG(a) and SADD) printed it unchanged
/// with n = 0. A DEF SEG with a segment is refused, at the line; a bare DEF SEG
/// stays accepted, and says it is ignored.
#[test]
fn a_def_seg_with_a_segment_is_refused_where_there_is_one_address_space_and_real_mode_accepts_it() {
    for source in ["DIM a(100) AS INTEGER\nDEF SEG = VARSEG(a(0))\n", "DEF SEG = &HB800\nPOKE 5, 1\n"] {
        let error = compiled(source, &flat()).expect_err("refused");
        assert!(error.contains("DEF SEG with a non-default segment is not supported at one address space"), "{error}");
        assert!(compiled(source, &real()).expect("compiles").is_empty());
    }
    let found = compiled("DEF SEG\nPOKE 5, 1\n", &flat()).expect("compiles");
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].starts_with("line 1: warning: DEF SEG is ignored at one address space"), "{found:?}");
    assert!(compiled("DEF SEG\nPOKE 5, 1\n", &real()).expect("compiles").is_empty());
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
