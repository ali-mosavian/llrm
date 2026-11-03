//! A function alias was refused on m68k: `__attribute__((alias))` programs did
//! not compile. It is a second symbol at the aliasee's address now; an alias of
//! data is still refused, as the assembly carries no data.

use llrm_m68k::driver::{Level, cpu_of};
use llrm_m68k::kernels::DATALAYOUT;

const ALIAS: &str = "@a = alias i32 (), ptr @f

define i32 @f() {
entry:
  ret i32 0
}
";

const DATA: &str = "@v = global i32 0
@a = alias i32, ptr @v
";

#[test]
fn a_function_alias_is_a_symbol_at_the_functions_address() {
    let module = format!("target datalayout = \"{DATALAYOUT}\"\n{ALIAS}");
    let text = llrm_m68k::driver::assembly(&module, cpu_of("68000").unwrap(), Level::O2).expect("assembly");
    assert!(text.contains("\t.set\ta, f\n"), "{text}");
}

#[test]
fn an_alias_of_data_is_refused_on_m68k() {
    let error = llrm_m68k::driver::program(DATA).err().expect("an error");
    assert!(error.contains("cannot emit the alias @a"), "{error}");
}
