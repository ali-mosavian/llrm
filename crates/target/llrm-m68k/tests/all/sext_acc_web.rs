//! early_exit_index -O2 on the 68000 kept three sums `s -= (a != b)` as `sne;
//! ext.w; ext.l; sub.l` (8 B each, +10 B over gcc's `suba.w`): each sum was
//! priced alone, with a move for every neighbour that stayed in a data
//! register, though the neighbours are the other sums.

use llrm_m68k::driver::{Level, assembly, cpu_of};

#[test]
fn sums_chained_through_a_loop_are_all_taken_to_an_address_register() {
    let ir = include_str!("../data/early_exit_index.ll");
    for level in [Level::O2, Level::O3] {
        let out = assembly(ir, cpu_of("68000").unwrap(), level).expect("compiles");
        let subtracted = out.lines().filter(|l| l.trim_start().starts_with("suba.w")).count();
        assert!(
            subtracted == 3 && !out.contains("ext.l"),
            "{level:?}: {subtracted} suba.w, the sums stay in data registers:\n{out}"
        );
    }
}

const WORDS: usize = 12;

/// `f(n)` stores `i * 9000 + n` as words and sums them sign-extended in a
/// straight line.
fn words_ir() -> String {
    let mut ir = String::from(
        "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
define i32 @f(i32 %n) {
entry:
  %t = alloca [12 x i16]
  br label %fill
fill:
  %i = phi i32 [0, %entry], [%i1, %fill]
  %v = mul i32 %i, 9000
  %w = add i32 %v, %n
  %h = trunc i32 %w to i16
  %p = getelementptr i16, ptr %t, i32 %i
  store i16 %h, ptr %p
  %i1 = add i32 %i, 1
  %c = icmp slt i32 %i1, 12
  br i1 %c, label %fill, label %sum
sum:
",
    );
    for k in 0..WORDS {
        ir += &format!(
            "  %q{k} = getelementptr i16, ptr %t, i32 {k}\n  %l{k} = load i16, ptr %q{k}\n  %s{k} = sext i16 %l{k} to i32\n"
        );
    }
    ir += "  %a1 = add i32 %s0, %s1\n";
    for k in 2..WORDS {
        ir += &format!("  %a{k} = add i32 %a{}, %s{k}\n", k - 1);
    }
    ir + &format!("  ret i32 %a{}\n}}\n", WORDS - 1)
}

/// A sum of sign-extended words whose first add has two of them: that
/// add cannot take an address register, and the adds after it were priced as
/// if it could. It was assembled `add.l %d0, %d0` into an address register,
/// and loop_epilogue_sum and loop_fusion -O2/-O3 on the 68000 printed other
/// sums than gcc's.
#[test]
fn a_sum_whose_first_add_keeps_its_data_registers_is_the_same_under_qemu() {
    use llrm_m68k::compile::{Options, items_with};
    use llrm_m68k::dagisel::hooks::Cpu;
    use llrm_m68k::object::{executable, start, write_executable};

    let Some(qemu) = crate::support::qemu() else { return };
    let n: i32 = 1234;
    let expected: i32 = (0..WORDS as i32).map(|i| i32::from((i * 9000 + n) as i16)).sum();
    let mut all = start("f", &[i64::from(n)]);
    all.extend(
        items_with(&llrm_mir::parse::module(&words_ir()).expect("parses"), "f", Cpu::M68000, Options::default())
            .unwrap(),
    );
    let path = crate::support::unique_path("sext_words");
    write_executable(&path, &executable(&all).unwrap()).unwrap();
    let code = crate::support::run(&qemu, &["-cpu", "m68000"], &path);
    std::fs::remove_file(&path).ok();
    assert_eq!(code, Some(expected & 0xff), "sum");
}
