//! A function built for size calls the runtime where that makes the linked
//! program smaller (DEVIATE M68K-SIZE-LIBCALL, `src/libcalls.rs`): the sites of
//! one translation unit pay for a member once, so one `cttz.i64` stays inline,
//! the second one flips both to `__ctzdi2`.

use llrm_m68k::compile::{Options, text_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::libcalls::Mode;

mod support;

const HEAD: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\n";

/// `uses` functions each counting trailing zeros of an i64.
fn ctz64(uses: usize) -> String {
    let mut ir = format!("{HEAD}declare i64 @llvm.cttz.i64(i64, i1)\n");
    for n in 0..uses {
        ir.push_str(&format!(
            "define i32 @f{n}(i64 %x) optsize {{ %r = call i64 @llvm.cttz.i64(i64 %x, i1 true) %t = trunc i64 %r to i32 ret i32 %t }}\n"
        ));
    }
    ir
}

fn asm(
    ir: &str,
    name: &str,
    cpu: Cpu,
    options: Options,
) -> String {
    let module = llrm_mir::parse::module(ir).expect("parses");
    text_with(&module, name, cpu, options).expect("compiles")
}

/// One use of `cttz.i64` at -Os: the member (48 B) is not worth 32 B saved.
#[test]
fn one_ctz64_site_stays_inline_two_call_the_runtime() {
    let options = Options { size: true, ..Options::default() };
    let one = asm(&ctz64(1), "f0", Cpu::M68020, options);
    assert!(!one.contains("__ctzdi2"), "{one}");
    let two = asm(&ctz64(2), "f0", Cpu::M68020, options);
    assert!(two.contains("__ctzdi2"), "{two}");
    let three = asm(&ctz64(3), "f1", Cpu::M68020, options);
    assert!(three.contains("__ctzdi2"), "{three}");
}

/// Speed levels keep the inline expansion whatever the count.
#[test]
fn a_function_built_for_speed_never_calls() {
    let out = asm(&ctz64(3).replace(" optsize", ""), "f0", Cpu::M68020, Options::default());
    assert!(!out.contains("__ctzdi2"), "{out}");
}

/// The 68000 and 68010 always call (M68K-BC1); the choice is the 68020's.
#[test]
fn the_choice_is_made_for_the_68020_and_later_only() {
    let out = asm(&ctz64(1), "f0", Cpu::M68000, Options { size: true, ..Options::default() });
    assert!(out.contains("__ctzdi2"), "{out}");
}

/// `Options::size_libcalls` overrides the choice.
#[test]
fn the_mode_forces_one_way() {
    let call = Options { size: true, size_libcalls: Mode::Call, ..Options::default() };
    assert!(asm(&ctz64(1), "f0", Cpu::M68020, call).contains("__ctzdi2"));
    let inline = Options { size: true, size_libcalls: Mode::Inline, ..Options::default() };
    assert!(!asm(&ctz64(3), "f0", Cpu::M68020, inline).contains("__ctzdi2"));
}

use llrm_m68k::compile::items_with;

/// Three variable 64-bit shifts of the corpus's `shuttle_value`, whose
/// operands are masked or zero-extended: the expansion is about 32 B a site,
/// not the 62 B of a plain one, so `__ashldi3` (72 B) saves 16 B and costs
/// more. With a flat 62 B column the pass called and the linked program was
/// 230 B -> 286 B at -Os.
const SHUTTLE: &str = "define i64 @g(i64 %x, i32 %y, i32 %z) optsize {
  %yy = zext i32 %y to i64
  %s = lshr i64 %x, %yy
  %m = and i64 %s, 4294967295
  %zm = and i32 %z, 31
  %zz = zext i32 %zm to i64
  %w = shl i64 %m, %zz
  %wm = and i64 %w, 4294967295
  %z2 = zext i32 %z to i64
  %w2 = shl i64 %wm, %z2
  ret i64 %w2
}\n";

#[test]
fn shifts_whose_operands_are_narrow_stay_inline_when_the_call_would_not_save_bytes() {
    let options = Options { mid_end: false, ..Options::default() };
    let out = asm(&format!("{HEAD}{SHUTTLE}"), "g", Cpu::M68020, options);
    assert!(!out.contains("jsr"), "{out}");
    // the same count of plain shifts does call
    let plain: String = (0..3)
        .map(|n| format!("define i64 @s{n}(i64 %x, i64 %n) optsize {{ %r = shl i64 %x, %n ret i64 %r }}\n"))
        .collect();
    assert!(asm(&format!("{HEAD}{plain}"), "s0", Cpu::M68020, options).contains("jsr\t__ashldi3"));
}

