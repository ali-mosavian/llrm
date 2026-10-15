//! Interchange on nests the interpreter runs, before and after: a column walk
//! of a table (swapped), a row walk (kept), a table whose cells a later pass
//! reads back across the swap (kept), and an inner count that follows the
//! outer counter (kept).

use llrm_mir::module::Module;

use super::Interchange;
use crate::testing::{managed, parsed, printed, results};

const INPUTS: &[&[i128]] = &[&[0], &[3], &[-7]];

/// `a[i][j] = body` over `outer` columns `j` of `inner` rows `i` of 16 words,
/// `inner_stride` bytes between the cells of `i` and `outer_stride` of `j`;
/// then a checksum of the table. `inner_bound` is the inner loop's test.
fn table(
    outer: i16,
    inner: i16,
    inner_bound: &str,
    outer_stride: i16,
    inner_stride: i16,
    body: &str,
) -> String {
    format!(
        "@a = global [512 x i16] zeroinitializer
@v = global i16 5

define i16 @f(i16 %x) {{
b0:
  br label %b1

b1:
  %j = phi i16 [ 0, %b0 ], [ %j.next, %b5 ]
  %run = icmp slt i16 %j, {outer}
  br i1 %run, label %b2, label %b6

b2:
  %c = mul i16 %j, {outer_stride}
  br label %b3

b3:
  %i = phi i16 [ 0, %b2 ], [ %i.next, %b4 ]
  %more = icmp slt i16 %i, {inner_bound}
  br i1 %more, label %b4, label %b5

b4:
  %r = mul i16 %i, {inner_stride}
  %o = add i16 %r, %c
  %o2 = add i16 %o, 64
  %p = getelementptr i8, ptr @a, i16 %o2
{body}  %i.next = add i16 %i, 1
  br label %b3

b5:
  %j.next = add i16 %j, 1
  br label %b1

b6:
  %q0 = getelementptr i8, ptr @a, i16 64
  %w0 = load i16, ptr %q0
  %q1 = getelementptr i8, ptr @a, i16 70
  %w1 = load i16, ptr %q1
  %q2 = getelementptr i8, ptr @a, i16 {far}
  %w2 = load i16, ptr %q2
  %s = add i16 %w0, %w1
  %t = add i16 %s, %w2
  %z = add i16 %t, %x
  ret i16 %z
}}
",
        far = 64 + 2 * (inner * 16 - 1)
    )
}

const FILL: &str = "  %v = add i16 %i, %x
  %v2 = mul i16 %v, 3
  %v3 = add i16 %v2, %j
  store i16 %v3, ptr %p
";

/// One cell read by every trip: a repeat of the outer loop's passes, which
/// jamming is for.
const READS_ONE_CELL: &str = "  %k = load i16, ptr @v
  %v = add i16 %k, %i
  store i16 %v, ptr %p
";

/// `a[i-1][j+1]` is read where `a[i][j]` is written: the vector `(1, -1)`.
const READS_BACK: &str = "  %back = getelementptr i8, ptr %p, i16 -30
  %b = load i16, ptr %back
  %v = add i16 %b, %i
  %v2 = add i16 %v, %x
  store i16 %v2, ptr %p
";

/// `text` through the pass: whether it changed, and it computes what it did.
fn through(text: &str) -> (bool, Module) {
    let mut module = parsed(text);
    let before = (printed(&module), results(&module, INPUTS));
    let after = managed(&mut module, Interchange);
    assert_eq!(results(&module, INPUTS), before.1, "{after}");
    (after != before.0, module)
}

/// The column walk of a 16 word row table: each `i` steps 32 bytes, each `j`
/// 2. Interchanged, the header of the outer loop counts the 16 that the inner
/// did, and the work that stood before the inner loop moves into it.
#[test]
fn a_column_walk_is_swapped() {
    let (changed, module) = through(&table(8, 16, "16", 2, 32, FILL));
    let text = printed(&module);
    assert!(changed, "{text}");
    assert!(text.contains("icmp slt i16 %i, 8") && text.contains("icmp slt i16 %j, 16"), "{text}");
    assert!(text.contains("b4:\n  %c = mul i16 %i, 2"), "{text}");
}

/// The row walk is already contiguous.
#[test]
fn a_row_walk_is_kept() {
    let (changed, module) = through(&table(8, 16, "16", 32, 2, FILL));
    assert!(!changed, "{}", printed(&module));
}

/// The cell a pass writes is read by the next column's pass one row back, which
/// the swap would make the later pass's: the table differs, so it stays.
#[test]
fn a_table_read_back_across_the_swap_is_kept() {
    let (changed, module) = through(&table(8, 16, "16", 2, 32, READS_BACK));
    assert!(!changed, "{}", printed(&module));
}

/// An inner count that is the outer counter is not rectangular.
#[test]
fn a_triangle_is_kept() {
    let (changed, module) = through(&table(8, 16, "%j", 2, 32, FILL));
    assert!(!changed, "{}", printed(&module));
}

/// A cell every trip reads is a repeat of the outer loop's passes, which
/// unroll-and-jam keeps and a swap would take away (pipeline_tests'
/// `only_o3_jams_a_nest_that_sweeps_the_same_cells` stopped jamming, its nest
/// swapped to walk nothing).
#[test]
fn a_nest_that_repeats_a_cell_is_left_to_the_jam() {
    let (changed, module) = through(&table(8, 16, "16", 2, 32, READS_ONE_CELL));
    assert!(!changed, "{}", printed(&module));
}
