//! A memset of a constant count: an inline `ldir` fill or the `__memseta` call.

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

const HEADER: &str = "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8\"\n\
    declare void @llvm.memset.p0.i16(ptr, i8, i16, i1)\n\
    @a = global [128 x i8] zeroinitializer\n\
    @b = global [256 x i8] zeroinitializer\n\
    define void @clear() {\n\
      call void @llvm.memset.p0.i16(ptr @a, i8 0, i16 128, i1 false)\n\
      call void @llvm.memset.p0.i16(ptr @b, i8 7, i16 256, i1 false)\n\
      ret void\n\
    }\n";

fn compiled(
    ir: &str,
    level: Level,
) -> String {
    text(ir, "clear", 0, &Request::new(level)).unwrap_or_else(|why| panic!("{why}"))
}

/// pattern_fill -Os called `__memseta` for its two constant clears: 10 B a site
/// and a 21 B routine linked for them, against 13 B a site for `ld (hl), v; ld
/// e, l; ld d, h; inc de; ld bc, n - 1; ldir` and no routine (459 B against
/// 443, SDCC 440).
#[test]
fn constant_fills_are_inline_ldir_where_no_other_site_needs_the_routine() {
    for level in [Level::Os, Level::O1] {
        let out = compiled(HEADER, level);
        assert!(!out.contains("__memseta") && out.matches("ldir").count() == 2, "{level:?}:\n{out}");
        assert!(out.contains("ld bc, #127") && out.contains("ld bc, #255"), "{level:?}: the count is one less:\n{out}");
    }
}

/// A fill of a variable count links `__memseta` anyway: the constant site is
/// then 3 B smaller as the call.
#[test]
fn a_constant_fill_stays_a_call_where_the_routine_is_linked_anyway() {
    let ir = format!(
        "{HEADER}define void @any(ptr %p, i16 %n) {{\n\
           call void @llvm.memset.p0.i16(ptr %p, i8 0, i16 %n, i1 false)\n\
           ret void\n\
         }}\n"
    );
    let out = compiled(&ir, Level::Os);
    assert!(out.contains("__memseta") && !out.contains("ldir"), "{out}");
}
