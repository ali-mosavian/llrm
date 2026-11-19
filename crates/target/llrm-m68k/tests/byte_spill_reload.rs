//! A byte value that was spilled, reloaded and stored straight back lost its
//! reload: the spill instructions named the register `Virt` where every other
//! instruction named the byte by its lane, so `drop_self_copies` saw no other
//! reader and dropped the load. The copy read after it took whatever the
//! register held: `add.b #127` ran on the result of `add.b #53` and `test`
//! returned 63 where x86 returns 24 (seeds_raw: 50 of 2400 cells wrong).

use llrm_m68k::driver::{Level, assembly, cpu_of};

mod support;

const IR: &str = include_str!("fixtures/byte_spill_reload.ll");

/// `k()` is 14 on x86; `_start` exits with `k() + 10`.
const EXIT: i32 = 24;

const START: &str = "const unsigned char zero = 0;
const unsigned char buf[16] = {1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16};
extern unsigned char k(void);
static long sys3(long n,long a,long b,long c){register long d0 __asm__(\"d0\")=n,d1 __asm__(\"d1\")=a,d2 __asm__(\"d2\")=b,d3 __asm__(\"d3\")=c;__asm__ volatile(\"trap #0\":\"+d\"(d0):\"d\"(d1),\"d\"(d2),\"d\"(d3):\"memory\");return d0;}
void _start(void){ int r = k(); sys3(1, r + 10, 0, 0); for(;;); }
";

fn binutils(tool: &str) -> Option<std::process::Command> {
    let root = support::oracle_root()?;
    let mut command = std::process::Command::new(format!("{root}/usr/bin/m68k-linux-gnu-{tool}"));
    command.env("LD_LIBRARY_PATH", format!("{root}/usr/lib/x86_64-linux-gnu"));
    Some(command)
}

#[test]
fn a_reloaded_byte_is_read_after_its_reload_under_qemu() {
    let Some(qemu) = support::qemu() else { return };
    for level in [Level::O0, Level::O1, Level::O2, Level::Os] {
        let (Some(mut assemble), Some(mut link)) = (binutils("as"), binutils("gcc")) else { return };
        let out = assembly(IR, cpu_of("68000").unwrap(), level).expect("compiles");
        let source = support::unique_path("byte-spill-s");
        let object = support::unique_path("byte-spill-o");
        let c = support::unique_path("byte-spill-c");
        let exe = support::unique_path("byte-spill-exe");
        std::fs::write(&source, format!("{out}\n")).unwrap();
        std::fs::write(&c, START).unwrap();
        assert!(assemble.arg("-m68000").arg("-o").arg(&object).arg(&source).status().unwrap().success(), "{out}");
        let linked = link
            .args(["-msoft-float", "-m68000", "-O1", "-nostdlib", "-static", "-ffreestanding", "-x", "c"])
            .arg(&c)
            .args(["-x", "none"])
            .arg(&object)
            .arg("-o")
            .arg(&exe)
            .status()
            .unwrap();
        assert!(linked.success(), "{out}");
        let code = support::run(&qemu, &["-cpu", "m68000"], &exe);
        assert_eq!(code, Some(EXIT), "{level:?}: exit {code:?}, x86 gives {EXIT}:\n{out}");
    }
}
