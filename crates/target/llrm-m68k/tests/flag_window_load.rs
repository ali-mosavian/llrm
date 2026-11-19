//! An argument load made again between the instruction that set the codes and
//! the branch that reads them (`move.l (64,%sp), %d4` before `bpl`) changed
//! what the branch saw. soft-float `__adddf3` was one unit in the last place
//! off for 7 of 400 operand pairs (recfloat, nbody, object_extent_loop wrong at
//! 68000 -O2).

use std::path::{Path, PathBuf};
use std::process::Command;

use llrm_m68k::compile::{Options, text_with};
use llrm_m68k::dagisel::hooks::Cpu;

mod support;

fn binutils(tool: &str) -> Option<Command> {
    let root = support::oracle_root()?;
    let mut command = Command::new(format!("{root}/usr/bin/m68k-linux-gnu-{tool}"));
    command.env("LD_LIBRARY_PATH", format!("{root}/usr/lib/x86_64-linux-gnu"));
    Some(command)
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests").join(name)
}

/// Every function label global, which is what `llrm-cc -S` leaves to the
/// caller.
fn globalised(
    text: &str,
    names: &[&str],
) -> String {
    let mut out = String::from(".text\n");
    for line in text.lines() {
        let label = line
            .strip_suffix(':')
            .filter(|name| !name.starts_with(".L") && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'));
        if let Some(name) = label.filter(|name| names.contains(&name)) {
            out.push_str(&format!(".globl {name}\n"));
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// `__adddf3` and `__subdf3` of runtime/compiler-rt/lib/fp/f64 as the mid-end
/// leaves them at -O2 (`LLRM_MIR_STAGES`, last stage), over 400 operand pairs
/// folded to a byte the host computes as 96; the C `main` checks it.
#[test]
fn soft_float_sums_match_the_host() {
    if binutils("as").is_none() {
        return;
    }
    let Some(qemu) = support::qemu() else { return };
    for (cpu, flag) in [(Cpu::M68000, "68000"), (Cpu::M68020, "68020")] {
        let mut objects = Vec::new();
        for (file, names) in
            [("soft_add.ll", &["__adddf3", "add_bits_64", "__subdf3"][..]), ("soft_pack.ll", &["sf64_pack"][..])]
        {
            let module =
                llrm_mir::parse::module(&std::fs::read_to_string(fixture(&format!("fixtures/{file}"))).unwrap())
                    .expect("parses");
            let mut text = String::new();
            for name in names {
                text +=
                    &text_with(&module, name, cpu, Options { mid_end: false, ..Options::default() }).expect("compiles");
            }
            let source = support::unique_path("flag-window-s");
            let object = support::unique_path("flag-window-o");
            std::fs::write(&source, globalised(&text, &["__adddf3", "__subdf3", "sf64_pack"])).unwrap();
            assert!(
                binutils("as")
                    .unwrap()
                    .arg(format!("-m{flag}"))
                    .arg("-o")
                    .arg(&object)
                    .arg(&source)
                    .status()
                    .unwrap()
                    .success()
            );
            std::fs::remove_file(source).ok();
            objects.push(object);
        }
        let exe = support::unique_path("flag-window-exe");
        let linked = binutils("gcc")
            .unwrap()
            .args(["-msoft-float", &format!("-m{flag}"), "-O1", "-nostdlib", "-static", "-ffreestanding"])
            .arg(fixture("data_rt/soft_add_main.c"))
            .args(&objects)
            .arg("-o")
            .arg(&exe)
            .status()
            .unwrap();
        assert!(linked.success());
        let code = support::run(&qemu, &["-cpu", &format!("m{flag}")], &exe);
        for path in objects.into_iter().chain([exe]) {
            std::fs::remove_file(path).ok();
        }
        assert_eq!(code, Some(0), "{flag}: a sum or difference differs from the host's");
    }
}
