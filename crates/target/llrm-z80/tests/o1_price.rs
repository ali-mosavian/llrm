//! The -O1 choice between allocations is priced on what is emitted.

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};
use llrm_z80::expand::{Line, price};

fn priced(level: Level) -> (u32, u32) {
    let ir = include_str!("data/o1_two_stores_loop.ll");
    let out = text(ir, "clear", 0, &Request::new(level)).unwrap_or_else(|why| panic!("{why}"));
    let lines: Vec<Line> = out.lines().filter(|one| one.starts_with('\t')).map(|one| Line::Inst(one.trim().to_owned())).collect();
    price(&lines)
}

/// Priced before the printer's peepholes, `clear` -O1 (two stores of a counter through two arrays) chose the
/// allocation that is larger and slower once emitted: 96 B / 577 T against 95 B / 550 T at -Os. Priced as emitted
/// it is the same 96 B for 548 T.
#[test]
fn o1_is_no_slower_than_os_where_it_is_larger() {
    let (o1, os) = (priced(Level::O1), priced(Level::Os));
    assert!(o1.0 <= os.0 && o1.1 <= os.1 || o1.0 > os.0 && o1.1 < os.1, "O1 {o1:?} is larger and not faster than Os {os:?}");
}
