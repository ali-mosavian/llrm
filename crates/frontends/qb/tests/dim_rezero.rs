//! A scalar's DIM that is not the name's first sight zeroes it (BC), and the
//! first sight is of this procedure.

use llrm_qb::semantic::{Options, compile_with_options};
use llrm_qb::{Dialect, parse};

fn compiled(source: &str) -> Result<String, String> {
    let module = parse(source, Dialect::QuickBasic45).expect("parses");
    compile_with_options(&module, "t", Dialect::QuickBasic45, "llrm", &Options::default())
        .map_err(|error| format!("{error:?}"))
}

/// #1637 panicked ("index out of bounds: the len is 3 but the index is 3") on
/// bench/parity/qmove and qbsp: `vel` was seen in one procedure, so the DIM of
/// another procedure's local record of the same name looked up a place by its
/// id as an index.
#[test]
fn a_local_record_dimmed_in_two_procedures_compiles() {
    let source = "TYPE V\n  x AS INTEGER\n  y AS LONG\nEND TYPE\nDECLARE SUB A ()\nDECLARE SUB B ()\nCALL A\nCALL B\n\
                  SUB A\n  DIM v AS V\n  v.x = 5\n  PRINT v.x\nEND SUB\nSUB B\n  DIM v AS V\n  v.x = 6\n  PRINT v.x\nEND SUB\n";
    assert!(compiled(source).is_ok(), "{:?}", compiled(source).err());
}
