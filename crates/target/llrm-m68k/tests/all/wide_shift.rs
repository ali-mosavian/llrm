//! An `i32` shift by a constant of 16 to 31 on the 68000 was `moveq #n, %dm;
//! lsr.l %dm, %dn`: 8 + 2n clocks and a register (`fix_usi`'s exponent,
//! `(x >> 23) & 255`, 58 of the 719,592 clocks per 3,000 vectors; the swap form
//! below took it to 684,462, under gcc's 707,210). Now the swap of the halves
//! and a shift of 15 or fewer, at the speed levels; `-Os` and the 68020 keep
//! the register form. Executed against C over every count and the edge values.

use std::path::Path;
use std::process::Command;

use llrm_m68k::driver::{Level, assembly, cpu_of};

use crate::support;

fn ir() -> String {
    let mut text =
        String::from("target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\n");
    for (name, op) in [("srl", "lshr"), ("shl", "shl"), ("sra", "ashr")] {
        for n in 16..32 {
            text += &format!("define i32 @{name}{n}(i32 %x) {{\n  %r = {op} i32 %x, {n}\n  ret i32 %r\n}}\n");
        }
    }
    text
}

#[test]
fn a_wide_constant_shift_is_a_swap_at_the_68000_speed_levels() {
    for level in [Level::O1, Level::O2, Level::O3] {
        let text = assembly(&ir(), cpu_of("68000").unwrap(), level).unwrap_or_else(|why| panic!("{level:?}: {why}"));
        for name in ["srl23", "shl23", "sra23"] {
            let body = text.split(&format!("{name}:")).nth(1).and_then(|rest| rest.split("rts").next()).expect(name);
            assert!(body.contains("swap"), "{level:?} {name}: no swap\n{body}");
            assert!(!body.contains("moveq\t#23"), "{level:?} {name}: the count is still in a register\n{body}");
        }
    }
}

#[test]
fn the_register_form_stays_at_os_and_on_the_68020() {
    for (cpu, level) in [("68000", Level::Os), ("68020", Level::O2), ("68020", Level::Os)] {
        let text = assembly(&ir(), cpu_of(cpu).unwrap(), level).unwrap_or_else(|why| panic!("{cpu} {level:?}: {why}"));
        let body = text.split("srl23:").nth(1).and_then(|rest| rest.split("rts").next()).expect("srl23");
        assert!(!body.contains("swap"), "{cpu} {level:?}\n{body}");
    }
}

#[test]
fn every_count_and_edge_value_computes_what_c_computes_when_run() {
    let (Some(root), Some(qemu)) = (support::oracle_root(), support::qemu()) else { return };
    let tool = |name: &str| {
        let mut command = Command::new(format!("{root}/usr/bin/m68k-linux-gnu-{name}"));
        command.env("LD_LIBRARY_PATH", format!("{root}/usr/lib/x86_64-linux-gnu"));
        command
    };
    for level in [Level::O1, Level::O2, Level::O3, Level::Os] {
        let mut text =
            assembly(&ir(), cpu_of("68000").unwrap(), level).unwrap_or_else(|why| panic!("{level:?}: {why}"));
        for name in ["srl", "shl", "sra"] {
            for n in 16..32 {
                text = text.replace(&format!("{name}{n}:"), &format!(".globl {name}{n}\n{name}{n}:"));
            }
        }
        let source = support::unique_path("wide-shift-s");
        let object = support::unique_path("wide-shift-o");
        let exe = support::unique_path("wide-shift-exe");
        std::fs::write(&source, format!(".text\n{text}")).unwrap();
        assert!(tool("as").arg("-m68000").arg("-o").arg(&object).arg(&source).status().unwrap().success());
        let main = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data_rt/wide_shift_main.c");
        let mut link = tool("gcc");
        link.args(["-msoft-float", "-m68000", "-O1", "-nostdlib", "-static", "-ffreestanding"]);
        link.arg(main).arg(&object).arg("-o").arg(&exe);
        let linked = link.output().unwrap();
        assert!(linked.status.success(), "{}", String::from_utf8_lossy(&linked.stderr));
        let code = support::run(&qemu, &["-cpu", "m68000"], &exe);
        for path in [source, object, exe] {
            std::fs::remove_file(path).ok();
        }
        assert_eq!(code, Some(0), "{level:?}: wrong result (1+k srl, 17+k shl, 33+k sra, count 16+k)");
    }
}

/// `x >> 23` in the entry block and `x >> 31` in a block that is not its
/// successor: the entry's `swap` was shared (`MachineCSE` did not know `SWAP`
/// is `isAsCheapAsAMove`), its range ran across the function, and the second
/// callee-saved register it forced cost every call 12 more clocks, the 1,632
/// of 3,000 `fix_si` vectors that return early included.
#[test]
fn a_swap_is_not_shared_into_a_block_that_does_not_follow_it() {
    let ir = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\n\
define i32 @f(i32 %x) {\nentry:\n  %v2 = lshr i32 %x, 23\n  %v3 = and i32 %v2, 255\n  %v4 = icmp samesign ult i32 %v3, 127\n  br i1 %v4, label %out, label %mid\n\
mid:\n  %v5 = icmp samesign ugt i32 %v3, 157\n  br i1 %v5, label %sat, label %body\n\
sat:\n  %v8 = icmp sgt i32 %x, -1\n  %v9 = select i1 %v8, i32 2147483647, i32 -2147483648\n  br label %out\n\
body:\n  %v11 = shl i32 %x, 8\n  %v12 = or i32 %v11, -2147483648\n  %v13 = sub nuw nsw i32 158, %v3\n  %v14 = lshr i32 %v12, %v13\n  %v15 = sub nsw i32 0, %v14\n  %v16 = icmp slt i32 %x, 0\n  %v17 = select i1 %v16, i32 %v15, i32 %v14\n  br label %out\n\
out:\n  %v19 = phi i32 [ %v17, %body ], [ %v9, %sat ], [ 0, %entry ]\n  ret i32 %v19\n}\n";
    let text = assembly(ir, cpu_of("68000").unwrap(), Level::O2).unwrap();
    let saved = text.lines().filter(|line| line.contains("-(%sp)")).count();
    assert_eq!(saved, 1, "one callee-saved register, not two\n{text}");
}
