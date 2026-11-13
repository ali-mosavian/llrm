use llrm_mir::interpret::{Val, run};
use llrm_mir::module::Module;
use llrm_mir::types::FloatKind;

use super::expanded;

fn conversion(
    bits: u32,
    signed: bool,
    float: &str,
) -> Module {
    let mangled = if float == "double" { "f64" } else { "f32" };
    let name = format!("llvm.experimental.fpto{}i.checked.i{bits}.{mangled}", if signed { 's' } else { 'u' });
    let text = format!(
        "declare {{ i{bits}, i1 }} @{name}({float})\n\
         define i{bits} @f({float} %x) {{\nb0:\n  %p = call {{ i{bits}, i1 }} @{name}({float} %x)\n  \
         %v = extractvalue {{ i{bits}, i1 }} %p, 0\n  %ok = extractvalue {{ i{bits}, i1 }} %p, 1\n  \
         br i1 %ok, label %fits, label %out\nfits:\n  ret i{bits} %v\nout:\n  ret i{bits} 77\n}}\n"
    );
    llrm_mir::parse::module(&text).unwrap_or_else(|error| panic!("{error}\n{text}"))
}

fn answer(
    module: &Module,
    x: f64,
) -> Val {
    run(module, "f", vec![Val::Float(FloatKind::Double, x.to_bits())], 10_000).unwrap_or_else(|trap| panic!("{trap:?}"))
}

/// The expansion gives the intrinsic's answers at the edges of every width, on the integer (wide) and on the
/// float (none), and leaves no call: a NaN, an infinity, the first float past each end and the last one in.
#[test]
fn an_expanded_checked_conversion_answers_as_the_intrinsic_does() {
    fn wide(bits: u32) -> Option<u32> {
        (bits <= 32).then_some(if bits <= 16 { 32 } else { 64 })
    }
    for (bits, signed) in [(8, true), (16, true), (32, true), (8, false), (16, false), (32, false)] {
        let before = conversion(bits, signed, "double");
        let (low, high) = if signed { (-(2f64.powi(bits as i32 - 1)), 2f64.powi(bits as i32 - 1)) } else { (0.0, 2f64.powi(bits as i32)) };
        let inputs = [
            0.0, 1.5, -1.5, -0.5, low, low - 1.0, low - 0.5, low + 0.5, high, high - 1.0, high - 0.5, high + 1.0,
            f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 1e30, -1e30,
        ];
        for mode in [Some(wide as fn(u32) -> Option<u32>), None] {
            let mut after = before.clone();
            expanded(&mut after, |bits| mode.and_then(|wide| wide(bits))).expect("expands");
            let text = llrm_mir::print::module(&after);
            assert_eq!(llrm_mir::verify::verify(&after), Vec::<String>::new(), "{text}");
            assert!(!text.contains("call {"), "i{bits} kept its call:\n{text}");
            for x in inputs {
                // the interpreter reads a float past the wide integer as poison; the hardware's answer
                // (the most negative value, out of range for the narrow one) is tests/run/nib's to show
                if mode.is_some() && !(x.trunc().abs() < 2f64.powi(if bits <= 16 { 31 } else { 63 })) {
                    continue;
                }
                assert_eq!(answer(&after, x), answer(&before, x), "i{bits} signed {signed} wide {} of {x}", mode.is_some());
            }
        }
    }
}

/// The two inputs the check must tell apart, on the wide path: -2^31 fits an i32 and 2^31 does not, though both
/// leave the low dword 0x80000000.
#[test]
fn the_wide_check_tells_the_minimum_from_the_first_value_past_the_maximum() {
    let mut module = conversion(32, true, "double");
    expanded(&mut module, |_| Some(64)).expect("expands");
    assert_eq!(answer(&module, -2147483648.0), Val::Int { bits: 0x8000_0000, width: 32 });
    assert_eq!(answer(&module, 2147483648.0), Val::Int { bits: 77, width: 32 });
    assert_eq!(answer(&module, -2147483649.0), Val::Int { bits: 77, width: 32 });
}
