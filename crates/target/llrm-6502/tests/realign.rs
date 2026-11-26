//! An object aligned past what the frame keeps was refused
//! ("needs stack realignment, not supported yet"),
//! so `over_aligned_local_after_even_pushes` had no 6502 build. The frame now
//! rounds `rs0` down in the prologue, keeps the old one in a slot and reads
//! stack arguments from it.

mod common;

use llrm_6502::driver::{Level, Request, assembly};

fn ir(body: &str) -> String {
    format!("target datalayout = \"{}\"\n\ndeclare void @use(ptr)\n{body}", common::pipeline::datalayout())
}

const PROBE: &str = "
define i8 @probe() {
entry:
  %x = alloca [16 x i8], align 16
  %p = getelementptr [16 x i8], ptr %x, i16 0, i16 3
  store i8 77, ptr %p
  call void @use(ptr %x)
  %v = load i8, ptr %p
  ret i8 %v
}
";

/// The sim65 program: `probe` called with `rs0` moved down by `pushed` bytes,
/// as a caller with that many bytes of frame does. `use` keeps the low byte of
/// its pointer. The code is 200 when `rs0` came back changed, 201 when the
/// address was not 16-aligned, else what `probe` returned.
fn run(
    name: &str,
    compiled: &str,
    pushed: u32,
    entry: &str,
) -> i32 {
    let functions: String =
        compiled.lines().filter(|line| !line.contains(" = $")).map(|line| format!("{line}\n")).collect();
    let set: String = (8..16).map(|n| format!("  lda #{}\n  sta rc{n}\n", 160 + n)).collect();
    let check: String = (8..16).map(|n| format!("  lda rc{n}\n  cmp #{}\n  bne gbad\n", 160 + n)).collect();
    let used = if compiled.contains("seen:") {
        ""
    } else {
        "use:\n  lda rc2\n  sta seen\n  rts\n.segment \"BSS\"\nseen: .res 1\n.segment \"CODE\"\n"
    };
    let extra = format!(
        "{functions}
g:
{set}  sec
  lda rc0
  sbc #{pushed}
  sta rc0
  lda rc1
  sbc #0
  sta rc1
  jsr {entry}
  tax
  clc
  lda rc0
  adc #{pushed}
  sta rc0
  lda rc1
  adc #0
  sta rc1
{check}  txa
  rts
gbad:
  lda #202
  rts
{used}"
    );
    let main = "  lda #0\n  sta rc0\n  lda #$F0\n  sta rc1\n  jsr g\n  tax\n  lda rc1\n  cmp #$F0\n  bne bad\n  lda rc0\n  bne bad\n  lda seen\n  and #15\n  bne bad2\n  txa\n  jmp done\nbad:\n  lda #200\n  jmp done\nbad2:\n  lda #201\ndone:\n";
    common::run_with(name, main, &extra)
}

#[test]
fn a_local_aligned_to_16_is_16_aligned_on_a_frame_of_any_size() {
    for level in [Level::O0, Level::O1, Level::O2, Level::Os] {
        let compiled = assembly(&ir(PROBE), &Request::new(level)).unwrap();
        for pushed in 0..4 {
            let code = run(&format!("realign-{level:?}-{pushed}"), &compiled, pushed, "probe");
            assert_eq!(code, 77, "{level:?}, {pushed} bytes pushed (200: rs0 not restored, 201: address not aligned)");
        }
    }
}

#[test]
fn the_prologue_rounds_rs0_down_and_a_frame_that_keeps_its_alignment_does_not() {
    let text = assembly(&ir(PROBE), &Request::new(Level::O2)).unwrap();
    assert!(text.contains("and #240"), "no rounding of rs0 to 16:\n{text}");
    for align in [2, 8] {
        let kept = PROBE.replace("[16 x i8], align 16", &format!("[16 x i8], align {align}"));
        let text = assembly(&ir(&kept), &Request::new(Level::O2)).unwrap();
        assert!(!text.contains("and #"), "an align-{align} local realigned:\n{text}");
    }
}

