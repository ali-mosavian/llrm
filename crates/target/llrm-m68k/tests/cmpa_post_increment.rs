//! countgt -Os at 68020 compared its bound with `cmp.l (%a1)+, %d2`: the
//! post-increment compare had no `cmpa` sibling, so the bound stayed in a data
//! register, the `slt` byte needed a second saved one, and the function
//! pushed `d3` and `d2` where gcc pushes `d2` (+4 B). Every compare form has an
//! address-destination twin now.

use llrm_m68k::compile::{Options, text_with};
use llrm_m68k::dagisel::hooks::Cpu;

const COUNTGT: &str = include_str!("../bench/kernels/countgt.ll");

fn size_build(cpu: Cpu) -> String {
    let module = llrm_mir::parse::module(COUNTGT).expect("parses");
    text_with(&module, "k", cpu, Options { size: true, ..Options::default() }).expect("compiles")
}

#[test]
fn a_bound_compared_through_post_increment_lives_in_an_address_register() {
    for cpu in [Cpu::M68020, Cpu::M68040] {
        let out = size_build(cpu);
        assert!(out.contains("cmpa.l\t(%a1)+, %a0") || out.contains("cmpa.l\t(%a0)+, %a1"), "{cpu:?}:\n{out}");
        let pushes = out.lines().filter(|line| line.contains("-(%sp)")).count();
        assert_eq!(pushes, 1, "{cpu:?}: one saved data register, as gcc:\n{out}");
    }
}

mod support;

/// The new row must assemble as gas does (our encoder is what writes the
/// object) and the loop must still count.
#[test]
fn the_cmpa_through_post_increment_encodes_as_gas_does_and_counts_when_run() {
    let (Some(root), Some(qemu)) = (support::oracle_root(), support::qemu()) else { return };
    let tool = |name: &str| {
        let mut command = std::process::Command::new(format!("{root}/usr/bin/m68k-linux-gnu-{name}"));
        command.env("LD_LIBRARY_PATH", format!("{root}/usr/lib/x86_64-linux-gnu"));
        command
    };
    for (cpu, flag) in [(Cpu::M68020, "68020"), (Cpu::M68040, "68040")] {
        let module = llrm_mir::parse::module(COUNTGT).expect("parses");
        let options = Options { size: true, ..Options::default() };
        let text = text_with(&module, "k", cpu, options).expect("compiles");
        let ours = llrm_m68k::object::assemble(&llrm_m68k::compile::items_with(&module, "k", cpu, options).unwrap(), 0)
            .expect("assembles")
            .code;
        let source = support::unique_path("cmpa-s");
        let object = support::unique_path("cmpa-o");
        let flat = support::unique_path("cmpa-bin");
        let exe = support::unique_path("cmpa-exe");
        // an unsized `blt` is a word branch to gas; ours is short where the
        // displacement fits, so size them alike
        let sized: String = text
            .lines()
            .map(|line| match line.strip_prefix('\t').and_then(|rest| rest.split_once('\t')) {
                Some((mnemonic, target)) if mnemonic.starts_with('b') && target.starts_with(".L") => {
                    format!("\t{mnemonic}.s\t{target}\n")
                }
                _ => format!("{line}\n"),
            })
            .collect();
        std::fs::write(&source, format!(".text\n.globl k\n{sized}")).unwrap();
        assert!(tool("as").arg(format!("-m{flag}")).arg("-o").arg(&object).arg(&source).status().unwrap().success());
        assert!(
            tool("objcopy").args(["-O", "binary", "-j", ".text"]).arg(&object).arg(&flat).status().unwrap().success()
        );
        let gas = std::fs::read(&flat).unwrap();
        let hex = |bytes: &[u8]| bytes.iter().map(|byte| format!("{byte:02x}")).collect::<String>();
        assert_eq!(gas, ours, "{cpu:?}: our encoder and gas differ\n gas  {}\n ours {}\n{text}", hex(&gas), hex(&ours));
        let main = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data_rt/cmpa_post_increment_main.c");
        let mut link = tool("gcc");
        link.args(["-msoft-float", &format!("-m{flag}"), "-O1", "-nostdlib", "-static", "-ffreestanding"]);
        link.arg(main).arg(&object).arg("-o").arg(&exe);
        assert!(link.status().unwrap().success());
        let code = support::run(&qemu, &["-cpu", &format!("m{flag}")], &exe);
        for path in [source, object, flat, exe] {
            std::fs::remove_file(path).ok();
        }
        assert_eq!(code, Some(0), "{cpu:?}: k miscounted");
    }
}
