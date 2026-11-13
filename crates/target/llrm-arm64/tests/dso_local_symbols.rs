//! arm64 takes every symbol as `dso_local` (`module_tables` returns every one
//! local): the implicit-visibility rule of the other mode must not reach it.

use llrm_arm64::dag::function::builder_target;
use llrm_arm64::machine::Platform;

#[test]
fn arm64_keeps_every_symbol_dso_local() {
    for platform in [Platform::Linux, Platform::Darwin] {
        assert!(builder_target(platform).dag.dso_local_symbols);
    }
}
