//! A word load or store whose pointer steps by two afterwards is
//! post-incremented.

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

const DATALAYOUT: &str = r#"target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8""#;

fn compile(
    body: &str,
    name: &str,
    level: Level,
) -> String {
    text(&format!("{DATALAYOUT}\n{body}"), name, 0, &Request::new(level)).expect("compiles")
}

/// A word copy loop: each pointer went back with `dec hl` and forward with `ld
/// bc, #2; add hl, bc` (7 B and 32 T a trip a pointer); the access steps it as
/// it goes.
#[test]
fn a_word_copy_loop_steps_its_pointers_with_the_access() {
    let asm = compile(
        r#"define void @copy(ptr %d, ptr %s, ptr %end) nounwind {
entry:
  %c = icmp eq ptr %s, %end
  br i1 %c, label %out, label %body
body:
  %q = phi ptr [ %s, %entry ], [ %n, %body ]
  %r = phi ptr [ %d, %entry ], [ %m, %body ]
  %x = load i16, ptr %q
  store i16 %x, ptr %r
  %n = getelementptr i8, ptr %q, i16 2
  %m = getelementptr i8, ptr %r, i16 2
  %e = icmp eq ptr %n, %end
  br i1 %e, label %out, label %body
out:
  ret void
}
"#,
        "copy",
        Level::O2,
    );
    assert!(!asm.contains("dec hl"), "a pointer went back after the access:\n{asm}");
    assert!(!asm.contains("ld bc, #2"), "a pointer stepped by an add of two:\n{asm}");
}

/// The byte forms stay: `ld a, (de)` and `inc de` need no `hl`. A first cut
/// made them post-increments through `hl` and a byte copy loop grew by 6 B.
#[test]
fn a_byte_copy_loop_keeps_its_pointers_out_of_hl() {
    let asm = compile(
        r#"define void @bytes(ptr %d, ptr %s, ptr %end) nounwind {
entry:
  br label %body
body:
  %q = phi ptr [ %s, %entry ], [ %n, %body ]
  %r = phi ptr [ %d, %entry ], [ %m, %body ]
  %x = load i8, ptr %q
  store i8 %x, ptr %r
  %n = getelementptr i8, ptr %q, i16 1
  %m = getelementptr i8, ptr %r, i16 1
  %e = icmp eq ptr %n, %end
  br i1 %e, label %out, label %body
out:
  ret void
}
"#,
        "bytes",
        Level::O2,
    );
    assert!(!asm.contains("push ix"), "the byte loop took a frame:\n{asm}");
}

/// A pointer whose step's carry is read (an overflow test on the new pointer)
/// keeps its `add`: the merged `inc hl` sets no carry.
#[test]
fn a_step_whose_carry_is_read_is_not_merged() {
    let asm = compile(
        r#"define i16 @walk(ptr %p, ptr %end) nounwind {
entry:
  br label %body
body:
  %q = phi ptr [ %p, %entry ], [ %n, %body ]
  %a = phi i16 [ 0, %entry ], [ %s, %body ]
  %x = load i16, ptr %q
  %s = add i16 %a, %x
  %n = getelementptr i8, ptr %q, i16 2
  %wrapped = icmp ult ptr %n, %q
  %e = icmp eq ptr %n, %end
  %stop = or i1 %wrapped, %e
  br i1 %stop, label %out, label %body
out:
  ret i16 %s
}
"#,
        "walk",
        Level::O2,
    );
    assert!(asm.contains("add hl"), "the carry of the step went with it:\n{asm}");
}
