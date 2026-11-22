source_filename = "r_h.c"
target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"
target triple = "z80"

define dso_local range(i32 -2147483641, -2147483648) i32 @f(i32 noundef %0, i32 noundef %1, i32 noundef %2) local_unnamed_addr {
  %4 = shl nsw i32 %1, 1
  %5 = mul nsw i32 %2, 3
  %6 = add i32 %0, 126
  %7 = add i32 %6, %4
  %8 = add i32 %7, %5
  %9 = shl nsw i32 %8, 1
  %10 = mul i32 %2, 9
  %11 = mul i32 %1, 7
  %12 = mul i32 %0, 5
  %13 = add i32 %12, 45
  %14 = add i32 %13, %11
  %15 = add i32 %14, %10
  %16 = add i32 %15, %8
  %17 = add i32 %16, %9
  ret i32 %17
}