/// A member another site already links costs this site nothing: a `ctpop.i64`
/// calls `__popcountdi2` (32 B, which needs `__popcountsi2`), so the
/// `ctpop.i32` beside it calls too, though alone its 62 B beat the 14 + 70 B
/// call.
#[test]
fn a_member_one_site_links_is_free_to_the_next() {
    let ir = format!(
        "{HEAD}declare i32 @llvm.ctpop.i32(i32)\ndeclare i64 @llvm.ctpop.i64(i64)\n\
         define i32 @narrow(i32 %x) optsize {{ %r = call i32 @llvm.ctpop.i32(i32 %x) ret i32 %r }}\n\
         define i32 @wide(i64 %x) optsize {{ %r = call i64 @llvm.ctpop.i64(i64 %x) %t = trunc i64 %r to i32 ret i32 %t }}\n"
    );
    let options = Options { mid_end: false, ..Options::default() };
    let alone = asm(
        &ir.replace("define i32 @wide", "define i32 @wide_unused")
            .replace("optsize { %r = call i64", "{ %r = call i64"),
        "narrow",
        Cpu::M68020,
        options,
    );
    assert!(!alone.contains("__popcountsi2"), "{alone}");
    let together = asm(&ir, "narrow", Cpu::M68020, options);
    assert!(together.contains("__popcountsi2"), "{together}");
}

/// Variable 64-bit shifts are the second class of the same table: two sites
/// call `__ashldi3` (2 x 24 + 72 B against 2 x 62 B); a constant amount stays
/// inline whatever the count.
#[test]
fn variable_wide_shifts_call_the_runtime_from_the_second_site_and_constant_ones_never() {
    let shifts = |amount: &str, uses: usize| {
        let mut ir = HEAD.to_owned();
        for n in 0..uses {
            ir.push_str(&format!(
                "define i64 @s{n}(i64 %x, i64 %n) optsize {{ %r = shl i64 %x, {amount} ret i64 %r }}\n"
            ));
        }
        ir
    };
    let options = Options { mid_end: false, ..Options::default() };
    assert!(!asm(&shifts("%n", 1), "s0", Cpu::M68020, options).contains("__ashldi3"));
    assert!(asm(&shifts("%n", 2), "s0", Cpu::M68020, options).contains("__ashldi3"));
    assert!(!asm(&shifts("7", 3), "s0", Cpu::M68020, options).contains("__ashldi3"));
}

/// Both ways give the same answers on the hardware: the programs run the
/// count and shift routines the choice calls (defined here, built for speed so
/// they stay inline) and exit 42 when every result is right.
mod executed {
    use llrm_m68k::object::{executable, start, write_executable};

    use super::*;
    use crate::support;

    /// The runtime unit: the routines with the compiler-rt signatures, built
    /// for speed.
    const RUNTIME: &str = "declare i64 @llvm.cttz.i64(i64, i1)\ndeclare i32 @llvm.ctpop.i32(i32)\n\
         define i32 @__ctzdi2(i64 %x) { %r = call i64 @llvm.cttz.i64(i64 %x, i1 false) %t = trunc i64 %r to i32 ret i32 %t }\n\
         define i32 @__popcountsi2(i32 %x) { %r = call i32 @llvm.ctpop.i32(i32 %x) ret i32 %r }\n\
         define i64 @__ashldi3(i64 %x, i32 %n) { %w = zext i32 %n to i64 %r = shl i64 %x, %w ret i64 %r }\n\
         declare i64 @llvm.ctpop.i64(i64)\n\
         define i32 @__popcountdi2(i64 %x) { %r = call i64 @llvm.ctpop.i64(i64 %x) %t = trunc i64 %r to i32 ret i32 %t }\n\
         define i64 @__lshrdi3(i64 %x, i32 %n) { %w = zext i32 %n to i64 %r = lshr i64 %x, %w ret i64 %r }\n\
         define i64 @__ashrdi3(i64 %x, i32 %n) { %w = zext i32 %n to i64 %r = ashr i64 %x, %w ret i64 %r }\n";

