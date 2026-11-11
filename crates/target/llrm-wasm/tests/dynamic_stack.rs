//! `DYNAMIC_STACKALLOC`, `STACKSAVE` and `STACKRESTORE` on wasm32 were
//! `CannotSelect`: the shared expansion needs the target to name its stack
//! pointer, and an alloca or a stacksave stopped compilation.

use llrm_wasm::Width;
use llrm_wasm::dag::Options;
use llrm_wasm::driver::{Level, assembly};

/// The instructions of `@f` between the entry label and the return, as the
/// pin's `llc -O2` prints them (tabs and trailing blanks dropped).
fn body(text: &str) -> Vec<String> {
    let out = assembly(text, Width::W32, Options { level: Level::O2, ..Options::default() })
        .unwrap_or_else(|why| panic!("refused: {why}"));
    let (_, rest) = out.split_once("# %bb.0:\n").expect("entry block");
    let (code, _) = rest.split_once("# fallthrough-return").expect("return");
    code.lines().map(|line| line.split_whitespace().collect::<Vec<_>>().join(" ")).filter(|line| !line.is_empty()).collect()
}

/// A variable-sized alloca stopped compilation (`CannotSelect`). The pin
/// lowers the stack pointer by the rounded size, keeps the incoming one in a
/// frame pointer and writes it back after the call and at the return.
#[test]
fn a_variable_alloca_matches_the_pin() {
    let text = "declare void @take(ptr)\ndefine void @f(i32 %n) {\n  %a = alloca i8, i32 %n\n  call void @take(ptr %a)\n  ret void\n}\n";
    assert_eq!(
        body(text),
        [
        "global.get __stack_pointer",
        "local.tee 1",
        "local.get 0",
        "i32.const 15",
        "i32.add",
        "i32.const -16",
        "i32.and",
        "i32.sub",
        "local.tee 0",
        "global.set __stack_pointer",
        "local.get 0",
        "call take",
        "local.get 1",
        "global.set __stack_pointer",
        ]
    );
}

/// `llvm.stacksave` / `llvm.stackrestore` were `CannotSelect`: they copy the
/// stack pointer register, as the pin's `STACKSAVE` / `STACKRESTORE` do.
#[test]
fn stacksave_and_stackrestore_match_the_pin() {
    let text = "declare ptr @llvm.stacksave.p0()\ndeclare void @llvm.stackrestore.p0(ptr)\ndeclare void @take(ptr)\n\
                define void @f(i32 %n) {\n  %s = call ptr @llvm.stacksave.p0()\n  %a = alloca i8, i32 %n\n  call void @take(ptr %a)\n  call void @llvm.stackrestore.p0(ptr %s)\n  ret void\n}\n";
    assert_eq!(
        body(text),
        [
        "global.get __stack_pointer",
        "local.tee 1",
        "local.get 0",
        "i32.const 15",
        "i32.add",
        "i32.const -16",
        "i32.and",
        "i32.sub",
        "local.tee 0",
        "global.set __stack_pointer",
        "local.get 0",
        "call take",
        "local.get 1",
        "drop",
        "local.get 1",
        "global.set __stack_pointer",
        ]
    );
}

/// An object aligned past the stack's 16 was refused by name (`needs stack
/// realignment`). The pin keeps the incoming stack pointer as a base pointer
/// and masks the new one.
#[test]
fn an_over_aligned_object_matches_the_pin() {
    let text = "declare void @take(ptr)\ndefine void @f() {\n  %a = alloca i32, align 32\n  call void @take(ptr %a)\n  ret void\n}\n";
    assert_eq!(
        body(text),
        [
        "global.get __stack_pointer",
        "local.tee 0",
        "i32.const 32",
        "i32.sub",
        "i32.const -32",
        "i32.and",
        "local.tee 1",
        "global.set __stack_pointer",
        "local.get 1",
        "call take",
        "local.get 0",
        "global.set __stack_pointer",
        ]
    );
}
