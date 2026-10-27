use llrm_mir::interpret::{Val, run};
use llrm_mir::module::Module;

use super::expanded;

fn absolute(bits: u32) -> Module {
    let text = format!(
        "declare i{bits} @llvm.abs.i{bits}(i{bits}, i1)\n\
         define i{bits} @f(i{bits} %x) {{\nb0:\n  %r = call i{bits} @llvm.abs.i{bits}(i{bits} %x, i1 false)\n  ret i{bits} %r\n}}\n"
    );
    llrm_mir::parse::module(&text).unwrap_or_else(|error| panic!("{error}\n{text}"))
}

fn answer(
    module: &Module,
    bits: u32,
    x: u128,
) -> Val {
    run(module, "f", vec![Val::Int { bits: x, width: bits }], 10_000).unwrap_or_else(|trap| panic!("{trap:?}"))
}

/// The expansion gives the answers of the intrinsic (the minimum value maps to
/// itself) and leaves no call of it.
#[test]
fn an_expanded_abs_answers_as_the_intrinsic_does() {
    for bits in [8, 16, 32] {
        let before = absolute(bits);
        let mut after = before.clone();
        expanded(&mut after, |_| true).expect("expands");
        let text = llrm_mir::print::module(&after);
        assert_eq!(llrm_mir::verify::verify(&after), Vec::<String>::new(), "{text}");
        assert!(!text.contains("call i"), "abs.i{bits} kept its call:\n{text}");
        let mask = llrm_mir::context::mask(bits);
        let mut inputs = vec![0, 1, mask, 1 << (bits - 1), (1 << (bits - 1)) - 1, (1 << (bits - 1)) + 1];
        if bits == 8 {
            inputs = (0..=255).collect();
        }
        for x in inputs {
            assert_eq!(answer(&after, bits, x), answer(&before, bits, x), "abs.i{bits}({x:#x})");
        }
    }
}

/// A target that does not list `abs` keeps the call.
#[test]
fn an_abs_the_target_does_not_list_is_left_alone() {
    let mut module = absolute(16);
    let before = llrm_mir::print::module(&module);
    expanded(&mut module, |operation| operation == "ctpop").expect("expands");
    assert_eq!(llrm_mir::print::module(&module), before);
}
