//! A loop's peeled first test and its latch's copy are one block, and a `jmp`
//! taken where a flag is known is a branch (2026-11-15): k_stpcpy 31 B,
//! k_strchr 38 B and k_strcpy 31 B against llvm-mos's 30, 37 and 30 at -O2.

mod common;

use common::pipeline::datalayout;
use llrm_6502::asm::Asm;
use llrm_6502::dagisel::hooks::Cpu;
use llrm_6502::driver::{Level, Request, assembly};
use llrm_6502::peephole::after_layout;
use llrm_6502::short_jump::jump_to_branch;
use llrm_6502::tail_merge::merge_opposite_tails;
use llrm_6502::timing::estimate;

const STPCPY: &str = "define ptr @f(ptr %dst, ptr %src) {
entry:
  %c = load i8, ptr %src
  store i8 %c, ptr %dst
  %z = icmp eq i8 %c, 0
  br i1 %z, label %out, label %loop
loop:
  %s = phi ptr [ %s1, %loop ], [ %src, %entry ]
  %d = phi ptr [ %d1, %loop ], [ %dst, %entry ]
  %s1 = getelementptr inbounds i8, ptr %s, i16 1
  %d1 = getelementptr inbounds i8, ptr %d, i16 1
  %v = load i8, ptr %s1
  store i8 %v, ptr %d1
  %e = icmp eq i8 %v, 0
  br i1 %e, label %out, label %loop
out:
  %r = phi ptr [ %dst, %entry ], [ %d1, %loop ]
  ret ptr %r
}";

const STRCHR: &str = "define ptr @f(ptr %s, i16 %c) {
entry:
  %t = trunc i16 %c to i8
  %a = load i8, ptr %s
  %q = icmp eq i8 %a, %t
  br i1 %q, label %out, label %head
head:
  %v = phi i8 [ %n, %step ], [ %a, %entry ]
  %p = phi ptr [ %p1, %step ], [ %s, %entry ]
  %z = icmp eq i8 %v, 0
  br i1 %z, label %out, label %step
step:
  %p1 = getelementptr inbounds i8, ptr %p, i16 1
  %n = load i8, ptr %p1
  %m = icmp eq i8 %n, %t
  br i1 %m, label %out, label %head
out:
  %r = phi ptr [ %s, %entry ], [ null, %head ], [ %p1, %step ]
  ret ptr %r
}";

const STRCPY: &str = "define ptr @f(ptr %dst, ptr %src) {
entry:
  br label %loop
loop:
  %s = phi ptr [ %src, %entry ], [ %s1, %loop ]
  %d = phi ptr [ %dst, %entry ], [ %d1, %loop ]
  %s1 = getelementptr inbounds i8, ptr %s, i16 1
  %v = load i8, ptr %s
  %d1 = getelementptr inbounds i8, ptr %d, i16 1
  store i8 %v, ptr %d
  %e = icmp eq i8 %v, 0
  br i1 %e, label %out, label %loop
out:
  ret ptr %dst
}";

fn body(ir: &str) -> (String, usize) {
    let ir = format!("target datalayout = \"{}\"\n{ir}\n", datalayout());
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    let from = text.find("f:").unwrap();
    let body: String = text[from..].split("  rts").next().unwrap().to_owned() + "  rts\n";
    let bytes = estimate(Cpu::Mos6502, &body).unwrap_or_else(|e| panic!("{e}\n{body}")).bytes as usize;
    (body, bytes)
}

fn text(input: &str) -> String {
    let mut asm = Asm::parse(input);
    merge_opposite_tails(&mut asm);
    jump_to_branch(&mut asm);
    asm.text()
}

/// stpcpy tested its byte before the loop and again at the latch: 31 B, now
/// the entry jumps into the latch's load, store and branch.
#[test]
fn stpcpy_loads_and_stores_once() {
    let (body, bytes) = body(STPCPY);
    assert!(bytes <= 29, "{bytes} B:\n{body}");
    assert_eq!(body.matches("lda (rc4),y").count(), 1, "{body}");
}

/// strchr compared the first byte before the loop and again at the latch (38 B,
/// llvm-mos 37), and left `bne h; jmp x` where the branch before it says Z is
/// set.
#[test]
fn strchr_compares_once_and_has_no_jump() {
    let (body, bytes) = body(STRCHR);
    assert!(bytes <= 36, "{bytes} B:\n{body}");
    assert!(!body.contains("jmp"), "{body}");
}

/// strcpy entered its loop by `ldy #0` after four copy instructions and a
/// 3-byte `jmp` (31 B, llvm-mos 30).
#[test]
fn strcpy_enters_its_loop_by_a_branch() {
    let (body, bytes) = body(STRCPY);
    assert!(bytes <= 30, "{bytes} B:\n{body}");
    assert!(!body.contains("jmp"), "{body}");
}

