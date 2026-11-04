use std::path::Path;
use std::process::Command;

/// `llrm-wasm-llc` of tests/ir/overflow_ops.ll, linked and run under node: each
/// function is called on edge operands and compared with BigInt arithmetic.
/// Every `llvm.*.with.overflow` stopped the compile with
/// `Unsupported { op: UAddO, .. "convert node to libcall" }` (so did any C
/// `__builtin_*_overflow`); the pin expands them in the legalizer.
#[test]
fn the_overflow_operations_compile_and_give_the_wrapped_value_and_the_overflow() {
    let dir = std::env::temp_dir().join(format!("llrm-wasm-overflow-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/overflow_ops.ll");
    for level in ["-O0", "-O1", "-O2"] {
        let made = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
            .args([level, "-filetype=obj", "-o"])
            .arg(dir.join("f.o"))
            .arg(&ir)
            .output()
            .expect("llrm-wasm-llc");
        assert!(made.status.success(), "{level}: {}", String::from_utf8_lossy(&made.stderr));
        let linked = Command::new("wasm-ld")
            .args(["--no-entry", "--export-all", "--allow-undefined"])
            .arg(dir.join("f.o"))
            .arg("-o")
            .arg(dir.join("f.wasm"))
            .output()
            .expect("wasm-ld");
        assert!(linked.status.success(), "{}", String::from_utf8_lossy(&linked.stderr));
        let script = "const m = new WebAssembly.Module(require('fs').readFileSync(process.env.MODULE));
             const e = new WebAssembly.Instance(m, {}).exports;
             const edge = [0n, 1n, 2n, 0x7fffffffn, 0x80000000n, 0xfffffffen, 0xffffffffn, 0x10000n, 0xffffn, 3n];
             const w = (x) => BigInt.asUintN(32, x), s = (x) => BigInt.asIntN(32, x);
             const pack = (v, o) => w(v) | (o ? 1n << 32n : 0n);
             const ops = {
               uaddo: (a, b) => pack(a + b, a + b > 0xffffffffn),
               usubo: (a, b) => pack(a - b, a < b),
               saddo: (a, b) => pack(a + b, s(a) + s(b) !== s(a + b)),
               ssubo: (a, b) => pack(a - b, s(a) - s(b) !== s(a - b)),
               umulo: (a, b) => pack(a * b, a * b > 0xffffffffn),
               smulo: (a, b) => pack(a * b, s(a) * s(b) !== s(a * b)),
             };
             let bad = 0;
             for (const [name, f] of Object.entries(ops))
               for (const a of edge) for (const b of edge) {
                 const got = BigInt.asUintN(64, e[name](Number(a), Number(b)));
                 if (got !== f(a, b)) { bad++; console.log(name, a, b, got, f(a, b)); }
               }
             const big = [0n, 1n, 0xffffffffn, 0x100000000n, 0xfffffffffffffffen, 0xffffffffffffffffn, 0x123456789n, 0x8000000000000000n];
             for (const a of big) for (const b of big) {
               const p = a * b, ovf = p > 0xffffffffffffffffn;
               const want = BigInt.asUintN(64, p) ^ (ovf ? 1n : 0n);
               const got = BigInt.asUintN(64, e.umulo64(a, b));
               if (got !== want) { bad++; console.log('umulo64', a, b, got, want); }
             }
             console.log(bad);";
        let ran = Command::new("node").args(["-e", script]).env("MODULE", dir.join("f.wasm")).output().expect("node");
        assert!(ran.status.success(), "{level}: {}", String::from_utf8_lossy(&ran.stderr));
        let out = String::from_utf8_lossy(&ran.stdout).into_owned();
        assert_eq!(out.lines().last(), Some("0"), "{level}: {out}");
    }
    std::fs::remove_dir_all(&dir).ok();
}