/// Values live across calls in a loop go in rs4..rs7, which the function saves.
/// It can call itself, so the saves are on the soft stack and are reloaded from
/// `rs0`: after the old pointer came back they read the caller's bytes, and the
/// caller's rs4..rs7 came back wrong (202).
const WITH_SAVED: &str = "
define i16 @callee(i16 %n, i16 %a, i16 %b) noinline {
entry:
  %x = alloca [16 x i8], align 16
  br label %loop

loop:
  %i = phi i16 [ 0, %entry ], [ %i2, %loop ]
  %s = phi i16 [ 0, %entry ], [ %s2, %loop ]
  call void @use(ptr %x)
  %t = add i16 %s, %a
  %s2 = add i16 %t, %b
  %i2 = add i16 %i, 1
  %end = icmp eq i16 %i2, %n
  br i1 %end, label %exit, label %loop

exit:
  %again_p = icmp eq i16 %a, 999
  br i1 %again_p, label %again, label %done

again:
  %q = call i16 @callee(i16 1, i16 0, i16 0)
  br label %done

done:
  ret i16 %s2
}

define i8 @driver() {
entry:
  %r = call i16 @callee(i16 3, i16 10, i16 20)
  %t = trunc i16 %r to i8
  ret i8 %t
}
";

#[test]
fn a_realigned_frame_gives_the_callee_saved_cells_back() {
    for level in [Level::O1, Level::O2, Level::Os] {
        let compiled = assembly(&ir(WITH_SAVED), &Request::new(level)).unwrap();
        assert!(compiled.contains("rc8"), "{level:?}: no preserved cell used:\n{compiled}");
        for pushed in 0..3 {
            let code = run(&format!("realign-saved-{level:?}-{pushed}"), &compiled, pushed, "driver");
            assert_eq!(code, 90, "{level:?}, {pushed} bytes pushed (202: a preserved cell changed)");
        }
    }
}

/// A function that cannot be re-entered has its locals made global storage,
/// which is not aligned: the over-aligned one stays in the frame. As a global
/// its address was arbitrary while `ptr & 15` folded to 0.
const NOT_REENTERED: &str = "
@seen = global i8 0

define void @use(ptr %p) noinline {
entry:
  %i = ptrtoint ptr %p to i16
  %t = trunc i16 %i to i8
  store volatile i8 %t, ptr @seen
  ret void
}

define i8 @probe() {
entry:
  %pad = alloca [3 x i8]
  %x = alloca [16 x i8], align 16
  store volatile i8 1, ptr %pad
  %p = getelementptr [16 x i8], ptr %x, i16 0, i16 3
  store volatile i8 77, ptr %p
  call void @use(ptr %x)
  %v = load volatile i8, ptr %p
  ret i8 %v
}
";

#[test]
fn an_over_aligned_local_of_a_function_never_re_entered_stays_aligned() {
    let module = ir(NOT_REENTERED).replace("declare void @use(ptr)\n", "");
    for level in [Level::O1, Level::O2, Level::Os] {
        let compiled = assembly(&module, &Request::new(level)).unwrap();
        for pushed in 0..3 {
            let code = run(&format!("realign-static-{level:?}-{pushed}"), &compiled, pushed, "probe");
            assert_eq!(code, 77, "{level:?}, {pushed} bytes pushed (201: address not aligned):\n{compiled}");
        }
    }
}

/// Stack arguments are read at the offsets they had at entry, from the old
/// `rs0`: through the new one they were the realigned frame's own bytes.
const WITH_ARGUMENTS: &str = "
define i16 @callee(i16 %a, i16 %b, i16 %c, i16 %d, i16 %e, i16 %f, i16 %g, i16 %h) noinline {
entry:
  %x = alloca [16 x i8], align 16
  store i8 1, ptr %x
  call void @use(ptr %x)
  %s1 = add i16 %a, %b
  %s2 = add i16 %s1, %c
  %s3 = add i16 %s2, %d
  %s4 = add i16 %s3, %e
  %s5 = add i16 %s4, %f
  %s6 = add i16 %s5, %g
  %s7 = add i16 %s6, %h
  ret i16 %s7
}

