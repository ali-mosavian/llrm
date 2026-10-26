use llrm_mir::interpret::{Val, run};
use llrm_mir::module::Module;

use super::{bits_reversed, expanded};

fn swapping(bits: u32) -> Module {
    let text = format!(
        "declare i{bits} @llvm.bswap.i{bits}(i{bits})\n\
         define i{bits} @f(i{bits} %x) {{\nb0:\n  %r = call i{bits} @llvm.bswap.i{bits}(i{bits} %x)\n  ret i{bits} %r\n}}\n"
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

/// The expansion at each width gives the answers of the intrinsic and leaves
/// no call of one.
#[test]
fn an_expanded_byte_swap_answers_as_the_intrinsic_does() {
    for bits in [16, 32, 48, 64, 128] {
        let before = swapping(bits);
        let mut after = before.clone();
        expanded(&mut after, |_| true).expect("expands");
        assert_eq!(llrm_mir::verify::verify(&after), Vec::<String>::new(), "{}", llrm_mir::print::module(&after));
        let text = llrm_mir::print::module(&after);
        assert!(!text.contains("call i"), "bswap.i{bits} kept its call:\n{text}");
        let mask = llrm_mir::context::mask(bits);
        let mut state = 0x9e37_79b9_7f4a_7c15_u128;
        let mut inputs = vec![0, 1, mask, 0x0123_4567_89ab_cdef & mask];
        for _ in 0..100 {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            inputs.push((state ^ state >> 29) & mask);
        }
        for x in inputs {
            assert_eq!(answer(&after, bits, x), answer(&before, bits, x), "bswap.i{bits}({x:#x})");
        }
    }
}

/// A target that does not list `bswap` keeps the call, and selection refuses
/// it by name.
#[test]
fn a_byte_swap_the_target_does_not_list_is_left_alone() {
    let mut module = swapping(16);
    let before = llrm_mir::print::module(&module);
    expanded(&mut module, |operation| operation == "ctpop").expect("expands");
    assert_eq!(llrm_mir::print::module(&module), before);
}

/// `bswap` of an `i16`: two shifts and masks, and an or.
#[test]
fn a_byte_swap_is_shifts_masks_and_an_or() {
    let mut module = swapping(16);
    expanded(&mut module, |_| true).expect("expands");
    let after = llrm_mir::print::module(&module);
    let body: Vec<&str> =
        after.lines().skip_while(|line| *line != "b0:").skip(1).take_while(|line| *line != "}").collect();
    assert_eq!(
        body,
        [
            "  %0 = shl i16 %x, 8",
            "  %1 = and i16 %0, -256",
            "  %2 = lshr i16 %x, 8",
            "  %3 = and i16 %2, 255",
            "  %4 = or i16 %1, %3",
            "  ret i16 %4",
        ]
    );
}

/// `llvm.bitreverse` reached the old selector as a call it refused by name
/// (`--isel=old` and the default path); expanded, it answers as the
/// intrinsic does at every power-of-two width and leaves no call.
#[test]
fn an_expanded_bit_reversal_answers_as_the_intrinsic_does() {
    for bits in [8, 16, 32, 64] {
        let text = format!(
            "declare i{bits} @llvm.bitreverse.i{bits}(i{bits})\n\
             define i{bits} @f(i{bits} %x) {{\nb0:\n  %r = call i{bits} @llvm.bitreverse.i{bits}(i{bits} %x)\n  ret i{bits} %r\n}}\n"
        );
        let before = llrm_mir::parse::module(&text).unwrap_or_else(|error| panic!("{error}\n{text}"));
        let mut after = before.clone();
        bits_reversed(&mut after, |_| true).expect("expands");
        assert_eq!(llrm_mir::verify::verify(&after), Vec::<String>::new(), "{}", llrm_mir::print::module(&after));
        assert!(!llrm_mir::print::module(&after).contains("call i"), "bitreverse.i{bits} kept its call");
        let mask = llrm_mir::context::mask(bits);
        let mut state = 0x9e37_79b9_7f4a_7c15_u128;
        let mut inputs = vec![0, 1, 2, 0x80, mask, 0x0123_4567_89ab_cdef & mask];
        for _ in 0..100 {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            inputs.push((state ^ state >> 29) & mask);
        }
        for x in inputs {
            let want = Val::Int { bits: x.reverse_bits() >> (128 - bits), width: bits };
            assert_eq!(answer(&after, bits, x), want, "bitreverse.i{bits}({x:#x})");
        }
    }
}
