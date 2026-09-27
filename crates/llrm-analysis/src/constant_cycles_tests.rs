//! Port of `tests/test_constant_cycles.py`.

use llrm_mir::module::Module;

use crate::consts::{Known, known};
use crate::memory::Unit;
use crate::testing::{DOS, function, layout, parsed, value};

/// A loop whose header joins 7 with `source + step`; `source` is the join
/// itself, or a value from outside the loop.
fn body_with_cycle(step: i64, external: bool) -> String {
    let source = if external { "%incoming" } else { "%joined" };
    format!(
        "define void @f(i32 %incoming, i1 %more) {{
b0:
  br label %b10

b10:
  %joined = phi i32 [ 7, %b0 ], [ %carried, %b10 ]
  %carried = add i32 {source}, {step}
  br i1 %more, label %b10, label %b20

b20:
  ret void
}}
"
    )
}

fn facts(text: &str) -> (Module, Option<Known>, Option<Known>) {
    let module = parsed(&format!("{DOS}{text}"));
    let dl = layout(&module);
    let f = function(&module, "f");
    let found = known(&Unit::of(&module, &dl, f), None, None, None);
    let (joined, carried) = (found.get(&value(f, "joined")).cloned(), found.get(&value(f, "carried")).cloned());
    (module, joined, carried)
}

#[test]
fn test_unchanged_loop_value_is_constant_through_its_backedge() {
    let (_, joined, carried) = facts(&body_with_cycle(0, false));
    assert_eq!(joined, Some(Known::new(7, 32)));
    assert_eq!(carried, Some(Known::new(7, 32)));
}

#[test]
fn test_changed_or_runtime_backedge_is_not_the_initial_constant() {
    for (step, external) in [(1, false), (0, true)] {
        let (_, joined, carried) = facts(&body_with_cycle(step, external));
        assert!(joined.is_none() && carried.is_none(), "{step} {external}");
    }
}

#[test]
fn test_unanchored_cycle_does_not_invent_a_constant() {
    let text = body_with_cycle(0, false).replace("[ 7, %b0 ], ", "");
    let (_, joined, _) = facts(&text);
    assert!(joined.is_none());
}

#[test]
fn test_block_order_does_not_change_cyclic_facts() {
    let text = body_with_cycle(0, false);
    let reordered = {
        let (head, rest) = text.split_at(text.find("b10:").unwrap());
        let (loop_, exit) = rest.split_at(rest.find("b20:").unwrap());
        let exit = exit.trim_end_matches("}\n");
        format!("{head}{exit}\n{loop_}}}\n")
    };
    assert_ne!(reordered, text);
    let (_, joined, carried) = facts(&text);
    let (_, joined_again, carried_again) = facts(&reordered);
    assert_eq!((joined, carried), (joined_again, carried_again));
}

#[test]
fn a_join_of_a_constant_and_an_argument_is_not_known() {
    let (_, joined, _) = facts(&body_with_cycle(0, false).replace("[ 7, %b0 ]", "[ %incoming, %b0 ]"));
    assert!(joined.is_none());
}
