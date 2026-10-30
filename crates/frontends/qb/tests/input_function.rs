//! INPUT$(n [, [#]channel]).

use llrm_qb::semantic::{Options, compile_with_options};
use llrm_qb::{Dialect, parse};

fn hir(source: &str) -> String {
    let module = parse(source, Dialect::QuickBasic45).expect("parses");
    compile_with_options(&module, "t", Dialect::QuickBasic45, "llrm", &Options::default()).expect("compiles")
}

/// INPUT$ was a keyword with no lowering ("INPUT$ is not supported"), and the
/// file number's # did not parse: qbdemo does not build.
#[test]
fn input_dollar_calls_the_runtime_with_a_count_and_a_channel_with_or_without_the_hash() {
    for source in ["a$ = INPUT$(3)\n", "a$ = INPUT$(3, 1)\n", "a$ = INPUT$(3, #1)\n"] {
        assert!(hir(source).contains("\"callee\":\"B$FINP\""), "{source}");
    }
}
