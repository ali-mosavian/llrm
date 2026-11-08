//! `visitSIGN_EXTEND_INREG`'s `(sext_in_reg (srl X, c), VT)` is `(sra X, c)`
//! where X has the sign bits to spare.

use super::dag_function::selected;

fn names(body: &str) -> Vec<String> {
    let ir = format!("target datalayout = \"e-p:32:32-i64:32-n8:16:32\"\n{body}");
    let module = llrm_mir::parse::module(&ir).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    done.body.insns().iter().filter_map(|i| i.what.as_ref()?.name.as_deref().map(str::to_owned)).collect()
}

/// `int f(short x) { return x / 4; }` came out `movsx; shr eax,2; movsx eax,ax`
/// (3 + 3 + 3 B): the demanded-bits fold turned the promoted `sra` into an
/// `srl`, and nothing turned the `sext_in_reg` of it back. The pin ends on
/// `movsx; sar eax,2`.
#[test]
fn a_signed_short_divide_by_a_power_of_two_widened_is_one_arithmetic_shift() {
    let seen = names(
        "define i32 @f(i16 signext %x) {
  %q = sdiv i16 %x, 4
  %e = sext i16 %q to i32
  ret i32 %e
}",
    );
    let last = seen.iter().rev().find(|name| !name.is_empty());
    assert_eq!(last.map(String::as_str), Some("sar"), "{seen:?}");
}
