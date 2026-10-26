//! `llrm-c`'s command line served by `llrm-cc` over the pinned clang: the
//! helper the tests that spawned `CARGO_BIN_EXE_llrm-c` use instead.
//! Needs the pinned clang (`LLRM_PIN_CLANG`, `LLRM_CLANG_M16` at m16) and the
//! Open Watcom headers (`LLRM_OW`) in the environment.

use std::process::Command;

/// `llrm-cc` for the Watcom target `args` name (`-m32` or, by default as
/// llrm-c, `-m16`). `args` are the llrm-c arguments; add more with `.args`.
pub fn llrm_c(args: &[&str]) -> Command {
    let target = if args.contains(&"-m32") { "x86_32-wat" } else { "x86_16-wat" };
    let mut command = Command::new(env!("CARGO_BIN_EXE_llrm-cc"));
    command.env("LLRM_CC_TARGET", target).args(args.iter().filter(|one| !matches!(**one, "-m16" | "-m32")));
    command
}
