//! z80 addresses are absolute, every symbol `dso_local`: the
//! implicit-visibility rule of the other mode must not reach it.

use llrm_z80::dagisel::function::builder_target;

#[test]
fn z80_keeps_every_symbol_dso_local() {
    assert!(builder_target().dag.dso_local_symbols);
}
