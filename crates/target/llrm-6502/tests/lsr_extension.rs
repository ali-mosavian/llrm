//! LSR on a loop nest: the outer loop's pointer step reaches into the inner
//! loop.

mod common;

use common::pipeline::datalayout;
use llrm_6502::driver::{Level, Request, assembly};

/// `s += g[i][j]` over 16 rows of 54 bytes, the counters bytes: the loop nest
/// of rep_count, as the optimizer hands it to LSR.
const ROWS: &str = "@g = global [16 x [54 x i8]] zeroinitializer\n\
    define i32 @f() {\n  br label %1\n\
    1:\n  %2 = phi i32 [ 0, %0 ], [ %13, %16 ]\n  %3 = phi i8 [ 0, %0 ], [ %17, %16 ]\n  \
    %4 = zext nneg i8 %3 to i16\n  %5 = getelementptr inbounds nuw [54 x i8], ptr @g, i16 %4\n  br label %6\n\
    6:\n  %7 = phi i32 [ %2, %1 ], [ %13, %6 ]\n  %8 = phi i8 [ 0, %1 ], [ %14, %6 ]\n  \
    %9 = zext nneg i8 %8 to i16\n  %10 = getelementptr inbounds nuw i8, ptr %5, i16 %9\n  \
    %11 = load i8, ptr %10, align 1\n  %12 = sext i8 %11 to i32\n  %13 = add nsw i32 %7, %12\n  \
    %14 = add nuw nsw i8 %8, 1\n  %15 = icmp samesign ult i8 %8, 53\n  br i1 %15, label %6, label %16\n\
    16:\n  %17 = add nuw nsw i8 %3, 1\n  %18 = icmp samesign ult i8 %3, 15\n  br i1 %18, label %1, label %19\n\
    19:\n  ret i32 %13\n}";

/// The outer loop's reduction expanded the row pointer plus `{0,+,1}<i16>` of
/// the inner loop: a second, 16-bit counter beside the byte one, and the load
/// through `row + index` added every trip (rep_count -O1: 108,818 clocks; with
/// the byte counter as the `y` of `(zp),y`, 81,992). The counter's extension is
/// the register, so the loop keeps the one counter.
#[test]
fn an_inner_byte_counter_is_the_index_of_the_row_pointer() {
    let ir = format!("target datalayout = \"{}\"\n{ROWS}\n", datalayout());
    let text = assembly(&ir, &Request::new(Level::O1)).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    let load = lines.iter().position(|line| line.contains("lda (rc")).expect("a load through a pointer cell");
    let before = &lines[load.saturating_sub(8)..load];
    assert!(
        !before.iter().any(|line| line.trim_start().starts_with("adc")),
        "the address is summed each trip:\n{text}"
    );
}
