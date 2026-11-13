//! DEVIATE z80-oz-best-of: the -Os the choice weighs -Oz against is -Os as clang makes it, without `minsize`.

use llrm_z80::compile::{prepare, text_in};
use llrm_z80::driver::{Level, Request};

/// A switch with a dominant 22-case run. `minsize` stops the pin peeling it, so the whole range was one 28-entry jump
/// table (56 B of `.dw`); -Os peels the run and tables 5. -Oz kept the table, 26 B above its own -Os, because the
/// -Os it weighed carried `minsize` too.
const IR: &str = include_str!("data/minsize_switch_table.ll");

fn module(
    ir: &str,
    request: &Request,
) -> String {
    let prepared = prepare(ir, request).expect("prepares");
    let mut out = String::new();
    for (number, name) in prepared.names().enumerate() {
        out.push_str(&text_in(&prepared, name, number, request).expect("compiles"));
    }
    out
}

#[test]
fn oz_keeps_the_os_compile_when_minsize_is_what_made_it_larger() {
    let os = module(&IR.replace("minsize ", ""), &Request::new(Level::Os));
    let oz = module(IR, &Request::new(Level::Oz));
    let tables = |text: &str| text.matches(".dw").count();
    assert!(tables(&os) < 10, "the premise: -Os peels the run ({} .dw)", tables(&os));
    assert_eq!(oz, os, "-Oz kept a table of {} .dw against -Os's {}", tables(&oz), tables(&os));
}
