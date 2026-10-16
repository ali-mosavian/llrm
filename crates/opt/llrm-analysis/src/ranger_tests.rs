//! The ranger gives what `ranges::edges_solved`'s table does.

use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::Module;

use super::{EdgeRanger, check};
use crate::cfg;
use crate::memory::Unit;
use crate::ranges::{self, Interval};
use crate::testing::{DOS, block, function, layout, parsed, value, with_registers};

struct Parsed {
    module: Module,
    layout: DataLayout,
}

impl Parsed {
    fn new(body: &str) -> Self {
        let module = parsed(&format!("{DOS}{body}"));
        let layout = layout(&module);
        Self { module, layout }
    }

    fn unit(&self) -> Unit<'_> {
        with_registers(Unit::of(&self.module, &self.layout, function(&self.module, "f")))
    }
}

fn agrees(parsed: &Parsed) {
    let unit = parsed.unit();
    let facts = unit.registers();
    let states = ranges::edges_solved(&unit, &facts, None).unwrap();
    check(&unit, &facts, &states);
}

/// A branch's two edges bound the compared value, and a join keeps neither.
#[test]
fn a_guard_bounds_the_value_on_its_edge_and_not_at_the_join() {
    let parsed = Parsed::new(
        "define i16 @f(i16 %x) {
b0:
  %c = icmp slt i16 %x, 5
  br i1 %c, label %b1, label %b2

b1:
  %y = add i16 %x, 1
  br label %b3

b2:
  br label %b3

b3:
  %p = phi i16 [ %y, %b1 ], [ %x, %b2 ]
  ret i16 %p
}
",
    );
    agrees(&parsed);
    let unit = parsed.unit();
    let facts = unit.registers();
    let ranger = EdgeRanger::new(&unit, &facts);
    let f = function(&parsed.module, "f");
    let (x, y) = (value(f, "x"), value(f, "y"));
    let at = |name: &str| cfg::id(block(f, name));
    assert_eq!(ranger.at(x, at("b1")).as_deref().map(|found| &found.high), Some(&4.into()));
    assert_eq!(ranger.at(y, at("b1")).as_deref().map(|found| &found.high), Some(&5.into()));
    assert_eq!(ranger.at(x, at("b2")).as_deref().map(|found| &found.low), Some(&5.into()));
    assert_eq!(ranger.at(x, at("b3")), None);
}

/// Two guards in a chain: the inner one starts from what the outer left.
#[test]
fn nested_guards_narrow_each_other() {
    let parsed = Parsed::new(
        "define i16 @f(i16 %x, i16 %y) {
b0:
  %c = icmp slt i16 %x, 5
  br i1 %c, label %b1, label %b4

b1:
  %d = icmp slt i16 %y, %x
  br i1 %d, label %b2, label %b3

b2:
  %z = mul i16 %y, 2
  ret i16 %z

b3:
  ret i16 %x

b4:
  ret i16 %y
}
",
    );
    agrees(&parsed);
}

/// A counted loop's header is a join: its body starts from the header's guard.
#[test]
fn a_loop_guard_is_known_in_the_body_and_the_exit() {
    let parsed = Parsed::new(
        "define void @f() {
b0:
  br label %b10

b10:
  %i = phi i16 [ 0, %b0 ], [ %next, %b40 ]
  %c = icmp slt i16 %i, 10
  br i1 %c, label %b20, label %b50

b20:
  %g = icmp slt i16 %i, 4
  br i1 %g, label %b30, label %b40

b30:
  %off = mul i16 %i, 2
  br label %b40

b40:
  %next = add i16 %i, 1
  br label %b10

b50:
  ret void
}
",
    );
    agrees(&parsed);
    let unit = parsed.unit();
    let facts = unit.registers();
    let ranger = EdgeRanger::new(&unit, &facts);
    let f = function(&parsed.module, "f");
    let i = value(f, "i");
    let high =
        |name: &str| ranger.at(i, cfg::id(block(f, name))).map(|found: std::rc::Rc<Interval>| found.high.clone());
    assert_eq!(high("b30"), Some(3.into()));
    assert_eq!(high("b20"), Some(9.into()));
}

/// The edges' facts were a table of every block, solved whole and again for
/// each edit (59 M of 308 M on d_faces -O1). They are answers kept by (value,
/// block), and an edit drops those of the blocks it reaches and those they
/// dominate, and no others.
#[test]
fn an_edit_drops_the_answers_below_it_and_keeps_those_above() {
    let parsed = Parsed::new(
        "define i16 @f(i16 %x, i16 %y) {
b0:
  %c = icmp slt i16 %x, 5
  br i1 %c, label %b1, label %b4

b1:
  %d = icmp slt i16 %y, %x
  br i1 %d, label %b2, label %b3

b2:
  %z = mul i16 %y, 2
  ret i16 %z

b3:
  ret i16 %x

b4:
  ret i16 %y
}
",
    );
    let unit = parsed.unit();
    let facts = unit.registers();
    let held = super::EdgeFacts::default();
    let f = function(&parsed.module, "f");
    let (x, y) = (value(f, "x"), value(f, "y"));
    let at = |name: &str| cfg::id(block(f, name));
    let ranger = held.over(&unit, &facts);
    for name in ["b1", "b2", "b3", "b4"] {
        ranger.at(x, at(name));
        ranger.at(y, at(name));
    }
    let asked = held.answers();
    assert!(asked >= 8, "premise: {asked} answers");
    let shape = unit.shape();
    let reached = std::collections::BTreeSet::from([at("b1")]);
    let after = held.keeping(|block| !crate::manager::dominated_by(&shape, &reached, block));
    // b1, b2 and b3 are b1's or below it; b4 is not.
    assert!(after.answers() < asked, "the answers below the edit stand");
    assert!(after.answers() >= 2, "b4's answers stand: {} of {asked}", after.answers());
}
