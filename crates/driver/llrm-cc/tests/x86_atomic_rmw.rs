//! x86-m32 (`i486`) and x86-m64: `atomicrmw add/sub/or` and `cmpxchg` are `lock
//! xadd`, `lock add` and `lock cmpxchg` (`X86TargetLowering::lowerAtomicArith`,
//! `shouldExpandAtomicRMWInIR`). x86 selected only `xchg`: the same source was
//! refused ("`rmwadd monotonic` is not lowered for this target yet"; m64
//! "`cmpxchg monotonic`") and QuickrBasic's `pmou.c` did not compile on either.
//! Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

fn listing(
    target: &str,
    level: &str,
) -> String {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("x86-rmw-{target}-{level}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let asm = dir.join("a.s");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", target)
        .env("LLRM_LLC", llc)
        .args(if target == "x86_32-linux" { vec!["-march=i486"] } else { vec![] })
        .args([level, "-w", "-S", "-o"])
        .arg(&asm)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/x86_atomic_rmw.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{target} {level}: {}", String::from_utf8_lossy(&run.stderr));
    std::fs::read_to_string(&asm).unwrap()
}

fn function<'a>(
    listing: &'a str,
    name: &str,
) -> Vec<&'a str> {
    let start = listing.lines().position(|line| line.trim() == format!("{name} proc near")).expect("the function");
    listing.lines().skip(start).take_while(|line| line.trim() != format!("{name} endp")).map(str::trim).collect()
}

#[test]
fn x86_add_sub_or_and_cmpxchg_are_the_locked_instructions() {
    for target in ["x86_32-linux", "x86_64-linux"] {
        for level in ["-O1", "-Os"] {
            if target == "x86_64-linux" {
                // a 64-bit exchange: the form has one destination, which the
                // encoder refused (pmou.c)
                let text = listing(target, level);
                let body = function(&text, "swap64");
                assert!(body.iter().any(|line| line.starts_with("xchg ")), "{level}: {}", body.join("\n"));
            }
            let text = listing(target, level);
            let has = |name: &str, what: &str| {
                let body = function(&text, name);
                assert!(
                    body.iter().any(|line| line.starts_with(what)),
                    "{target} {level} {name}: {what}\n{}",
                    body.join("\n")
                );
            };
            // the result is read: `lock xadd`; unread: `lock add`/`lock or` of
            // the cell itself
            has("fetch_add", "lock xadd ");
            has("fetch_sub", "lock xadd ");
            has("add_only", "lock add ");
            has("or_only", "lock or ");
            has("cas", "lock cmpxchg ");
            // `xadd` and `cmpxchg` tie their register to the result: `lock xadd
            // w, edi` with the old value returned in eax was
            // selected on x86_64 (the tie was never made)
            has("fetch_add", "lock xadd dword ptr w, eax");
            has("fetch_sub", "lock xadd dword ptr w, eax");
            let cas = function(&text, "cas");
            let at = cas.iter().position(|line| line.starts_with("lock cmpxchg ")).unwrap();
            assert!(cas[..at].iter().any(|line| line.starts_with("mov eax, ")), "{target} {level}: {}", cas.join("\n"));
        }
    }
}

/// `-S` prints `xchg` without encoding it; `-c` hit the encoder, which refused
/// a 64-bit exchange (`swap64`, `take`: an indexed cell, pmou_press) with
/// "llrm-llc: swap64: Semantics(op=EXCHANGE ...".
#[test]
fn a_64_bit_exchange_encodes_to_an_object() {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("x86-xchg64-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for level in ["-O1", "-Os"] {
        let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
            .env("LLRM_CC_TARGET", "x86_64-linux")
            .env("LLRM_LLC", &llc)
            .args([level, "-w", "-c", "-o"])
            .arg(dir.join("a.o"))
            .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/x86_atomic_rmw.c"))
            .output()
            .unwrap();
        assert!(run.status.success(), "{level}: {}", String::from_utf8_lossy(&run.stderr));
    }
}