#[test]
fn the_entry_test_that_the_latch_repeats_is_a_jump_into_the_latch() {
    let before = "f:\n  ldy #0\n  lda (rc4),y\n  sta (rc2),y\n  beq x\nh:\n  inc rc4\n  lda (rc4),y\n  sta (rc2),y\n  bne h\nx:\n  rts\n";
    let mut asm = Asm::parse(before);
    merge_opposite_tails(&mut asm);
    assert_eq!(
        asm.text(),
        "f:\n  ldy #0\n  jmp _tm1\nh:\n  inc rc4\n_tm1:\n  lda (rc4),y\n  sta (rc2),y\n  bne h\nx:\n  rts\n"
    );
}

#[test]
fn a_tail_of_one_two_byte_instruction_is_kept() {
    // `tax; beq x` is 3 B, as long as the `jmp` that would replace it
    let before = "f:\n  tax\n  beq x\nh:\n  inc rc4\n  tax\n  bne h\nx:\n  rts\n";
    let mut asm = Asm::parse(before);
    merge_opposite_tails(&mut asm);
    assert_eq!(asm.text(), before);
}

#[test]
fn a_loop_whose_branches_are_not_opposite_is_kept() {
    let before =
        "f:\n  lda (rc4),y\n  sta (rc2),y\n  beq x\nh:\n  inc rc4\n  lda (rc4),y\n  sta (rc2),y\n  beq h\nx:\n  rts\n";
    let mut asm = Asm::parse(before);
    merge_opposite_tails(&mut asm);
    assert_eq!(asm.text(), before);
}

#[test]
fn a_jump_after_a_fall_through_branch_is_the_opposite_branch() {
    assert_eq!(text("f:\n  bne a\n  jmp b\na:\n  inx\nb:\n  rts\n"), "f:\n  bne a\n  beq b\na:\n  inx\nb:\n  rts\n");
}

#[test]
fn a_label_before_the_jump_keeps_it() {
    let before = "f:\n  bne a\nc:\n  jmp b\na:\n  inx\nb:\n  rts\n";
    assert_eq!(text(before), before);
}

#[test]
fn an_index_load_moves_down_to_the_jump_that_it_sets_the_flag_for() {
    assert_eq!(
        text("f:\n  ldy #0\n  lda rc2\n  sta rc6\n  jmp t\n  inc rc4\nt:\n  lda (rc4),y\n  bne f\n  rts\n"),
        "f:\n  lda rc2\n  sta rc6\n  ldy #0\n  beq t\n  inc rc4\nt:\n  lda (rc4),y\n  bne f\n  rts\n"
    );
}

#[test]
fn an_index_load_stays_when_the_target_reads_the_flags() {
    let before = "f:\n  ldy #0\n  lda rc2\n  jmp t\n  inc rc4\nt:\n  beq f\n  rts\n";
    assert_eq!(text(before), before);
}

#[test]
fn an_index_load_stays_when_the_work_between_reads_the_index() {
    let before = "f:\n  ldy #0\n  lda (rc2),y\n  jmp t\n  inc rc4\nt:\n  lda rc5\n  rts\n";
    assert_eq!(text(before), before);
}

#[test]
fn a_target_out_of_reach_keeps_the_jump() {
    let mut before = String::from("f:\n  ldy #0\n  jmp t\n");
    for _ in 0..60 {
        before.push_str("  lda rc2\n  sta rc3\n");
    }
    before.push_str("t:\n  rts\n");
    assert_eq!(text(&before), before);
}

#[test]
fn the_passes_run_after_the_layout() {
    let mut asm = Asm::parse(
        "f:\n  ldy #0\n  lda (rc4),y\n  sta (rc2),y\n  beq x\nh:\n  inc rc4\n  lda (rc4),y\n  sta (rc2),y\n  bne h\nx:\n  rts\n",
    );
    after_layout(&mut asm);
    assert_eq!(asm.text().matches("lda (rc4),y").count(), 1, "{}", asm.text());
}

/// k_strrchr at -O2 went from 38 to 40 B (+17 cy) when these passes ran inside
/// the passes the layout choice is priced on: the jump-in layout got two bytes
/// smaller, was kept, and kept the variants from reaching the 38 B layout. They
/// run after the choice (`peephole::after_layout`). Needs `llrm-cc` built
/// (`LLRM_CC`).
#[test]
fn strrchr_keeps_its_layout_when_the_passes_shrink_another() {
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("strrchr-layout");
    std::fs::create_dir_all(&dir).unwrap();
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/bench/hand/strrchr.c");
    let made = std::process::Command::new(common::pipeline::llrm_cc())
        .current_dir(&dir)
        .args(["--target=6502-sim65", "-O2", "-S", "-w", "-o", "o.s"])
        .arg(&src)
        .output()
        .unwrap();
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    let text = std::fs::read_to_string(dir.join("o.s")).unwrap();
    let from = text.find("k_strrchr:").unwrap();
    let body: String = text[from..].split("  rts").next().unwrap().to_owned() + "  rts\n";
    let bytes = estimate(Cpu::Mos6502, &body).unwrap_or_else(|e| panic!("{e}\n{body}")).bytes;
    assert!(bytes <= 38, "{bytes} B:\n{body}");
}