    /// The user unit: `uses` functions of each class built for size, and `f`,
    /// which exits 42 when every result is right. Its calls to the routines
    /// are external.
    fn program(
        uses: usize,
        wide: bool,
    ) -> (String, Vec<String>) {
        let mut ir = format!(
            "{HEAD}declare i64 @llvm.cttz.i64(i64, i1)\ndeclare i32 @llvm.ctpop.i32(i32)\ndeclare i64 @llvm.ctpop.i64(i64)\n"
        );
        let mut names = Vec::new();
        for n in 0..uses {
            if wide {
                ir.push_str(&format!(
                    "define i32 @d{n}(i64 %x) optsize {{ %r = call i64 @llvm.ctpop.i64(i64 %x) %t = trunc i64 %r to i32 ret i32 %t }}\n\
                     define i64 @r{n}(i64 %x, i64 %n) optsize {{ %r = lshr i64 %x, %n ret i64 %r }}\n\
                     define i64 @a{n}(i64 %x, i64 %n) optsize {{ %r = ashr i64 %x, %n ret i64 %r }}\n"
                ));
                names.extend([format!("d{n}"), format!("r{n}"), format!("a{n}")]);
                continue;
            }
            ir.push_str(&format!(
                "define i32 @c{n}(i64 %x) optsize {{ %r = call i64 @llvm.cttz.i64(i64 %x, i1 true) %t = trunc i64 %r to i32 ret i32 %t }}\n\
                 define i32 @p{n}(i32 %x) optsize {{ %r = call i32 @llvm.ctpop.i32(i32 %x) ret i32 %r }}\n\
                 define i64 @s{n}(i64 %x, i64 %n) optsize {{ %r = shl i64 %x, %n ret i64 %r }}\n"
            ));
            names.extend([format!("c{n}"), format!("p{n}"), format!("s{n}")]);
        }
        // each wrong answer adds its own number to the exit status
        let mut checks = String::from("define i32 @f() {\n  %a0 = add i32 0, 0\n");
        let mut at = 0;
        let mut check = |checks: &mut String, call: String, ty: &str, expected: i64| {
            at += 1;
            checks.push_str(&format!(
                "  %r{at} = {call}\n  %w{at} = icmp ne {ty} %r{at}, {expected}\n  %z{at} = zext i1 %w{at} to i32\n  \
                 %m{at} = mul i32 %z{at}, {at}\n  %a{at} = add i32 %a{}, %m{at}\n",
                at - 1
            ));
        };
        for n in 0..uses {
            if wide {
                check(&mut checks, format!("call i32 @d{n}(i64 -1)"), "i32", 64);
                check(&mut checks, format!("call i32 @d{n}(i64 1311768467463790320)"), "i32", 32);
                check(&mut checks, format!("call i64 @r{n}(i64 -1, i64 40)"), "i64", 0xFFFFFF);
                check(&mut checks, format!("call i64 @r{n}(i64 -9223372036854775808, i64 63)"), "i64", 1);
                check(&mut checks, format!("call i64 @a{n}(i64 -9223372036854775808, i64 33)"), "i64", -1i64 << 30);
                check(&mut checks, format!("call i64 @a{n}(i64 4294967296, i64 0)"), "i64", 4294967296);
                continue;
            }
            check(&mut checks, format!("call i32 @c{n}(i64 4294967296)"), "i32", 32);
            check(&mut checks, format!("call i32 @c{n}(i64 -9223372036854775808)"), "i32", 63);
            check(&mut checks, format!("call i32 @p{n}(i32 -1)"), "i32", 32);
            check(&mut checks, format!("call i32 @p{n}(i32 305419896)"), "i32", 13);
            check(&mut checks, format!("call i64 @s{n}(i64 1, i64 40)"), "i64", 1 << 40);
            check(&mut checks, format!("call i64 @s{n}(i64 -1, i64 33)"), "i64", -1i64 << 33);
            check(&mut checks, format!("call i64 @s{n}(i64 3, i64 0)"), "i64", 3);
        }
        checks.push_str(&format!(
            "  %ok = icmp eq i32 %a{at}, 0\n  %s = select i1 %ok, i32 42, i32 1\n  ret i32 %s\n}}\n"
        ));
        ir.push_str(&checks);
        names.push("f".to_owned());
        (ir, names)
    }

    /// The user unit's text and items, then the runtime's.
    fn linked(
        uses: usize,
        wide: bool,
        cpu: Cpu,
    ) -> (String, Vec<llrm_m68k::compile::Item>) {
        let options = Options { mid_end: false, ..Options::default() };
        let (user, names) = program(uses, wide);
        let user = llrm_mir::parse::module(&user).expect("parses");
        // local labels are numbered by function: the runtime's follow the user
        // unit's
        let pad: String = (0..names.len()).map(|n| format!("define void @pad{n}() {{ ret void }}\n")).collect();
        let runtime = llrm_mir::parse::module(&format!("{HEAD}{pad}{RUNTIME}")).expect("parses");
        let mut items = start("f", &[]);
        let mut text = String::new();
        for function in &names {
            items.extend(items_with(&user, function, cpu, options).unwrap_or_else(|why| panic!("{cpu:?}: {why}")));
            text.push_str(&text_with(&user, function, cpu, options).unwrap());
        }
        for function in ["__ctzdi2", "__popcountsi2", "__ashldi3", "__popcountdi2", "__lshrdi3", "__ashrdi3"] {
            items.extend(items_with(&runtime, function, cpu, options).unwrap_or_else(|why| panic!("{cpu:?}: {why}")));
        }
        (text, items)
    }

