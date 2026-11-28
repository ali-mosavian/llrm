//! `drop_known_loads` shortens one of two copies of a run (rc8 and rc9 are
//! known zero in the first, `lda rc8` goes), so the copies were no longer equal
//! and the outliner left both inline: object_extent_loop -Os, 205 B against 186
//! B before #10422 (one `jsr __outl_N` of the whole run at both places).

mod common;

use common::pipeline::datalayout;
use llrm_6502::driver::{Level, Request, assembly};

/// `(long)(dot() * 16)` twice: each is the store of the double's halves into
/// the soft-float argument block, then `__muldf3` and `__fixdfsi`.
#[test]
fn a_run_the_known_loads_shorten_in_one_copy_is_still_one_routine() {
    let ir = format!(
        "target datalayout = \"{}\"\ndeclare double @dot()\ndefine i32 @main() {{\n\
         %a = call double @dot()\n  %m = fmul double %a, 1.600000e+01\n  %i = fptosi double %m to i32\n\
         %b = call double @dot()\n  %n = fmul double %b, 1.600000e+01\n  %j = fptosi double %n to i32\n\
         %s = add i32 %i, %j\n  ret i32 %s\n}}\n",
        datalayout()
    );
    let text = assembly(&ir, &Request::new(Level::Os)).unwrap_or_else(|e| panic!("{e}"));
    // the stores of the second half of the argument block are in the routine,
    // not in either caller
    let stores = text.matches("  ldy #5\n  sta (rc18),y\n").count();
    assert_eq!(stores, 1, "the run is inline at {stores} places:\n{text}");
}
