use llrm_mir::interpret::{Val, run};
use llrm_mir::module::Module;

use super::expanded;

fn comparing(
    name: &str,
    result: u32,
    operand: u32,
) -> Module {
    let text = format!(
        "declare i{result} @llvm.{name}.i{result}.i{operand}(i{operand}, i{operand})\n\
         define i{result} @f(i{operand} %a, i{operand} %b) {{\nb0:\n  %r = call i{result} @llvm.{name}.i{result}.i{operand}(i{operand} %a, i{operand} %b)\n  ret i{result} %r\n}}\n"
    );
    llrm_mir::parse::module(&text).unwrap_or_else(|error| panic!("{error}\n{text}"))
}

fn answer(
    module: &Module,
    operand: u32,
    a: u128,
    b: u128,
) -> Val {
    let arguments = vec![Val::Int { bits: a, width: operand }, Val::Int { bits: b, width: operand }];
    run(module, "f", arguments, 10_000).unwrap_or_else(|trap| panic!("{trap:?}"))
}

/// The expansion gives the intrinsic's answers over every pair of the edge
/// values of each operand width, into several result widths.
#[test]
fn an_expanded_compare_answers_as_the_intrinsic_does() {
    for name in ["scmp", "ucmp"] {
        for (result, operand) in [(2, 8), (8, 8), (8, 16), (16, 32), (32, 64)] {
            let before = comparing(name, result, operand);
            let mut after = before.clone();
            expanded(&mut after, |_| true).expect("expands");
            assert_eq!(llrm_mir::verify::verify(&after), Vec::<String>::new(), "{}", llrm_mir::print::module(&after));
            let text = llrm_mir::print::module(&after);
            assert!(!text.contains("call i"), "{name}.i{result}.i{operand} kept its call:\n{text}");
            let mask = llrm_mir::context::mask(operand);
            let edges = [0, 1, 2, mask >> 1, (mask >> 1) + 1, mask - 1, mask];
            for a in edges {
                for b in edges {
                    assert_eq!(
                        answer(&after, operand, a, b),
                        answer(&before, operand, a, b),
                        "{name}.i{result}.i{operand}({a:#x}, {b:#x})"
                    );
                }
            }
        }
    }
}

/// Only the compare a target lists is expanded.
#[test]
fn a_compare_the_target_does_not_list_is_left_alone() {
    let mut module = comparing("ucmp", 8, 16);
    let before = llrm_mir::print::module(&module);
    expanded(&mut module, |operation| operation == "scmp").expect("expands");
    assert_eq!(llrm_mir::print::module(&module), before);
}

/// `ucmp` of two `i16` into an `i8`: two compares, widened, and a subtract.
#[test]
fn a_three_way_compare_is_two_compares_and_a_subtract() {
    let mut module = comparing("ucmp", 8, 16);
    expanded(&mut module, |_| true).expect("expands");
    let after = llrm_mir::print::module(&module);
    let body: Vec<&str> =
        after.lines().skip_while(|line| *line != "b0:").skip(1).take_while(|line| *line != "}").collect();
    assert_eq!(
        body,
        [
            "  %0 = icmp ugt i16 %a, %b",
            "  %1 = zext i1 %0 to i8",
            "  %2 = icmp ult i16 %a, %b",
            "  %3 = zext i1 %2 to i8",
            "  %4 = sub i8 %1, %3",
            "  ret i8 %4",
        ]
    );
}
