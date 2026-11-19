use std::path::Path;
use std::process::Command;

fn function(
    level: &str,
    name: &str,
) -> String {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/call_order.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", level, "-o", "-"])
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success());
    let asm = String::from_utf8_lossy(&out.stdout).into_owned();
    let from = asm.find(&format!("\n{name}:")).expect("function");
    asm[from..].split("end_function").next().unwrap().to_owned()
}

/// RegStackify moved a `call` down past a store: `CALL` is in no instruction
/// table, so `query` saw an instruction that touches nothing. `h` stored
/// before it called `g`.
#[test]
fn a_call_stays_before_a_later_store() {
    for level in ["-O1", "-O2"] {
        let asm = function(level, "h");
        let (call, store) = (asm.find("call\tg"), asm.find("i32.store"));
        assert!(call < store, "{level}: {asm}");
    }
}

/// The same bug put the epilogue's `global.set __stack_pointer` before a call
/// that reads the frame: a callee with a frame of its own overwrote it.
#[test]
fn a_call_that_reads_the_frame_comes_before_the_stack_pointer_restore() {
    for level in ["-O1", "-O2"] {
        let asm = function(level, "frame");
        let (call, restore) = (asm.find("call\ttake"), asm.rfind("global.set\t__stack_pointer"));
        assert!(call < restore, "{level}: {asm}");
    }
}

/// `h` linked and run: `g` reads the word `h` stores to, so the call must come
/// first. The call moved after the store returned 2 where 5 + 1 is 6.
#[test]
fn a_call_that_reads_what_a_later_store_writes_sees_the_old_value() {
    let dir = std::env::temp_dir().join(format!("llrm-wasm-call-order-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/call_order.ll");
    let made = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-O1", "-filetype=obj", "-o"])
        .arg(dir.join("f.o"))
        .arg(&ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    let linked = Command::new("wasm-ld")
        .args(["--no-entry", "--export-all", "--allow-undefined", "--import-memory"])
        .arg(dir.join("f.o"))
        .arg("-o")
        .arg(dir.join("f.wasm"))
        .output()
        .expect("wasm-ld");
    assert!(linked.status.success(), "{}", String::from_utf8_lossy(&linked.stderr));
    let script = "const memory = new WebAssembly.Memory({ initial: 2 });
         const view = new DataView(memory.buffer);
         view.setUint32(1024, 5, true);
         const env = { memory, g: () => view.getUint32(1024, true), take: () => 0 };
         const m = new WebAssembly.Module(require('fs').readFileSync(process.env.MODULE));
         const instance = new WebAssembly.Instance(m, { env });
         console.log(instance.exports.h(1024));";
    let ran = Command::new("node").args(["-e", script]).env("MODULE", dir.join("f.wasm")).output().expect("node");
    std::fs::remove_dir_all(&dir).ok();
    assert!(ran.status.success(), "{}", String::from_utf8_lossy(&ran.stderr));
    assert_eq!(String::from_utf8_lossy(&ran.stdout).trim(), "6");
}
