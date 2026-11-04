target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx10.4.0"

define i64 @add_shl(i64 %a, i64 %b) {
  %s = shl i64 %b, 3
  %r = add i64 %a, %s
  ret i64 %r
}

define i32 @sub_lshr(i32 %a, i32 %b) {
  %s = lshr i32 %b, 2
  %r = sub i32 %a, %s
  ret i32 %r
}

define i64 @eor_ashr(i64 %a, i64 %b) {
  %s = ashr i64 %b, 5
  %r = xor i64 %s, %a
  ret i64 %r
}

define i64 @shift_read_twice(i64 %a, i64 %b) {
  %s = shl i64 %b, 3
  %r = add i64 %a, %s
  %t = mul i64 %r, %s
  ret i64 %t
}

define i64 @variable_amount(i64 %a, i64 %b, i64 %n) {
  %s = shl i64 %b, %n
  %r = add i64 %a, %s
  ret i64 %r
}

define i64 @subtracted_from_the_shift(i64 %a, i64 %b) {
  %s = shl i64 %b, 3
  %r = sub i64 %s, %a
  ret i64 %r
}
