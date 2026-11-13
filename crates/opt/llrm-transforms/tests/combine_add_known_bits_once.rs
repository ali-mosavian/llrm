//! `visitAdd` asked the known bits of each operand again for the disjoint-or
//! fold, `nsw` and `nuw`: 18 asks per add of two loads, 10 now that the pin's
//! `WithCache` shares them (it was 1.5x the pin's `visitAdd` cost per call).

use llrm_mir::counts::{Event, get};
use llrm_mir::passes::PassManager;
use llrm_transforms::combine_pass::Combine;

/// Known-bits asks of one `combine` over `@f` holding `n` adds of two loads.
fn asks(n: usize) -> u64 {
    let mut text = String::from("define void @f(ptr %p, ptr %q) {\nentry:\n");
    for k in 0..n {
        text += &format!(
            "  %a{k} = load i32, ptr %p\n  %b{k} = load i32, ptr %q\n  %s{k} = add i32 %a{k}, %b{k}\n  store i32 %s{k}, ptr %q\n"
        );
    }
    text += "  ret void\n}\n";
    let mut module = llrm_mir::parse::module(&text).unwrap_or_else(|why| panic!("{why:?}"));
    let mut manager = PassManager::default();
    manager.add(Combine { enabled: true, phi_constant_users: false });
    let before = get(Event::KnownBitsComputed);
    manager.run_module(&mut module, std::rc::Rc::new(llrm_mir::target::Neutral)).expect("runs");
    get(Event::KnownBitsComputed) - before
}

#[test]
fn an_add_shares_the_known_bits_of_its_operands() {
    let (one, five) = (asks(1), asks(5));
    let per_add = (five - one) / 4;
    assert_eq!(per_add, 10, "asks for 1 add {one}, for 5 adds {five}");
}
