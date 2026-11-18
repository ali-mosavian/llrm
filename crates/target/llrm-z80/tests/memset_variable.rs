//! A fill of a count known only at run time is an inline `ldir` behind a test
//! of the count where that makes the module smaller.

use llrm_z80::compile::{prepare, text_in};
use llrm_z80::driver::{Level, Request};

const IR: &str = "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8\"\n\
    declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg)\n\
    define void @fill(ptr %p, i16 %n) {\n  call void @llvm.memset.p0.i16(ptr %p, i8 0, i16 %n, i1 false)\n  ret void\n}\n\
";

fn module(level: Level) -> String {
    let request = Request::new(level);
    let prepared = prepare(IR, &request).expect("prepares");
    prepared
        .names()
        .enumerate()
        .map(|(number, name)| text_in(&prepared, name, number, &request).expect("compiles"))
        .collect()
}

/// `memset(p, 0, n)` called `__memseta` (a 22 B routine linked for it) at -Os,
/// where the inline `ldir` fill is 16 B; the variable count's fill was never
/// tried.
#[test]
fn a_variable_count_fill_at_os_is_an_ldir_not_a_routine() {
    let text = module(Level::Os);
    assert!(text.contains("ldir"), "no ldir:\n{text}");
    assert!(!text.contains("__memseta"), "calls the routine:\n{text}");
}
