//! `SimplifyDemandedBits` of `CTPOP`: one demanded bit is the parity.

use super::dag_function::selected;

fn names(body: &str) -> Vec<String> {
    let ir = format!("target datalayout = \"e-p:32:32-i64:32-n8:16:32\"\ndeclare i32 @llvm.ctpop.i32(i32)\n{body}");
    let module = llrm_mir::parse::module(&ir).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    done.body.insns().iter().filter_map(|i| i.what.as_ref()?.name.as_deref().map(str::to_owned)).collect()
}

/// `__builtin_parity(unsigned short)` (ctpop.i32 of a zext, and 1) at -O1 -m32 was the whole popcount
/// expansion (21 instructions, 66 B); the pin folds the one demanded bit of
/// the ctpop to ISD::PARITY and emits `xor cl, ch; setnp`.
#[test]
fn the_low_bit_of_a_population_count_is_the_parity_flag() {
    let seen = names(
        "define i32 @f(i16 zeroext %x) {
  %z = zext i16 %x to i32
  %c = call i32 @llvm.ctpop.i32(i32 %z)
  %b = and i32 %c, 1
  ret i32 %b
}",
    );
    assert!(seen.iter().any(|name| name == "setnp") && !seen.iter().any(|name| name == "sub"), "{seen:?}");
}
