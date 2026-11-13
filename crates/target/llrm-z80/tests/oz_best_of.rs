//! DEVIATE z80-oz-best-of: -Oz keeps the -Os compile of a module when it is smaller.

use llrm_z80::compile::{prepare, text_in};
use llrm_z80::driver::{Level, Request};

/// local_initializers: `sum` is called twice and a constant array fills `numbers`. -Os inlines `sum` and folds the
/// array away (215 B); -Oz, whose inline threshold is 5, kept the calls and the array (319 B, +104 B over -Os).
const IR: &str = include_str!("data/oz_larger_than_os.ll");

fn module(request: &Request) -> String {
    let prepared = prepare(IR, request).expect("prepares");
    let mut out = String::new();
    for (number, name) in prepared.names().enumerate() {
        out.push_str(&text_in(&prepared, name, number, request).expect("compiles"));
    }
    out.push_str(&llrm_z80::data::text(IR, request).expect("data"));
    out
}

#[test]
fn oz_is_never_larger_than_os_where_the_module_is_smaller_at_os() {
    let os = module(&Request::new(Level::Os));
    let plain = module(&Request { oz_best_of: false, ..Request::new(Level::Oz) });
    let oz = module(&Request::new(Level::Oz));
    assert!(plain.lines().count() > os.lines().count(), "the premise: -Oz alone is larger ({} vs {} lines)", plain.lines().count(), os.lines().count());
    assert_eq!(oz, os, "-Oz kept its own larger compile");
}
