use std::path::Path;
use std::process::Command;

const IR: &str = "tests/ir/i128_abi.ll";

fn compile(
    level: &str,
    args: &[&str],
    out: &Path,
) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args([level])
        .args(args)
        .arg("-o")
        .arg(out)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join(IR))
        .output()
        .expect("llrm-wasm-llc")
}

/// An `i128` parameter was refused (`a wasm signature with Int(128)`), a result
/// (`a wasm return of more than one part`) and a variable shift (`expand shift
/// to a library call`). The pin passes each `i128` as two `i64`, returns it
/// through a leading pointer, and calls `__ashlti3 (sret, lo, hi, i32 amount)`.
#[test]
fn an_i128_is_two_i64_arguments_a_result_through_a_pointer_and_a_shift_a_library_call() {
    let dir = std::env::temp_dir().join(format!("llrm-wasm-i128-text-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let made = compile("-O2", &["-S"], &dir.join("f.s"));
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    let asm = std::fs::read_to_string(dir.join("f.s")).unwrap();
    std::fs::remove_dir_all(&dir).ok();
    for want in [
        ".functype\tadd (i32, i64, i64, i64, i64) -> ()",
        ".functype\tcmp (i64, i64, i64, i64) -> (i32)",
        ".functype\tshl (i32, i64, i64, i64, i64) -> ()",
        "call\t__ashlti3",
        "i32.wrap_i64",
    ] {
        assert!(asm.contains(want), "no `{want}` in:\n{asm}");
    }
}

/// The same module linked and run under node, with `__ashlti3` supplied by the
/// page: the sums, the compare and the shift are the BigInt ones.
#[test]
fn an_i128_add_compare_and_shift_give_the_bigint_results_under_node() {
    let dir = std::env::temp_dir().join(format!("llrm-wasm-i128-run-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for level in ["-O0", "-O1", "-O2"] {
        let made = compile(level, &["-filetype=obj"], &dir.join("f.o"));
        assert!(made.status.success(), "{level}: {}", String::from_utf8_lossy(&made.stderr));
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
             const read = (p) => (view.getBigUint64(p + 8, true) << 64n) | view.getBigUint64(p, true);
             const write = (p, v) => { view.setBigUint64(p, BigInt.asUintN(64, v), true); view.setBigUint64(p + 8, BigInt.asUintN(64, v >> 64n), true); };
             const lo = (v) => BigInt.asUintN(64, v), hi = (v) => BigInt.asUintN(64, v >> 64n);
             const env = { memory, __ashlti3: (p, l, h, n) => write(p, BigInt.asUintN(128, ((BigInt.asUintN(64, h) << 64n) | BigInt.asUintN(64, l)) << BigInt(n))) };
             const m = new WebAssembly.Module(require('fs').readFileSync(process.env.MODULE));
             const e = new WebAssembly.Instance(m, { env }).exports;
             const edge = [0n, 1n, 0xffffffffffffffffn, 0x10000000000000000n, (1n << 127n), (1n << 128n) - 1n, 0x123456789abcdef0123456789abcdefn, 3n << 64n];
             let bad = 0;
             for (const a of edge) for (const b of edge) {
               e.add(1024, lo(a), hi(a), lo(b), hi(b));
               if (read(1024) !== BigInt.asUintN(128, a + b)) { bad++; console.log('add', a, b); }
               if (e.cmp(lo(a), hi(a), lo(b), hi(b)) !== (a < b ? 1 : 0)) { bad++; console.log('cmp', a, b); }
             }
             for (const a of edge) for (const n of [0n, 1n, 63n, 64n, 65n, 127n]) {
               e.shl(1024, lo(a), hi(a), lo(n), hi(n));
               if (read(1024) !== BigInt.asUintN(128, a << n)) { bad++; console.log('shl', a, n); }
             }
             console.log(bad);";
        let ran = Command::new("node").args(["-e", script]).env("MODULE", dir.join("f.wasm")).output().expect("node");
        assert!(ran.status.success(), "{level}: {}", String::from_utf8_lossy(&ran.stderr));
        let out = String::from_utf8_lossy(&ran.stdout).into_owned();
        assert_eq!(out.lines().last(), Some("0"), "{level}: {out}");
    }
    std::fs::remove_dir_all(&dir).ok();
}
