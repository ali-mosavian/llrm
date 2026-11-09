//! A stack array read at an index after a loop was addressed through a `lea`
//! of its frame slot, re-formed for every access because a call destroys the
//! register (`lea (4,%sp), %a0; move.l (0,%a0,%d2), -(%sp)`, 4 B and 8 clk
//! each): `move.l (4,%sp,%d2), -(%sp)` takes the stack pointer as the base.

use llrm_m68k::compile::{Options, items_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::driver::{Level, assembly, cpu_of};
use llrm_m68k::frameindexed::{fits, margin};
use llrm_m68k::object::{executable, start, write_executable};

mod support;

const IR: &str = r#"target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
define float @k(i32 %n) optsize {
  %a = alloca [6 x float], align 4
  %b = alloca [6 x float], align 4
  br label %loop
loop:
  %i = phi i32 [ 0, %0 ], [ %next, %loop ]
  %m7 = mul nuw nsw i32 %i, 7
  %x7 = add nsw i32 %m7, -15
  %f7 = sitofp i32 %x7 to float
  %pa = getelementptr inbounds nuw [4 x i8], ptr %a, i32 %i
  store float %f7, ptr %pa, align 4
  %m5 = mul nuw nsw i32 %i, 5
  %x5 = add nsw i32 %m5, -12
  %f5 = sitofp i32 %x5 to float
  %pb = getelementptr inbounds nuw [4 x i8], ptr %b, i32 %i
  store float %f5, ptr %pb, align 4
  %next = add nuw nsw i32 %i, 1
  %done = icmp eq i32 %next, 6
  br i1 %done, label %out, label %loop, !llvm.loop !0
out:
  %ra = getelementptr inbounds [4 x i8], ptr %a, i32 %n
  %rb = getelementptr inbounds [4 x i8], ptr %b, i32 %n
  %va = load float, ptr %ra, align 4
  %vb = load float, ptr %rb, align 4
  %s = fadd float %va, %vb
  ret float %s
}

!0 = distinct !{!0, !1}
!1 = !{!"llvm.loop.unroll.disable"}
"#;

#[test]
fn a_stack_array_indexed_by_a_register_is_addressed_from_the_stack_pointer() {
    for cpu in ["68000", "68020"] {
        for level in [Level::Os, Level::O2] {
            let out = assembly(IR, cpu_of(cpu).unwrap(), level).expect("compiles");
            let lines: Vec<&str> = out.lines().collect();
            for (at, line) in lines.iter().enumerate() {
                let Some(rest) = line.strip_prefix("\tlea\t(") else { continue };
                if !rest.contains(",%sp), %a") {
                    continue;
                }
                let register = rest.rsplit(' ').next().unwrap();
                let base = format!("(0,{register},");
                assert!(
                    !lines[at + 1..].iter().take(3).any(|next| next.contains(&base)),
                    "{cpu} {level:?}: a lea of a stack slot feeds an indexed operand:\n{out}"
                );
            }
            assert!(out.contains(",%sp,%d"), "{cpu} {level:?}: no (d,%sp,Xn) operand:\n{out}");
        }
    }
}

/// An indexed operand's displacement is 8 bits: -128 folds, 127 folds, one past
/// either end does not (the fold off by one is a silent miscompile at that
/// frame size: `move.l (128,%sp,%d1), -(%sp)` encodes as -128).
#[test]
fn the_eight_bit_range_is_exact() {
    assert!(fits(-128, 127));
    assert!(!fits(-129, 0), "low -129");
    assert!(!fits(0, 128), "high 128");
    assert!(fits(127, 127) && !fits(128, 128));
}

/// Room for what moves the stack pointer between the frame and the read: two
/// calls' arguments, the condition code and the frame's rounding.
#[test]
fn the_margin_is_two_call_areas_and_twelve() {
    assert_eq!(margin(0), 12);
    assert_eq!(margin(8), 28);
}

/// A slot `pad` bytes above the frame's bottom, read and written at an index
/// with the stack pointer where the frame has it (the call's two arguments
/// are stored, not pushed).
fn padded(pad: usize) -> String {
    format!(
        r#"target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
declare void @sink(ptr)
declare i32 @two(i32, i32)
define i32 @f(i32 %k) {{
  %a = alloca [4 x i32], align 4
  %pad = alloca [{pad} x i8], align 4
  call void @sink(ptr %pad)
  %pa = getelementptr inbounds [4 x i32], ptr %a, i32 0, i32 %k
  store volatile i32 5, ptr %pa
  %v1 = load volatile i32, ptr %pa
  %v2 = load volatile i32, ptr %pa
  %s = call i32 @two(i32 %v1, i32 %v2)
  ret i32 %s
}}
"#
    )
}

/// The displacements of the `(d,%sp,%dN)` operands in `text`.
fn sp_indexed(text: &str) -> Vec<i64> {
    text.lines()
        .filter_map(|line| {
            let at = line.find(",%sp,%d")?;
            line[..at].rsplit('(').next()?.parse().ok()
        })
        .collect()
}

/// The displacements of the `lea (d,%sp), %aN` that address a slot (not the
/// frame's own `%sp` adjustments).
fn slot_leas(text: &str) -> Vec<i64> {
    text.lines()
        .filter_map(|line| {
            let rest = line.strip_prefix("\tlea\t(")?;
            let (displacement, tail) = rest.split_once(",%sp), %a")?;
            (tail.len() == 1 && tail.bytes().all(|c| c.is_ascii_digit())).then(|| displacement.parse().ok()).flatten()
        })
        .collect()
}

/// The slot folds exactly where its displacement plus the margin for a call of
/// two arguments (8) fits: 96 folds, 100 does not (127 - 28 = 99). With the
/// guard loosened by one, the margin short by one or the pushed arguments left
/// out, 100 folds.
#[test]
fn a_slot_folds_exactly_where_the_displacement_and_the_margin_fit() {
    let (mut folded, mut kept) = (Vec::new(), Vec::new());
    for cpu in ["68000", "68020"] {
        for level in [Level::O2, Level::Os] {
            for pad in (4..140).step_by(4) {
                let out = assembly(&padded(pad), cpu_of(cpu).unwrap(), level).expect("compiles");
                let displacements = sp_indexed(&out);
                let reference = match displacements.first() {
                    Some(d) => *d,
                    None => *slot_leas(&out)
                        .iter()
                        .max()
                        .unwrap_or_else(|| panic!("no lea {cpu} {level:?} pad {pad}:\n{out}")),
                };
                assert_eq!(
                    !displacements.is_empty(),
                    reference <= 127 - margin(8),
                    "{cpu} {level:?} pad {pad}:\n{out}"
                );
                if displacements.is_empty() { kept.push(reference) } else { folded.push(reference) }
            }
        }
    }
    assert_eq!((folded.iter().max(), kept.iter().min()), (Some(&96), Some(&100)));
}

/// Two stack arrays of `n` words filled by a loop, read at `k` and passed to a
/// call: the arguments are pushed, so the second read is 4 B further from the
/// stack pointer than the frame says.
fn arrays(n: usize) -> String {
    format!(
        r#"target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
define i32 @two(i32 %x, i32 %y) noinline {{
  %r = add i32 %x, %y
  ret i32 %r
}}
define i32 @f(i32 %k) {{
  %a = alloca [{n} x i32], align 4
  %b = alloca [{n} x i32], align 4
  br label %loop
loop:
  %i = phi i32 [ 0, %0 ], [ %next, %loop ]
  %m7 = mul nuw nsw i32 %i, 7
  %x7 = add nsw i32 %m7, -15
  %pa = getelementptr inbounds nuw [4 x i8], ptr %a, i32 %i
  store i32 %x7, ptr %pa, align 4
  %m5 = mul nuw nsw i32 %i, 5
  %x5 = add nsw i32 %m5, -12
  %pb = getelementptr inbounds nuw [4 x i8], ptr %b, i32 %i
  store i32 %x5, ptr %pb, align 4
  %next = add nuw nsw i32 %i, 1
  %done = icmp eq i32 %next, 4
  br i1 %done, label %out, label %loop
out:
  %ra = getelementptr inbounds [4 x i8], ptr %a, i32 %k
  %rb = getelementptr inbounds [4 x i8], ptr %b, i32 %k
  %va = load i32, ptr %ra, align 4
  %vb = load i32, ptr %rb, align 4
  %s = call i32 @two(i32 %va, i32 %vb)
  ret i32 %s
}}
"#
    )
}

/// The displacement of an indexed operand is 8 bits: a fold judged without the
/// bytes a call's arguments push ahead of the second read left 128 in it
/// (`move.l (128,%sp,%d1), -(%sp)`, which no instruction encodes), so the
/// fold is off by the arguments pushed. Every size from a frame well under the
/// range to one well over it: the operand stays in range, some sizes fold, some
/// keep their `lea`.
#[test]
fn the_folded_displacement_stays_in_eight_bits_at_every_frame_size() {
    let (mut folded, mut kept) = (0, 0);
    for cpu in ["68000", "68020"] {
        for n in 8..=44 {
            let out = assembly(&arrays(n), cpu_of(cpu).unwrap(), Level::O2).expect("compiles");
            let displacements = sp_indexed(&out);
            assert!(
                displacements.iter().all(|d| (-128..=127).contains(d)),
                "{cpu} {n} words: {displacements:?}\n{out}"
            );
            folded += usize::from(!displacements.is_empty());
            kept += usize::from(!slot_leas(&out).is_empty());
        }
    }
    assert!(folded > 0 && kept > 0, "the sweep crossed no boundary: folded {folded}, kept {kept}");
}

/// The largest frame that still folds, run: the pushed argument and the
/// displacement at the edge of the range read the right words.
#[test]
fn the_largest_folded_frame_runs() {
    let Some(qemu) = support::qemu() else { return };
    let edge = (8..=44)
        .rev()
        .find(|n| {
            let out = assembly(&arrays(*n), cpu_of("68000").unwrap(), Level::O2).expect("compiles");
            sp_indexed(&out).iter().any(|d| *d >= 80)
        })
        .expect("a size that folds near the edge");
    let module = llrm_mir::parse::module(&arrays(edge)).expect("parses");
    for k in [0_i64, 1, 3] {
        let mut all = start("f", &[k]);
        all.extend(items_with(&module, "f", Cpu::M68000, Options::default()).unwrap());
        all.extend(items_with(&module, "two", Cpu::M68000, Options::default()).unwrap());
        let path = support::unique_path("frame_indexed");
        write_executable(&path, &executable(&all).unwrap()).unwrap();
        let code = support::run(&qemu, &["-cpu", "m68000"], &path);
        std::fs::remove_file(&path).ok();
        assert_eq!(code, Some(((12 * k - 27) & 0xff) as i32), "{edge} words, element {k}");
    }
}

/// The range proof assumes the stack pointer moves only for pushed arguments. A
/// dynamic `alloca` moves it by a run-time amount, so the function gets a frame
/// pointer and its locals are addressed through it, below the saved `%a6` and
/// the return address. The frame-pointer slot sits after the fixed objects in
/// the layout and every local's index was one short of it: the 32-byte array
/// was addressed `(0,%a6,%d0)`, over the saved `%a6`.
#[test]
fn a_dynamic_alloca_addresses_locals_below_the_frame_pointer() {
    let ir = r#"target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
declare void @sink(ptr)
define i32 @f(i32 %n, i32 %k) {
  %a = alloca [8 x i32], align 4
  %d = alloca i32, i32 %n, align 4
  call void @sink(ptr %d)
  %p = getelementptr inbounds [8 x i32], ptr %a, i32 0, i32 %k
  store volatile i32 7, ptr %p
  %v = load volatile i32, ptr %p
  ret i32 %v
}"#;
    for level in [Level::O2, Level::Os] {
        let text = assembly(ir, cpu_of("68000").unwrap(), level).expect("compiles");
        assert!(text.contains("link.w"), "{text}");
        // `(disp,%a6...)` with a displacement of 0 (the saved `%a6`) to 7 (the
        // return address): no local lives there
        let over_the_frame = text
            .lines()
            .any(
                |line| line.find(",%a6").is_some_and(|end| {
                    let head = &line[..end];
                    head.rfind('(')
                        .and_then(|start| head[start + 1..].parse::<i64>().ok())
                        .is_some_and(|disp| (0..8).contains(&disp))
                }),
            );
        assert!(!over_the_frame, "a local addressed over the saved frame pointer or the return address:\n{text}");
    }
}