define i8 @driver() {
entry:
  %r = call i16 @callee(i16 1, i16 2, i16 4, i16 8, i16 16, i16 32, i16 64, i16 3)
  %t = trunc i16 %r to i8
  ret i8 %t
}
";

#[test]
fn stack_arguments_are_read_through_the_old_stack_pointer() {
    for level in [Level::O0, Level::O2, Level::Os] {
        let compiled = assembly(&ir(WITH_ARGUMENTS), &Request::new(level)).unwrap();
        for pushed in 0..3 {
            let code = run(&format!("realign-args-{level:?}-{pushed}"), &compiled, pushed, "driver");
            assert_eq!(code, 130, "{level:?}, {pushed} bytes pushed");
        }
    }
}

/// Eight values live across a call do not fit the registers that survive it.
/// The function can call itself, so its spill slots are on the soft stack:
/// above the aligned locals, with the old `rs0` read back after them.
const WITH_SPILLS: &str = "
define i16 @callee(i16 %a, i16 %b, i16 %c, i16 %d, i16 %e, i16 %f, i16 %g, i16 %h) noinline {
entry:
  %x = alloca [16 x i8], align 16
  %t1 = mul i16 %a, 3
  %t2 = mul i16 %b, 5
  %t3 = mul i16 %c, 7
  %t4 = mul i16 %d, 9
  %t5 = mul i16 %e, 11
  %t6 = mul i16 %f, 13
  %t7 = mul i16 %g, 15
  %t8 = mul i16 %h, 17
  %again_p = icmp eq i16 %a, 999
  br i1 %again_p, label %again, label %done

again:
  %q = call i16 @callee(i16 0, i16 0, i16 0, i16 0, i16 0, i16 0, i16 0, i16 0)
  call void @use(ptr %x)
  br label %done

done:
  %s1 = add i16 %t1, %t2
  %s2 = add i16 %s1, %t3
  %s3 = add i16 %s2, %t4
  %s4 = add i16 %s3, %t5
  %s5 = add i16 %s4, %t6
  %s6 = add i16 %s5, %t7
  %s7 = add i16 %s6, %t8
  ret i16 %s7
}

define i8 @driver() {
entry:
  %r = call i16 @callee(i16 1, i16 1, i16 1, i16 1, i16 1, i16 1, i16 1, i16 1)
  %t = trunc i16 %r to i8
  ret i8 %t
}
";

#[test]
fn a_realigned_frame_with_spills_returns_the_right_value_and_the_old_stack_pointer() {
    for level in [Level::O1, Level::O2, Level::Os] {
        let compiled = assembly(&ir(WITH_SPILLS), &Request::new(level)).unwrap();
        assert!(!compiled.contains("s_callee"), "{level:?}: the spills are not on the stack:\n{compiled}");
        for pushed in 0..3 {
            let code = run(&format!("realign-spill-{level:?}-{pushed}"), &compiled, pushed, "driver");
            assert_eq!(code, 80, "{level:?}, {pushed} bytes pushed");
        }
    }
}

#[test]
fn a_realigned_frame_with_a_dynamic_alloca_is_refused_by_name() {
    let text = ir("
define i8 @f(i16 %n) {
entry:
  %a = alloca [16 x i8], align 16
  %d = alloca i8, i16 %n
  call void @use(ptr %a)
  call void @use(ptr %d)
  ret i8 0
}
");
    for level in [Level::O0, Level::O2] {
        let error = assembly(&text, &Request::new(level)).unwrap_err();
        assert!(error.contains("DynamicStackAlloc"), "{level:?}: {error}");
    }
}
