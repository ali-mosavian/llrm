//! `WasmMachine::size_costs` against the bytes `wasm-objdump` reads back from
//! the object of one instruction of each class. The default table (1 per op)
//! priced a load at 1 byte where it is 3, a call at 1 where it is 6.

use llrm_mir::target::Machine;
use llrm_wasm::Width;
use llrm_wasm::dag::Options;
use llrm_wasm::driver::object;
use llrm_wasm::machine::WasmMachine;

const MODULE: &str = "
declare i32 @ext(i32)
define i32 @add(i32 %a, i32 %b) {\n  %r = add i32 %a, %b\n  ret i32 %r\n}
define i32 @mul(i32 %a, i32 %b) {\n  %r = mul i32 %a, %b\n  ret i32 %r\n}
define i32 @div(i32 %a, i32 %b) {\n  %r = sdiv i32 %a, %b\n  ret i32 %r\n}
define i32 @shl(i32 %a, i32 %b) {\n  %r = shl i32 %a, %b\n  ret i32 %r\n}
define i32 @load(ptr %p) {\n  %q = getelementptr i32, ptr %p, i32 2\n  %r = load i32, ptr %q\n  ret i32 %r\n}
define void @store(ptr %p, i32 %v) {\n  %q = getelementptr i32, ptr %p, i32 2\n  store i32 %v, ptr %q\n  ret void\n}
define i32 @call(i32 %a) {\n  %r = call i32 @ext(i32 %a)\n  ret i32 %r\n}
define i32 @branch(i32 %a, i32 %b) {
  %c = icmp slt i32 %a, %b
  br i1 %c, label %t, label %f
t:
  ret i32 %a
f:
  %r = call i32 @ext(i32 %b)
  ret i32 %r
}
define double @fadd(double %a, double %b) {\n  %r = fadd double %a, %b\n  ret double %r\n}
define double @fmul(double %a, double %b) {\n  %r = fmul double %a, %b\n  ret double %r\n}
define double @fdiv(double %a, double %b) {\n  %r = fdiv double %a, %b\n  ret double %r\n}
define double @fload(ptr %p) {\n  %q = getelementptr double, ptr %p, i32 2\n  %r = load double, ptr %q\n  ret double %r\n}
define void @fstore(ptr %p, double %v) {\n  %q = getelementptr double, ptr %p, i32 2\n  store double %v, ptr %q\n  ret void\n}
";

/// The bytes of the first instruction whose mnemonic is `mnemonic`, read from
/// the disassembly of `object`.
fn bytes_of(
    dump: &str,
    mnemonic: &str,
) -> i64 {
    let line = dump
        .lines()
        .find(|line| line.split_once('|').is_some_and(|(_, text)| text.trim().starts_with(mnemonic)))
        .unwrap_or_else(|| panic!("no `{mnemonic}` in:\n{dump}"));
    let (head, _) = line.split_once('|').expect("a bar");
    let hex = head.split_once(':').expect("an address").1;
    i64::try_from(hex.split_whitespace().count()).expect("a count")
}

/// The table against the object of `width`: one machine prices both.
fn table_matches_the_object(width: Width) {
    let bytes = object(MODULE, width, Options::default()).unwrap_or_else(|why| panic!("{why}"));
    let dir = std::env::temp_dir().join(format!("wasm-size-costs-{}-{width:?}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a directory");
    let path = dir.join("o.wasm");
    std::fs::write(&path, &bytes).expect("the object");
    let out = std::process::Command::new("wasm-objdump").arg("-d").arg(&path).output().expect("wasm-objdump");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let dump = String::from_utf8(out.stdout).expect("text");
    let _ = std::fs::remove_dir_all(&dir);

    let costs = WasmMachine.size_costs();
    let found = [
        ("add", costs.add, bytes_of(&dump, "i32.add")),
        ("multiply", costs.multiply, bytes_of(&dump, "i32.mul")),
        ("divide", costs.divide, bytes_of(&dump, "i32.div_s")),
        ("shift", costs.shift, bytes_of(&dump, "i32.shl")),
        ("load", costs.load, bytes_of(&dump, "i32.load")),
        ("store", costs.store, bytes_of(&dump, "i32.store")),
        ("call", costs.call, bytes_of(&dump, "call ")),
        ("branch", costs.branch, bytes_of(&dump, "br_if")),
        ("float_add", costs.float_add, bytes_of(&dump, "f64.add")),
        ("float_multiply", costs.float_multiply, bytes_of(&dump, "f64.mul")),
        ("float_divide", costs.float_divide, bytes_of(&dump, "f64.div")),
        ("float_load", costs.float_load, bytes_of(&dump, "f64.load")),
        ("float_store", costs.float_store, bytes_of(&dump, "f64.store")),
    ];
    let wrong: Vec<String> = found
        .iter()
        .filter(|(_, table, assembled)| table != assembled)
        .map(|(class, table, assembled)| format!("{class}: table {table}, assembled {assembled}"))
        .collect();
    assert!(wrong.is_empty(), "{width:?}: {}", wrong.join("; "));
}

#[test]
fn the_size_table_is_the_bytes_of_each_class_as_assembled_at_wasm32() {
    table_matches_the_object(Width::W32);
}

#[test]
fn the_size_table_is_the_bytes_of_each_class_as_assembled_at_wasm64() {
    table_matches_the_object(Width::W64);
}
