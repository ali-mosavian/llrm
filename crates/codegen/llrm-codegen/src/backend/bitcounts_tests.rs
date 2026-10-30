use llrm_mir::interpret::{Val, run};
use llrm_mir::module::Module;

use super::expanded;

/// `@f(x)` counting with `name` at `bits`, as a module.
fn counting(
    name: &str,
    bits: u32,
) -> Module {
    let (parameters, arguments) = if name == "ctpop" { ("", "") } else { (", i1", ", i1 false") };
    let text = format!(
        "declare i{bits} @llvm.{name}.i{bits}(i{bits}{parameters})\n\
         define i{bits} @f(i{bits} %x) {{\nb0:\n  %r = call i{bits} @llvm.{name}.i{bits}(i{bits} %x{arguments})\n  ret i{bits} %r\n}}\n"
    );
    llrm_mir::parse::module(&text).unwrap_or_else(|error| panic!("{error}\n{text}"))
}

fn expanded_all(module: &Module) -> Module {
    let mut after = module.clone();
    expanded(&mut after, |_| true).expect("expands");
    assert_eq!(llrm_mir::verify::verify(&after), Vec::<String>::new(), "{}", llrm_mir::print::module(&after));
    after
}

fn answer(
    module: &Module,
    bits: u32,
    x: u128,
) -> Val {
    let argument = vec![Val::Int { bits: x, width: bits }];
    run(module, "f", argument, 10_000).unwrap_or_else(|trap| panic!("@f({x}): {trap:?}"))
}

/// The inputs worth running at `bits`: every value up to 8 bits, else the
/// edges, each single bit and a run of pseudo-random ones.
fn inputs(bits: u32) -> Vec<u128> {
    let mask = llrm_mir::context::mask(bits);
    if bits <= 8 {
        return (0..=mask).collect();
    }
    let mut all = vec![0, 1, 2, 3, mask, mask - 1, mask >> 1, (mask >> 1) + 1];
    all.extend((0..bits).map(|bit| 1u128 << bit));
    let mut state = 0x9e37_79b9_7f4a_7c15_u128;
    for _ in 0..200 {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        all.push((state ^ state >> 29) & mask);
    }
    all
}

/// The expansion of each count, at each width (a power of two or not), gives
/// the answers of the intrinsic and leaves no call of one.
#[test]
fn an_expanded_bit_count_answers_as_the_intrinsic_does() {
    for name in ["ctpop", "ctlz", "cttz"] {
        for bits in [1, 5, 8, 16, 17, 32, 64, 100, 128] {
            let before = counting(name, bits);
            let after = expanded_all(&before);
            let text = llrm_mir::print::module(&after);
            assert!(!text.contains("call i"), "{name}.i{bits} kept its call:\n{text}");
            for x in inputs(bits) {
                assert_eq!(answer(&after, bits, x), answer(&before, bits, x), "{name}.i{bits}({x:#x})");
            }
        }
    }
}

/// Only the counts a target lists are expanded: the others reach selection,
/// which refuses them by name.
#[test]
fn a_count_the_target_does_not_list_is_left_alone() {
    let mut module = counting("ctlz", 16);
    let before = llrm_mir::print::module(&module);
    expanded(&mut module, |operation| operation == "ctpop").expect("expands");
    assert_eq!(llrm_mir::print::module(&module), before);
}

/// `ctpop` of an `i8`: the parallel count in shifts, masks and adds.
#[test]
fn a_population_count_is_shifts_and_masks() {
    let after = llrm_mir::print::module(&expanded_all(&counting("ctpop", 8)));
    let body: Vec<&str> =
        after.lines().skip_while(|line| *line != "b0:").skip(1).take_while(|line| *line != "}").collect();
    assert_eq!(
        body,
        [
            "  %0 = lshr i8 %x, 1",
            "  %1 = and i8 %0, 85",
            "  %2 = sub i8 %x, %1",
            "  %3 = and i8 %2, 51",
            "  %4 = lshr i8 %2, 2",
            "  %5 = and i8 %4, 51",
            "  %6 = add i8 %3, %5",
            "  %7 = lshr i8 %6, 4",
            "  %8 = add i8 %6, %7",
            "  %9 = and i8 %8, 15",
            "  %10 = and i8 %9, 15",
            "  ret i8 %10",
        ]
    );
}
