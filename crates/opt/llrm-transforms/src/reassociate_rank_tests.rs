use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::{BlockId, Function, Operand, ValueId};
use llrm_mir::opcode::Opcode;

use super::Ranks;
use crate::testing::parsed;

fn value(
    function: &Function,
    name: &str,
) -> Operand {
    let id = (0..function.value_count() as u32)
        .map(ValueId)
        .find(|&v| function.value(v).name.as_deref() == Some(name))
        .unwrap_or_else(|| panic!("%{name}"));
    Operand::Value(id)
}

/// The rank of each named value of `@f`, its only block ranked, with phis
/// pinned.
fn ranks(
    text: &str,
    names: &[&str],
) -> Vec<u32> {
    let module = parsed(text);
    let function = module.global(module.named("f").expect("@f")).function().expect("a function");
    let context = &module.context;
    let layout = DataLayout::default();
    let order: Vec<BlockId> = function.layout().to_vec();
    let mut ranks = Ranks::build(function, &order, |inst| function.instruction(inst).opcode == Opcode::Phi);
    names.iter().map(|n| ranks.of(context, &layout, function, value(function, n))).collect()
}

/// Arguments are 3, 4, ...; an instruction is one more than its highest
/// operand.
#[test]
fn test_an_instruction_ranks_one_above_its_highest_operand() {
    let r = ranks(
        "define i32 @f(i32 %x, i32 %y) {
b0:
  %a = add i32 %x, 1
  %b = mul i32 %a, %y
  ret i32 %b
}
",
        &["x", "y", "a", "b"],
    );
    assert_eq!(r, [3, 4, 4, 5]);
}

/// `not`, `neg` do not count, so X and ~X, X and -X rank alike; without that
/// the sort would put a value and its negation apart and miss `x + -x`.
#[test]
fn test_a_not_and_a_neg_rank_as_their_operand() {
    let r = ranks(
        "define i32 @f(i32 %x) {
b0:
  %n = sub i32 0, %x
  %t = xor i32 %x, -1
  ret i32 %n
}
",
        &["x", "n", "t"],
    );
    assert_eq!(r, [3, 3, 3]);
}

/// A phi cannot move: it takes its block's rank (the second block is 5: one
/// argument is 3, the entry block 4), which cuts the loop through its back edge
/// (the walk would otherwise never end).
#[test]
fn test_a_phi_in_a_loop_takes_its_blocks_rank() {
    let r = ranks(
        "define i32 @f(i32 %n) {
b0:
  br label %l

l:
  %p = phi i32 [ 0, %b0 ], [ %q, %l ]
  %q = add i32 %p, 1
  %c = icmp eq i32 %q, %n
  br i1 %c, label %e, label %l

e:
  ret i32 %q
}
",
        &["p", "q"],
    );
    assert_eq!(r, [(5 << 16) + 1, (5 << 16) + 2]);
}

/// A rank asked twice is walked once: the second ask used to build a query
/// and an operand stack again (118M Ir against the pin's 37M at 512 chained
/// adds).
#[test]
fn test_a_cached_rank_builds_no_query_or_stack() {
    let module = parsed("define i32 @f(i32 %x) {\nb0:\n  %a = add i32 %x, 1\n  %b = add i32 %a, 2\n  ret i32 %b\n}\n");
    let function = module.global(module.named("f").expect("@f")).function().expect("a function");
    let layout = DataLayout::default();
    let order: Vec<BlockId> = function.layout().to_vec();
    let mut ranks = Ranks::build(function, &order, |_| false);
    let b = value(function, "b");
    let first = ranks.of(&module.context, &layout, function, b);
    let walks = ranks.walks();
    assert_eq!(ranks.of(&module.context, &layout, function, b), first);
    assert_eq!(ranks.walks(), walks);
}
