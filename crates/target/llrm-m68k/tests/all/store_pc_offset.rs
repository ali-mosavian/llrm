//! A store to a global plus a constant offset failed with
//! `CannotSelect(Store)`: the address matched as PC-relative, which a store
//! cannot use, and the matcher never retried with the symbol in a register.
//! The pin's `llc` fails the same way; the symbol is now loaded with `lea`.

use llrm_m68k::compile::{Options, text_with};
use llrm_m68k::dagisel::hooks::Cpu;

const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";

fn text(
    cpu: Cpu,
    body: &str,
) -> String {
    let module = llrm_mir::parse::module(&format!(
        "target datalayout = \"{DATALAYOUT}\"\n@shared = global [10 x i32] zeroinitializer, align 4\n{body}"
    ))
    .expect("parses");
    text_with(&module, "f", cpu, Options::default()).expect("compiles")
}

const STORE: &str = "
define void @f() {
  store i32 2, ptr getelementptr inbounds (i8, ptr @shared, i32 4), align 4
  ret void
}";

const STORE_INDEXED: &str = "
define void @f(i32 %i) {
  %p = getelementptr inbounds i32, ptr @shared, i32 %i
  store i32 7, ptr %p, align 4
  ret void
}";

#[test]
fn a_store_to_a_global_plus_offset_compiles_on_every_cpu() {
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let out = text(cpu, STORE);
        assert!(out.contains("(shared,%pc)") && out.contains("(4,%a"), "not lea + offset store on {cpu:?}:\n{out}");
    }
}

#[test]
fn a_store_to_a_global_never_writes_through_the_pc() {
    for cpu in [Cpu::M68000, Cpu::M68020] {
        for body in [STORE, STORE_INDEXED] {
            let out = text(cpu, body);
            let bad = out
                .lines()
                .find(
                    |line| {
                        let line = line.trim_start();
                        line.starts_with("move") && line.rsplit(',').next().is_some_and(|dst| dst.contains("%pc"))
                    },
                );
            assert!(bad.is_none(), "a store with a PC-relative destination on {cpu:?}: {bad:?}\n{out}");
        }
    }
}

#[test]
fn a_load_of_a_global_plus_offset_stays_one_pc_relative_instruction() {
    let body = "
define i32 @f() {
  %v = load i32, ptr getelementptr inbounds (i8, ptr @shared, i32 4), align 4
  ret i32 %v
}";
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let out = text(cpu, body);
        assert!(out.contains("(shared+4,%pc)") && !out.contains("lea"), "the load changed on {cpu:?}:\n{out}");
    }
}
