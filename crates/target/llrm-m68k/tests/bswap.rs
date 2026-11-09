//! `bswap` on a 32-bit value: the pin's `rol.w #8, dn; swap dn; rol.w #8, dn`
//! (`M68kInstrShiftRotate.td`), which the emitter refused (`SUBREG_TO_REG of
//! anything but a cleared low word`); a combine's shift by a constant made
//! after legalization had the shared `i8` amount and no pattern
//! (`CannotSelect(Shl)`, `x_popcount` at O1 to Os). Executed against C on
//! qemu-m68k.

use std::path::Path;
use std::process::Command;

use llrm_m68k::driver::{Level, assembly, cpu_of};

mod support;

const IR: &str = include_str!("data_rt/bswap_forms.ll");

#[test]
fn bswap_i32_is_rotate_swap_rotate_on_one_register() {
    for cpu in ["68000", "68020"] {
        for level in [Level::O1, Level::O2, Level::Os] {
            let text = assembly(IR, cpu_of(cpu).unwrap(), level).unwrap_or_else(|why| panic!("{cpu} {level:?}: {why}"));
            let body = text.split("bs:").nth(1).and_then(|rest| rest.split("rts").next()).expect("bs");
            let lines: Vec<&str> = body.lines().map(str::trim).filter(|line| !line.is_empty()).collect();
            let at = lines.iter().position(|line| line.starts_with("rol.w\t#8,")).expect("a rol.w #8");
            let register = lines[at].rsplit(' ').next().unwrap();
            assert_eq!(
                &lines[at..at + 3],
                [format!("rol.w\t#8, {register}"), format!("swap\t{register}"), format!("rol.w\t#8, {register}")],
                "{cpu} {level:?}\n{text}"
            );
        }
    }
}

#[test]
fn every_form_computes_what_c_computes_when_run() {
    let (Some(root), Some(qemu)) = (support::oracle_root(), support::qemu()) else { return };
    let tool = |name: &str| {
        let mut command = Command::new(format!("{root}/usr/bin/m68k-linux-gnu-{name}"));
        command.env("LD_LIBRARY_PATH", format!("{root}/usr/lib/x86_64-linux-gnu"));
        command
    };
    for cpu in ["68000", "68020"] {
        for level in [Level::O1, Level::O2, Level::Os] {
            let mut text =
                assembly(IR, cpu_of(cpu).unwrap(), level).unwrap_or_else(|why| panic!("{cpu} {level:?}: {why}"));
            for name in ["bs", "bs_low", "bs_high", "rev", "rev_loop"] {
                text = text.replace(&format!("{name}:"), &format!(".globl {name}\n{name}:"));
            }
            let source = support::unique_path("bswap-s");
            let object = support::unique_path("bswap-o");
            let exe = support::unique_path("bswap-exe");
            std::fs::write(&source, format!(".text\n{text}")).unwrap();
            assert!(tool("as").arg(format!("-m{cpu}")).arg("-o").arg(&object).arg(&source).status().unwrap().success());
            let main = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data_rt/bswap_forms_main.c");
            let mut link = tool("gcc");
            link.args(["-msoft-float", &format!("-m{cpu}"), "-O1", "-nostdlib", "-static", "-ffreestanding"]);
            link.arg(main).arg(&object).arg("-o").arg(&exe);
            assert!(link.status().unwrap().success());
            let code = support::run(&qemu, &["-cpu", &format!("m{cpu}")], &exe);
            for path in [source, object, exe] {
                std::fs::remove_file(path).ok();
            }
            assert_eq!(
                code,
                Some(0),
                "{cpu} {level:?}: wrong form (1 bs, 2 low byte, 4 high byte, 8 bit reverse, 16 loop)"
            );
        }
    }
}
