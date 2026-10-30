//! `getBaseObjectSize` on each kind of base, with the sizes worked by hand.

use super::objectsize::{ObjectSizeOpts, get_base_object_size};
use crate::datalayout::DataLayout;
use crate::module::Operand;
use crate::parse;
use crate::valuetracking::Query;

const TEXT: &str = "@g = global [10 x i32] zeroinitializer, align 64
@w = weak global i32 0
@e = external global i32
define void @f(ptr byval([3 x i64]) align 16 %a, ptr %u) {
b0:
  %x = alloca i64, i32 3
  %y = alloca [5 x i8], align 8
  %pg = getelementptr i8, ptr @g, i64 0
  %pw = getelementptr i8, ptr @w, i64 0
  %pe = getelementptr i8, ptr @e, i64 0
  %pn = getelementptr i8, ptr null, i64 0
  ret void
}
";

/// The size of the base each of `f`'s bases has, in the order of the text:
/// `@g`, `@w`, `@e`, null, `%x`, `%y`, `%a`, `%u`.
fn sizes(opts: ObjectSizeOpts) -> Vec<Option<u64>> {
    let module = parse::module(TEXT).expect("parses");
    let layout = DataLayout::default();
    let function = module
        .globals
        .iter()
        .find_map(|global| match &global.kind {
            crate::module::GlobalKind::Function(function) if global.name.owned().as_deref() == Some("f") => {
                Some(&**function)
            }
            _ => None,
        })
        .expect("@f");
    let q = Query::new(&module, function, &layout);
    let gep_base = |name: &str| {
        let (_, inst) = function
            .walk()
            .find(|&(_, inst)| {
                function.instruction(inst).result.is_some_and(|r| function.value(r).name.as_deref() == Some(name))
            })
            .expect("a named instruction");
        function.instruction(inst).operands[0]
    };
    let value = |name: &str| {
        let (_, inst) = function
            .walk()
            .find(|&(_, inst)| {
                function.instruction(inst).result.is_some_and(|r| function.value(r).name.as_deref() == Some(name))
            })
            .expect("a named instruction");
        Operand::Value(function.instruction(inst).result.unwrap())
    };
    let params = function.parameters();
    [
        gep_base("pg"),
        gep_base("pw"),
        gep_base("pe"),
        gep_base("pn"),
        value("x"),
        value("y"),
        Operand::Value(params[0]),
        Operand::Value(params[1]),
    ]
    .into_iter()
    .map(|base| get_base_object_size(&q, base, opts))
    .collect()
}

/// Sizes without rounding: 40 (10 x i32), no weak or declared global, null
/// unknown, 24 (i64 x 3), 5, 24 (byval [3 x i64]), no size for a plain
/// pointer.
#[test]
fn the_size_of_each_base() {
    let opts = ObjectSizeOpts { null_is_unknown_size: true, round_to_align: false };
    assert_eq!(sizes(opts), [Some(40), None, None, None, Some(24), Some(5), Some(24), None]);
}

/// Rounded to the stated alignment: 40 to 64 (`align 64`), 5 to 8
/// (`align 8`), 24 to 32 (`align 16`); the alloca without one rounds to the
/// preferred alignment of i64.
#[test]
fn the_size_rounds_to_the_alignment() {
    let opts = ObjectSizeOpts { null_is_unknown_size: true, round_to_align: true };
    let rounded = sizes(opts);
    assert_eq!(rounded[0], Some(64));
    assert_eq!(rounded[5], Some(8));
    assert_eq!(rounded[6], Some(32));
}

/// Null is 0 bytes when it is not unknown and the address space is 0.
#[test]
fn null_is_empty_unless_it_is_unknown() {
    let opts = ObjectSizeOpts { null_is_unknown_size: false, round_to_align: false };
    assert_eq!(sizes(opts)[3], Some(0));
}