    #[test]
    fn one_two_and_three_uses_agree_on_qemu_at_68020_68040_68060() {
        let Some(qemu) = support::qemu() else { return };
        for uses in 1..=3 {
            for (cpu, name) in [(Cpu::M68020, "m68020"), (Cpu::M68040, "m68040"), (Cpu::M68060, "m68060")] {
                let (text, items) = linked(uses, false, cpu);
                // one use of each class stays inline, from the second the sites
                // call
                for routine in ["__ctzdi2", "__popcountsi2", "__ashldi3"] {
                    assert_eq!(text.contains(&format!("jsr\t{routine}")), uses >= 2, "{uses} uses {cpu:?} {routine}");
                }
                let path = support::unique_path("size_libcalls");
                write_executable(&path, &executable(&items).unwrap()).unwrap();
                let code = support::run(&qemu, &["-cpu", name], &path);
                std::fs::remove_file(&path).ok();
                assert_eq!(code, Some(42), "{uses} uses {cpu:?}");
            }
        }
    }
    /// `ctpop.i64` and the right shifts, the other classes of the table: the
    /// 64-bit count calls from one use (32 B + `__popcountsi2` against 124 B),
    /// the shifts from two.
    #[test]
    fn wide_popcount_and_right_shifts_agree_on_qemu_at_68020_68040_68060() {
        let Some(qemu) = support::qemu() else { return };
        for uses in 1..=3 {
            for (cpu, name) in [(Cpu::M68020, "m68020"), (Cpu::M68040, "m68040"), (Cpu::M68060, "m68060")] {
                let (text, items) = linked(uses, true, cpu);
                for (routine, from) in [("__popcountdi2", 1), ("__lshrdi3", 2), ("__ashrdi3", 2)] {
                    assert_eq!(
                        text.contains(&format!("jsr\t{routine}")),
                        uses >= from,
                        "{uses} uses {cpu:?} {routine}"
                    );
                }
                let path = support::unique_path("size_libcalls_wide");
                write_executable(&path, &executable(&items).unwrap()).unwrap();
                let code = support::run(&qemu, &["-cpu", name], &path);
                std::fs::remove_file(&path).ok();
                assert_eq!(code, Some(42), "{uses} uses {cpu:?}");
            }
        }
    }
}

mod driver {
    use llrm_m68k::compile::CodeModel;
    use llrm_m68k::driver::{Level, Policy, assembly_flagged};

    use super::*;

    fn text(
        ir: &str,
        level: Level,
    ) -> String {
        assembly_flagged(ir, Cpu::M68020, level, false, None, CodeModel::Small, &Policy::default()).expect("compiles")
    }

    /// -Os chooses by bytes per module and -O2 never calls, whatever the
    /// functions' attributes.
    #[test]
    fn the_driver_chooses_at_os_and_never_at_o2() {
        let one = ctz64(1).replace(" optsize", "");
        let three = ctz64(3).replace(" optsize", "");
        assert!(!text(&one, Level::Os).contains("__ctzdi2"));
        assert!(text(&three, Level::Os).contains("__ctzdi2"));
        assert!(!text(&three, Level::O2).contains("__ctzdi2"));
    }
}

/// The translation unit that defines the routine never calls it: `__ctzdi2`
/// built for size with three `cttz.i64` sites (its own and two more) called
/// itself, an endless recursion.
#[test]
fn a_routine_is_never_called_from_the_unit_that_defines_it() {
    let mut ir = ctz64(2);
    ir.push_str(
        "define i32 @__ctzdi2(i64 %x) optsize { %r = call i64 @llvm.cttz.i64(i64 %x, i1 true) %t = trunc i64 %r to i32 ret i32 %t }\n",
    );
    let out = asm(&ir, "__ctzdi2", Cpu::M68020, Options { mid_end: false, ..Options::default() });
    assert!(!out.contains("jsr\t__ctzdi2"), "{out}");
}
