//! A 64-bit add or subtract was the compare form: `add.l; cmp.l; bcc; moveq`
//! for the carry, then another add (i64 add: 8 instructions and a branch
//! against the pin's `add.l; addx.l`). `ExpandIntRes_ADDSUB` takes `ADDC` and
//! `ADDE` where the target has them, and m68k's lowering makes `add`/`addx`.

use llrm_m68k::compile::{Options, items_with, text_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::object::{executable, start, write_executable};

mod support;

const IR: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
define i64 @f(i64 %a, i64 %b) {
  %r = add i64 %a, %b
  ret i64 %r
}
define i64 @g(i64 %a, i64 %b) {
  %r = sub i64 %a, %b
  ret i64 %r
}
";

#[test]
fn a_64_bit_sum_and_difference_carry_through_the_extend_flag() {
    let module = llrm_mir::parse::module(IR).expect("parses");
    for (name, high) in [("f", "addx.l"), ("g", "subx.l")] {
        for cpu in [Cpu::M68000, Cpu::M68020] {
            let text = text_with(&module, name, cpu, Options::default()).expect("compiles");
            assert!(text.contains(high), "{name} {cpu:?}: {text}");
            assert!(!text.contains("cmp.l") && !text.contains("bcc"), "{name} {cpu:?}: {text}");
        }
    }
}

/// Operands that carry and borrow across the halves, and wrap at 64 bits.
const VALUES: [u64; 6] = [0, 1, 0xffff_ffff, 0x1_0000_0000, 0x8000_0000_0000_0000, u64::MAX];

/// `f()` counts the sums and differences, with the operands from volatile
/// slots, that differ from the host's.
fn sums_and_differences() -> String {
    let mut body = String::from("define i32 @f() {\n  %p = alloca i64\n  %q = alloca i64\n  br label %b0\n");
    let (mut bad, mut k) = (String::from("0"), 0);
    for &a in &VALUES {
        for &b in &VALUES {
            for (op, want) in [("add", a.wrapping_add(b)), ("sub", a.wrapping_sub(b))] {
                body.push_str(&format!(
                    "b{k}:\n  store volatile i64 {a}, ptr %p\n  store volatile i64 {b}, ptr %q\n  %a{k} = load volatile i64, ptr %p\n  %b{k} = load volatile i64, ptr %q\n  %r{k} = {op} i64 %a{k}, %b{k}\n  %e{k} = icmp ne i64 %r{k}, {want}\n  %z{k} = zext i1 %e{k} to i32\n  %s{k} = add i32 {bad}, %z{k}\n  br label %b{}\n",
                    k + 1
                ));
                bad = format!("%s{k}");
                k += 1;
            }
        }
    }
    body.push_str(&format!("b{k}:\n  ret i32 {bad}\n}}"));
    body
}

#[test]
fn a_64_bit_sum_and_difference_match_the_host_across_the_carry() {
    let Some(qemu) = support::qemu() else { return };
    let ir = sums_and_differences();
    let module = llrm_mir::parse::module(&format!(
        "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\n{ir}"
    ))
    .expect("parses");
    for (cpu, flag) in [(Cpu::M68000, "m68000"), (Cpu::M68020, "m68020")] {
        for options in [Options::default(), Options { size: true, ..Options::default() }] {
            let mut all = start("f", &[]);
            all.extend(items_with(&module, "f", cpu, options).expect("compiles"));
            let path = support::unique_path("carry_chain");
            write_executable(&path, &executable(&all).unwrap()).unwrap();
            let code = support::run(&qemu, &["-cpu", flag], &path);
            std::fs::remove_file(&path).ok();
            assert_eq!(code, Some(0), "{cpu:?} size={}: sums or differences that differ from the host's", options.size);
        }
    }
}

/// `sum += f(a, b)` over a 64-bit xorshift state
/// (shl_of_a_zero_extended_dividend -O1): `addx.l` takes data registers on both
/// halves, the old compare form let the high half live in `%a2`. With the add
/// as `addx` alone the loop spilled and reloaded the low half every trip: +12 B
/// and +280 clk.
const LOOP: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
declare i32 @f(i32, i32)
declare void @report(i32)
define i32 @main() {
  br label %1
1:
  %2 = phi i64 [ 0, %0 ], [ %17, %1 ]
  %3 = phi i64 [ 88172645463325252, %0 ], [ %10, %1 ]
  %4 = phi i32 [ 0, %0 ], [ %18, %1 ]
  %5 = shl i64 %3, 13
  %6 = xor i64 %5, %3
  %7 = lshr i64 %6, 7
  %8 = xor i64 %7, %6
  %9 = shl i64 %8, 17
  %10 = xor i64 %9, %8
  %11 = trunc i64 %10 to i32
  %12 = lshr i64 %10, 32
  %13 = trunc i64 %12 to i32
  %14 = or i32 %13, 1
  %15 = call i32 @f(i32 %11, i32 %14)
  %16 = zext i32 %15 to i64
  %17 = add i64 %2, %16
  %18 = add i32 %4, 1
  %19 = icmp eq i32 %18, 200000
  br i1 %19, label %20, label %1
20:
  %21 = trunc i64 %17 to i32
  call void @report(i32 %21)
  ret i32 0
}
";

#[test]
fn the_compare_form_is_kept_where_addx_runs_out_of_data_registers() {
    use llrm_m68k::timing::Price;
    let module = llrm_mir::parse::module(LOOP).expect("parses");
    let options = Options { strict: true, ..Options::default() };
    let kept = items_with(&module, "main", Cpu::M68000, options).expect("compiles");
    let plain = items_with(&module, "main", Cpu::M68000, Options { carry_chain: false, ..options }).expect("compiles");
    let price = Price::of(false);
    assert!(
        !llrm_m68k::trial::keeps(&plain, &kept, Cpu::M68000, price),
        "the compare form is cheaper than the kept build"
    );
}

/// `sbb_wide_sub`: a 64-bit `zext(a) - zext(b)` is `sub.l; negx.l`, and the
/// trigger of the trial named only `addx` and `subx`, so the compare form was
/// never tried for it.
#[test]
fn a_negx_counts_as_carrying_through_the_extend_flag() {
    use llrm_m68k::mfunc::MFunction;
    use llrm_spike_mf::{Block, Inst, Kind};
    let with = |name: &'static str| {
        let mut function = MFunction::new("f");
        function.blocks.push(Block { id: 0, insts: vec![Inst::new(Kind::Op(name), vec![])], phis: vec![] });
        llrm_m68k::compile::uses_extend(&function)
    };
    for name in ["ADDX32dd", "ADDX8dd", "SUBX16dd", "NEGX32d", "NEGX8d"] {
        assert!(with(name), "{name}");
    }
    assert!(!with("MOV32dd") && !with("ADD32dd"));
}
