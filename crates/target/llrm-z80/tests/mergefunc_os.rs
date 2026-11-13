//! Two functions of one body are one function at -Os and -Oz
//! (`MergeFunctions`).

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

const IR: &str = "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8\"
declare void @use(i32)
define internal i32 @twice(i32 %x) noinline { %r = shl i32 %x, 1\n  ret i32 %r }
define internal i32 @twice_signed(i32 %x) noinline { %r = shl i32 %x, 1\n  ret i32 %r }
define void @f(i32 %x) {
  %a = call i32 @twice(i32 %x)
  call void @use(i32 %a)
  %b = call i32 @twice_signed(i32 %x)
  call void @use(i32 %b)
  ret void
}
";

fn callees(level: Level) -> Vec<String> {
    let out = text(IR, "f", 0, &Request::new(level)).expect("compiles");
    out.lines().filter_map(|line| line.trim().strip_prefix("call ")).map(str::to_owned).collect()
}

/// shl_long_by_one -Os kept `twice` and `twice_signed`, two copies of `add hl,
/// hl; rl e; rl d; ret`: 150 B against SDCC's 147. Both calls name one
/// function.
#[test]
fn two_functions_of_one_body_are_one_at_the_size_levels() {
    for level in [Level::Os, Level::Oz] {
        let calls = callees(level);
        let twice: Vec<_> = calls.iter().filter(|name| name.contains("twice")).collect();
        assert_eq!(twice.len(), 2, "{level:?}: {calls:?}");
        assert_eq!(twice[0], twice[1], "{level:?}: {calls:?}");
    }
}

/// Above the size levels the copies stay: a merged body is a call to a shared
/// name, never faster.
#[test]
fn the_speed_levels_keep_both() {
    let calls = callees(Level::O2);
    let twice: Vec<_> = calls.iter().filter(|name| name.contains("twice")).collect();
    assert!(twice.len() < 2 || twice[0] != twice[1], "{calls:?}");
}

/// shl_long_by_one -O1 kept `twice` and `twice_signed` (151 B against SDCC's
/// 147): a local function of the same body as another is dropped at -O1 too,
/// where no thunk is written (a thunk saves bytes and costs a `jmp`).
#[test]
fn at_o1_a_local_duplicate_is_dropped_and_an_external_one_is_not_thunked() {
    let calls = callees(Level::O1);
    let twice: Vec<_> = calls.iter().filter(|name| name.contains("twice")).collect();
    assert_eq!(twice.len(), 2, "{calls:?}");
    assert_eq!(twice[0], twice[1], "{calls:?}");
    let external = IR.replace("define internal i32 @twice_signed", "define i32 @twice_signed");
    let out = text(&external, "f", 0, &Request::new(Level::O1)).expect("compiles");
    let calls: Vec<&str> = out.lines().filter_map(|line| line.trim().strip_prefix("call ")).collect();
    assert!(calls.iter().filter(|name| name.contains("twice")).any(|name| name.contains("twice_signed")), "{calls:?}");
}
