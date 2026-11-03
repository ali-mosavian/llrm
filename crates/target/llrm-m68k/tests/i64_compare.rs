//! A 64-bit `<` is the low words' borrow into a `subx` of the high words
//! (`SETCCCARRY`). The legalizer had no lowering for it (`Unsupported
//! SetCCCarry` for every i64 compare, so `__floatdisf` did not compile), and
//! once it had, `negadd` turned the borrow's `add.b #-1` into `subq.b #1`
//! because the scan for readers of the codes stopped at `subx`, which sets
//! them, before counting that it reads the extend flag: every compare of
//! operands with equal high words answered 0.

use llrm_m68k::compile::{Options, items_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::object::{executable, start, write_executable};

mod support;

const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";

const VALUES: [u64; 7] = [0, 1, u64::MAX, 0xffff_ffff, 0x1_0000_0000, 0x8000_0000_0000_0000, 0x7fff_ffff_ffff_ffff];

/// The count of pairs `(a, b)` whose `ult` or `slt` answer differs from the
/// host's, with the values taken from volatile slots so nothing folds.
fn module() -> llrm_mir::module::Module {
    let mut body = String::from("define i32 @f() {\n  %x = alloca i64\n  %y = alloca i64\n  br label %b0\n");
    let mut block = 0;
    let mut bad = String::from("0");
    for (i, &a) in VALUES.iter().enumerate() {
        for (j, &b) in VALUES.iter().enumerate() {
            for (op, want) in [("ult", a < b), ("slt", (a as i64) < (b as i64))] {
                let k = format!("{i}_{j}_{op}");
                body.push_str(&format!(
                    "b{block}:\n  store volatile i64 {}, ptr %x\n  store volatile i64 {}, ptr %y\n  %a{k} = load volatile i64, ptr %x\n  %b{k} = load volatile i64, ptr %y\n  %c{k} = icmp {op} i64 %a{k}, %b{k}\n  %w{k} = zext i1 %c{k} to i32\n  %d{k} = xor i32 %w{k}, {}\n  %n{k} = add i32 {bad}, %d{k}\n  br label %b{}\n",
                    a as i64,
                    b as i64,
                    u8::from(want),
                    block + 1
                ));
                bad = format!("%n{k}");
                block += 1;
            }
        }
    }
    body.push_str(&format!("b{block}:\n  ret i32 {bad}\n}}"));
    llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{body}")).expect("parses")
}

#[test]
fn a_64_bit_less_than_agrees_with_the_host_under_qemu() {
    let Some(qemu) = support::qemu() else { return };
    let module = module();
    for (cpu, flag) in [(Cpu::M68000, "m68000"), (Cpu::M68020, "m68020")] {
        let mut all = start("f", &[]);
        all.extend(items_with(&module, "f", cpu, Options { mid_end: false, ..Options::default() }).unwrap());
        let path = support::unique_path("i64cmp");
        write_executable(&path, &executable(&all).unwrap()).unwrap();
        let code = support::run(&qemu, &["-cpu", flag], &path);
        std::fs::remove_file(&path).ok();
        assert_eq!(code, Some(0), "{cpu:?}: comparisons that differ from the host's");
    }
}
