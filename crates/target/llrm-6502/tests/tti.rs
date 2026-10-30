//! The 6502 cost model (`llrm_6502::tti`) reaches the passes that price code.

mod common;

use common::pipeline::datalayout;

/// The mid-end's last stage of `ir` at -O2, as `llrm-6502-llc` dumps it.
fn final_ir(ir: &str) -> String {
    static CALLS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let call = CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("llrm6502-tti-{}-{call}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("f.ll");
    std::fs::write(&source, format!("target datalayout = \"{}\"\n{ir}\n", datalayout())).unwrap();
    let ran = std::process::Command::new(env!("CARGO_BIN_EXE_llrm-6502-llc"))
        .args(["--platform=sim65", "-S", "-O2", "-o", dir.join("f.s").to_str().unwrap(), source.to_str().unwrap()])
        .env("LLRM_MIR_STAGES", dir.join("stages"))
        .status()
        .unwrap();
    assert!(ran.success());
    let mut stages: Vec<_> = std::fs::read_dir(dir.join("stages"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "ll"))
        .collect();
    stages.sort();
    let text = std::fs::read_to_string(stages.last().unwrap()).unwrap();
    std::fs::remove_dir_all(dir).ok();
    text
}

/// `DivRemPairs` wrote the remainder of `x / y` and `x % y` as `x - (x / y) * y` because the target said it
/// had no divide that makes both: itoa called `__udivhi3` and `__mulhi3` per digit and grew 9 bytes. The 6502
/// runtime makes both in one call, so the pair stays a `udiv` and a `urem`.
#[test]
fn a_quotient_and_remainder_of_the_same_operands_stay_a_pair() {
    let text = final_ir(
        "define i16 @f(i16 %x, i16 %y) {
  %q = udiv i16 %x, %y
  %r = urem i16 %x, %y
  %s = add i16 %q, %r
  ret i16 %s
}",
    );
    assert!(text.contains("urem"), "the remainder was expanded:\n{text}");
    assert!(!text.contains("mul"), "the remainder was expanded:\n{text}");
}

/// With every operation priced 1 whatever its width, `SimplifyCFG` ran the arm
/// of `if (c) r = a == 0 ? 3 : 0` on both paths: a 32-bit select (four byte
/// selects) for the sake of one branch. `strnicmp` did that to its tail and
/// came out 247 bytes against 147.
#[test]
fn a_long_select_is_not_speculated_to_save_a_branch() {
    let text = final_ir(
        "define i32 @f(i1 %c, i8 %a) {
  br i1 %c, label %t, label %m
t:
  %x = icmp eq i8 %a, 0
  %s = select i1 %x, i32 3, i32 0
  br label %m
m:
  %r = phi i32 [ 1, %0 ], [ %s, %t ]
  ret i32 %r
}",
    );
    assert!(text.contains("br i1 %c"), "the arm was run on both paths:\n{text}");
}

/// A word load priced above `parts` made the compare and the load feeding it (2
/// + 3) exceed the two-entry-phi budget of 4, so `c >= '0' && c <= '9'` stayed
/// two branches and never became the range check: isdigit 28 -> 50 bytes,
/// isalpha 30 -> 74.
#[test]
fn a_range_test_of_a_word_is_flattened_and_becomes_one_compare() {
    let text = final_ir(
        "define i16 @k_isdigit(i16 noundef %0) {
  %2 = alloca i16, align 2
  store i16 %0, ptr %2, align 2
  %3 = load i16, ptr %2, align 2
  %4 = icmp sge i16 %3, 48
  br i1 %4, label %5, label %8
5:
  %6 = load i16, ptr %2, align 2
  %7 = icmp sle i16 %6, 57
  br label %8
8:
  %9 = phi i1 [ false, %1 ], [ %7, %5 ]
  %10 = zext i1 %9 to i16
  ret i16 %10
}",
    );
    assert!(text.contains("add i16 %0, -48") && text.contains("icmp ult i16 %"), "two compares remain:\n{text}");
}

/// The CRC-16 loop of `crc16` (clang's `-O0`-shaped IR) stays a loop: at -O2
/// the pin's cost model turns it into a 512-byte Sarwate table (596 B, 1275 cy
/// against 88 B, 6221 cy; cc65 -O is 259 B, 18578 cy), which loses on size. It
/// stays a loop because this cost model keeps the inner `if` a branch, so the
/// CRC pass never sees a one-block loop. Decision: crc16 is the small loop.
#[test]
fn the_crc16_loop_does_not_become_a_lookup_table() {
    let text = final_ir(
        "define i16 @crc16(ptr %p, i8 %n) {
  %crc = alloca i16
  %i = alloca i8
  %j = alloca i8
  store i16 -1, ptr %crc
  store i8 0, ptr %i
  br label %outer
outer:
  %iv = load i8, ptr %i
  %iz = zext i8 %iv to i16
  %nz = zext i8 %n to i16
  %c0 = icmp slt i16 %iz, %nz
  br i1 %c0, label %body, label %done
body:
  %q = getelementptr inbounds nuw i8, ptr %p, i16 %iz
  %b = load i8, ptr %q
  %bz = zext i8 %b to i16
  %sh = shl i16 %bz, 8
  %c1 = load i16, ptr %crc
  %x = xor i16 %c1, %sh
  store i16 %x, ptr %crc
  store i8 0, ptr %j
  br label %inner
inner:
  %jv = load i8, ptr %j
  %jz = zext i8 %jv to i16
  %c2 = icmp slt i16 %jz, 8
  br i1 %c2, label %step, label %next
step:
  %v = load i16, ptr %crc
  %top = and i16 %v, -32768
  %set = icmp ne i16 %top, 0
  br i1 %set, label %yes, label %no
yes:
  %v1 = load i16, ptr %crc
  %s1 = shl i16 %v1, 1
  %r1 = xor i16 %s1, 4129
  br label %join
no:
  %v2 = load i16, ptr %crc
  %r2 = shl i16 %v2, 1
  br label %join
join:
  %r = phi i16 [ %r1, %yes ], [ %r2, %no ]
  store i16 %r, ptr %crc
  %jn = add i8 %jv, 1
  store i8 %jn, ptr %j
  br label %inner
next:
  %in = add i8 %iv, 1
  store i8 %in, ptr %i
  br label %outer
done:
  %res = load i16, ptr %crc
  ret i16 %res
}",
    );
    assert!(!text.contains("crctable"), "{text}");
    assert!(text.contains("phi"), "{text}");
}
