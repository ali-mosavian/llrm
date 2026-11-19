use llrm_wasm::Width;
use llrm_wasm::dag::Options;
use llrm_wasm::driver::{Level, assembly};

const IR: &str = "target datalayout = \"e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20\"
target triple = \"wasm32-unknown-unknown\"
@bitpos = global i32 0
@stream = global [64 x i8] zeroinitializer
declare void @use(i32)
define i32 @f(i32 %lim) {
  %p = load i32, ptr @bitpos, align 4
  %i = ashr i32 %p, 3
  %g = getelementptr inbounds i8, ptr @stream, i32 %i
  %b = load i8, ptr %g, align 1
  %z = zext i8 %b to i32
  %m = and i32 %p, 7
  %x = xor i32 %m, 7
  %s = lshr i32 %z, %x
  %bit = and i32 %s, 1
  %n = add nsw i32 %p, 1
  store i32 %n, ptr @bitpos, align 4
  %c = icmp sgt i32 %bit, %lim
  br i1 %c, label %a, label %e
a:
  call void @use(i32 %bit)
  br label %e
e:
  ret i32 %bit
}
";

/// A store to a global kept the load of an unrelated array in its chain: only
/// accesses whose pointer was no global carried the IR pointer the alias
/// analysis needs, so the pair was a may-alias. The byte load stayed above the
/// store and its value waited in a local (`local.set`/`local.get`, a second
/// local). The pin records the IR pointer on every access; the load follows
/// the store and is stackified. huffman_decode -O3 was +64 B over the pin.
#[test]
fn a_store_to_a_global_does_not_hold_a_load_of_another_object() {
    for level in [Level::O2, Level::O3] {
        let text = assembly(IR, Width::W32, Options { level, ..Options::default() }).unwrap();
        let store = text.find("i32.store\tbitpos").expect("the store");
        let load = text.find("i32.load8_u").expect("the byte load");
        assert!(store < load, "{text}");
        assert!(text.contains(".local  \ti32\n"), "{text}");
    }
}
