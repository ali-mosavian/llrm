//! A pointer stepped by a few bytes and a word filled by a loop, in the
//! cheapest form the Z80 has.

use std::process::Command;

fn assembly(
    name: &str,
    ir: &str,
) -> String {
    let dir = std::env::temp_dir().join(format!("llrm-stepping-z80-{name}"));
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("g.ll");
    std::fs::write(&source, format!("target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16\"\n{ir}"))
        .unwrap();
    let out = dir.join("g.s");
    let done = Command::new(env!("CARGO_BIN_EXE_llrm-z80-llc"))
        .args(["-S", "-O2", "-o", out.to_str().unwrap(), source.to_str().unwrap()])
        .output()
        .expect("the driver runs");
    assert_eq!(done.status.code(), Some(0), "{}", String::from_utf8_lossy(&done.stderr));
    std::fs::read_to_string(out).unwrap()
}

/// `p + 2` was `ld bc, #2; add hl, bc` (4 B, 21 T, bc lost).
#[test]
fn a_pointer_moved_by_two_is_two_incs() {
    let text = assembly("two", "define ptr @g(ptr %p) {\n  %q = getelementptr i8, ptr %p, i16 2\n  ret ptr %q\n}\n");
    assert!(!text.contains("add hl") && text.matches("inc ").count() == 2, "{text}");
}

/// A word stored through a pointer and the pointer stepped by two was `inc hl;
/// ld (hl), d; dec hl; ld bc, #2; add hl, bc`.
#[test]
fn a_word_fill_loop_steps_its_pointer_with_incs() {
    let text = assembly(
        "fill",
        "@a = global [64 x i16] zeroinitializer\n\
         define void @g(i16 %n, i16 %v) {\n\
         entry:\n  %c = icmp sgt i16 %n, 0\n  br i1 %c, label %loop, label %exit\n\
         loop:\n  %i = phi i16 [ 0, %entry ], [ %i1, %loop ]\n\
           %p = getelementptr inbounds i16, ptr @a, i16 %i\n  store i16 %v, ptr %p\n\
           %i1 = add nuw nsw i16 %i, 1\n  %d = icmp eq i16 %i1, %n\n  br i1 %d, label %exit, label %loop\n\
         exit:\n  ret void\n}\n",
    );
    // the loop is the text from the label the back edge jumps to
    let (_, body) = text.rsplit_once("jr nz, L0_").map(|(head, tail)| (head, tail)).expect("a back edge");
    let label = format!("L0_{}:", body.lines().next().unwrap().trim());
    let looped = &text[text.find(&label).expect("the loop label")..];
    assert!(!looped.contains("sla") && !looped.contains("dec hl") && !looped.contains("#2"), "{text}");
}
