//! Fourteen pointer arguments stored through, then a call, then loaded
//! through: more values live across the call than saved registers, so four
//! were stored to frame slots (a 40 B frame, a `movem` each way) although each
//! is an incoming stack argument that already sits in memory. The pin makes a
//! load of one again where it is read (`isLoadFromInvariantOrConstantMemory`).

use std::process::Command;

use llrm_m68k::driver::{Level, assembly, cpu_of};

mod support;

const POINTERS: &str = include_str!("fixtures/arg_pointers.ll");

#[test]
fn an_argument_the_allocator_cannot_keep_across_a_call_is_loaded_again_not_stored() {
    for cpu in ["68000", "68020", "68040"] {
        for level in [Level::O2, Level::Os] {
            let out = assembly(POINTERS, cpu_of(cpu).unwrap(), level).expect("compiles");
            assert!(!out.contains("lea\t(-"), "{cpu} {level:?}: a spill frame:\n{out}");
        }
    }
}

fn binutils(tool: &str) -> Option<Command> {
    let root = support::oracle_root()?;
    let mut command = Command::new(format!("{root}/usr/bin/m68k-linux-gnu-{tool}"));
    command.env("LD_LIBRARY_PATH", format!("{root}/usr/lib/x86_64-linux-gnu"));
    Some(command)
}

/// The values f returns and stores under qemu: the loads after the call read
/// the argument slots at the offsets the frame has there.
#[test]
fn the_reloaded_arguments_are_the_pointers_the_caller_passed_under_qemu() {
    let Some(qemu) = support::qemu() else { return };
    let (Some(mut assemble), Some(mut link)) = (binutils("as"), binutils("gcc")) else { return };
    let out = assembly(POINTERS, cpu_of("68020").unwrap(), Level::O2).expect("compiles");
    let main = "extern int f(int*,int*,int*,int*,int*,int*,int*,int*,int*,int*,int*,int*,int*,int*);
static long sys3(long n,long a,long b,long c){register long d0 __asm__(\"d0\")=n,d1 __asm__(\"d1\")=a,d2 __asm__(\"d2\")=b,d3 __asm__(\"d3\")=c;__asm__ volatile(\"trap #0\":\"+d\"(d0):\"d\"(d1),\"d\"(d2),\"d\"(d3):\"memory\");return d0;}
void g(void) {}
int main(void) {
  int a[14];
  for (int i = 0; i < 14; i++) a[i] = -1;
  int r = f(a, a+1, a+2, a+3, a+4, a+5, a+6, a+7, a+8, a+9, a+10, a+11, a+12, a+13);
  if (r != 91) return 1;
  for (int i = 0; i < 14; i++) if (a[i] != i) return 2 + i;
  return 0;
}
void _start(void){ int r = main(); sys3(1, r > 255 ? 255 : r, 0, 0); for(;;); }
";
    let source = support::unique_path("argremat-s");
    let object = support::unique_path("argremat-o");
    let c = support::unique_path("argremat-c");
    let exe = support::unique_path("argremat-exe");
    std::fs::write(&source, format!("{out}\n")).unwrap();
    std::fs::write(&c, main).unwrap();
    assert!(assemble.arg("-m68020").arg("-o").arg(&object).arg(&source).status().unwrap().success(), "{out}");
    let linked = link
        .args(["-msoft-float", "-m68020", "-O1", "-nostdlib", "-static", "-ffreestanding", "-x", "c"])
        .arg(&c)
        .args(["-x", "none"])
        .arg(&object)
        .arg("-o")
        .arg(&exe)
        .status()
        .unwrap();
    assert!(linked.success(), "{out}");
    let code = support::run(&qemu, &["-cpu", "m68020"], &exe);
    for path in [source, object, c, exe] {
        std::fs::remove_file(path).ok();
    }
    assert_eq!(code, Some(0), "f returned a wrong value or stored through a wrong pointer (code {code:?}):\n{out}");
}
