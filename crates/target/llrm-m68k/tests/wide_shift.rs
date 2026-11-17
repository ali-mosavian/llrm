//! An `i32` shift by a constant of 16 to 31 on the 68000 was `moveq #n, %dm;
//! lsr.l %dm, %dn`: 8 + 2n clocks and a register (`fix_usi`'s exponent,
//! `(x >> 23) & 255`, 58 of the 719,592 clocks per 3,000 vectors; the swap form
//! below took it to 684,462, under gcc's 707,210). Now the swap of the halves
//! and a shift of 15 or fewer, at the speed levels; `-Os` and the 68020 keep
//! the register form. Executed against C over every count and the edge values.

use std::path::Path;
use std::process::Command;

use llrm_m68k::driver::{Level, assembly, cpu_of};

mod support;

fn ir() -> String {
    let mut text = String::from(
        "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\n",
    );
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
        let mut text = assembly(&ir(), cpu_of("68000").unwrap(), level).unwrap_or_else(|why| panic!("{level:?}: {why}"));
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
