//! DEF SEG = VARSEG(array(i)) where one address space has no segments.

use qbfront::semantic::{compile_with_warnings, Options};
use qbfront::{parse, Dialect};

const SOURCE: &str = "DIM a(100) AS INTEGER\nDEF SEG = VARSEG(a(0))\nPOKE 5, 1\nDEF SEG = VARSEG(x%)\n";

fn warnings(options: &Options) -> Vec<String> {
    let module = parse(SOURCE, Dialect::QuickBasic45).expect("parses");
    compile_with_warnings(&module, "t", Dialect::QuickBasic45, "llrm", options).expect("compiles").1
}

/// qbdemo's fracline does this and POKEd the interrupt table at -m32: it died
/// with no message. The warning names the line and the limit; real mode (a far
/// pointer is not a near one) and a plain variable's VARSEG say nothing.
#[test]
fn the_segment_of_an_array_element_warns_where_there_is_one_address_space() {
    let flat = Options { near_bytes: 4, far_bytes: 4, ..Options::default() };
    let found = warnings(&flat);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].starts_with("line 2: warning: DEF SEG = VARSEG(array element) is not supported"), "{found:?}");
    assert!(found[0].contains("docs/frontends/qb/memory-model.md"), "{found:?}");
    let real = Options { near_bytes: 2, far_bytes: 4, ..Options::default() };
    assert!(warnings(&real).is_empty());
}
