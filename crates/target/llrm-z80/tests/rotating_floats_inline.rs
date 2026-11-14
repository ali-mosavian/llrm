//! A large callee with one call site is not copied into its caller at -O2/-O3
//! where that grows the code.

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

/// x87rotate -O2 inlined `rot` (ten float adds and multiplies per trip) into
/// `main` and kept the original: 3764 B against 2819 B at -O1 and 3500 B for
/// SDCC. `Options::speed_rate` priced the copy's bytes at 3 T a byte, which the
/// call overhead and the loop's clocks repaid. The z80 prices bytes first at
/// every level, so -O2 takes the pin's threshold alone, as -O1 does.
#[test]
fn a_large_callee_is_not_copied_into_its_caller_at_speed() {
    for level in [Level::O1, Level::O2, Level::O3] {
        let asm =
            text(include_str!("kernels/ir/rotating_floats.ll"), "main", 0, &Request::new(level)).expect("compiles");
        assert!(asm.contains("call _rot"), "{level:?}: rot was inlined into main\n{asm}");
    }
}
