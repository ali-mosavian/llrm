use llrm_mir::interpret::{Val, run};
use llrm_mir::module::Module;

use super::expanded;

/// `@f(x, y, z)` calling `llvm.fshl` or `llvm.fshr`, `z` a parameter or the
/// constant `by`.
fn shifting(
    name: &str,
    bits: u32,
    by: Option<u32>,
) -> Module {
    let (parameters, amount) = match by {
        None => (format!("i{bits} %x, i{bits} %y, i{bits} %z"), "%z".to_owned()),
        Some(by) => (format!("i{bits} %x, i{bits} %y"), by.to_string()),
    };
    let text = format!(
        "declare i{bits} @llvm.{name}.i{bits}(i{bits}, i{bits}, i{bits})\n\
         define i{bits} @f({parameters}) {{\nb0:\n  %r = call i{bits} @llvm.{name}.i{bits}(i{bits} %x, i{bits} %y, i{bits} {amount})\n  ret i{bits} %r\n}}\n"
    );
    llrm_mir::parse::module(&text).unwrap_or_else(|error| panic!("{error}\n{text}"))
}

fn answer(
    module: &Module,
    bits: u32,
    arguments: &[u128],
) -> Val {
    let arguments = arguments.iter().map(|&bits_| Val::Int { bits: bits_, width: bits }).collect();
    run(module, "f", arguments, 10_000).unwrap_or_else(|trap| panic!("{trap:?}"))
}

/// The inputs tried at `bits`: edges, and a run of pseudo-random values.
fn inputs(bits: u32) -> Vec<u128> {
    let mask = llrm_mir::context::mask(bits);
    let mut state = 0x9e37_79b9_7f4a_7c15_u128;
    let mut out = vec![0, 1, mask, 1 << (bits - 1)];
    for _ in 0..40 {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        out.push((state ^ state >> 29) & mask);
    }
    out
}

/// The expansion gives the answers of the intrinsic, for a variable amount
/// (any value, including past the width) and for constants, at a power-of-two
/// width and at one that is not, and leaves no call of it.
#[test]
fn an_expanded_funnel_shift_answers_as_the_intrinsic_does() {
    for name in ["fshl", "fshr"] {
        for bits in [8, 16, 32, 12] {
            let amounts = [None, Some(0), Some(1), Some(bits - 1), Some(bits), Some(bits + 3)];
            for by in amounts {
                let before = shifting(name, bits, by);
                let mut after = before.clone();
                expanded(&mut after, |_| true, |_| false).expect("expands");
                let text = llrm_mir::print::module(&after);
                assert_eq!(llrm_mir::verify::verify(&after), Vec::<String>::new(), "{text}");
                assert!(!text.contains("call i"), "{name}.i{bits} kept its call:\n{text}");
                for &x in &inputs(bits) {
                    for &y in inputs(bits).iter().take(8) {
                        let zs: Vec<u128> = match by {
                            None => (0..=u128::from(2 * bits)).collect(),
                            Some(_) => vec![0],
                        };
                        for z in zs {
                            let arguments: Vec<u128> = if by.is_none() { vec![x, y, z] } else { vec![x, y] };
                            assert_eq!(
                                answer(&after, bits, &arguments),
                                answer(&before, bits, &arguments),
                                "{name}.i{bits} by {by:?}: ({x:#x}, {y:#x}, {z})"
                            );
                        }
                    }
                }
            }
        }
    }
}

/// A target that lists neither keeps the call (selection then refuses it by
/// name); one that lists `fshl` alone expands only that one.
#[test]
fn a_funnel_shift_the_target_does_not_list_is_left_alone() {
    let mut module = shifting("fshr", 16, None);
    let before = llrm_mir::print::module(&module);
    expanded(&mut module, |operation| operation == "fshl", |_| false).expect("expands");
    assert_eq!(llrm_mir::print::module(&module), before);
    let mut module = shifting("fshl", 16, None);
    expanded(&mut module, |operation| operation == "fshl", |_| false).expect("expands");
    assert!(!llrm_mir::print::module(&module).contains("call i"));
}

/// A rotate by a constant is two shifts and an or, and a zero amount is the
/// operand itself.
#[test]
fn a_constant_funnel_shift_is_two_shifts_and_an_or() {
    let mut module = shifting("fshl", 16, Some(3));
    expanded(&mut module, |_| true, |_| false).expect("expands");
    let after = llrm_mir::print::module(&module);
    let body: Vec<&str> =
        after.lines().skip_while(|line| *line != "b0:").skip(1).take_while(|line| *line != "}").collect();
    assert_eq!(body, ["  %0 = shl i16 %x, 3", "  %1 = lshr i16 %y, 13", "  %2 = or i16 %0, %1", "  ret i16 %2"]);
    let mut module = shifting("fshr", 16, Some(16));
    expanded(&mut module, |_| true, |_| false).expect("expands");
    let after = llrm_mir::print::module(&module);
    assert!(after.contains("ret i16 %y"), "{after}");
}

/// An `i32` `fshl` under the DAG was expanded to `shl`, `lshr 1`, `lshr` and
/// `or` before selection (`mov; shr; shl; shr; or`, `not` for the complement),
/// where the pin selects one `shld`.
#[test]
fn the_dag_keeps_a_byte_word_or_dword_funnel_shift() {
    use crate::backend::funnel::kept_for_isel;
    use crate::backend::shadow::Selector;
    assert!(
        kept_for_isel(Selector::Dag, "i32")
            && kept_for_isel(Selector::Dag, "i16")
            && kept_for_isel(Selector::Dag, "i8")
    );
    assert!(!kept_for_isel(Selector::Dag, "i64"), "an i64 funnel shift is not lowered");
    assert!(!kept_for_isel(Selector::Old, "i32"), "the old selector has no shld pattern");
    let mut module = shifting("fshl", 32, None);
    expanded(&mut module, |_| true, |bits| kept_for_isel(Selector::Dag, &format!("i{bits}"))).expect("expands");
    assert!(llrm_mir::print::module(&module).contains("@llvm.fshl.i32("), "the call stays");
}
