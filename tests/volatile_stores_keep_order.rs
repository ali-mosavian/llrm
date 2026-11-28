//! Guard for the m68k release-store fix (chain combine and `isSimple`): two
//! volatile stores keep their program order on x86_16, the data before the
//! flag.

use std::process::Command;

#[test]
fn a_volatile_data_store_stays_before_the_volatile_flag_store_x86_16() {
    let scratch = tempfile::tempdir().unwrap();
    let dir = scratch.path();
    let source = dir.join("publish.ll");
    std::fs::write(
        &source,
        "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3\"\ntarget triple = \"i386-unknown-unknown-code16\"\n\
         @data = global [4 x i16] zeroinitializer, align 2\n@flag = global i16 0, align 2\n\
         define void @publish(i16 %x) {\n  store volatile i16 %x, ptr getelementptr inbounds (i8, ptr @data, i16 2), align 2\n  \
         store volatile i16 1, ptr @flag, align 2\n  ret void\n}\n",
    )
    .unwrap();
    for level in ["-O0", "-O1", "-O2", "-O3", "-Os"] {
        let out = dir.join("publish.s");
        let done = Command::new(env!("CARGO_BIN_EXE_llrm-llc"))
            .args(["-m16", level, "-S", "-o", out.to_str().unwrap(), source.to_str().unwrap()])
            .output()
            .expect("the driver runs");
        assert!(done.status.success(), "{level}: {}", String::from_utf8_lossy(&done.stderr));
        let asm = std::fs::read_to_string(&out).unwrap();
        let body: Vec<&str> =
            asm.lines().skip_while(|line| !(line.contains("publish") && line.contains(" proc"))).skip(1).collect();
        let at = |name: &str| body.iter().position(|line| line.contains(name));
        assert!(at("data").is_some() && at("data") < at("flag"), "{level}: the flag is stored first:\n{asm}");
    }
}
