target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx10.4.0"

define i64 @accumulate(i64 %acc, i32 %a, i32 %b) {
  %p = mul nsw i32 %a, %b
  %w = sext i32 %p to i64
  %r = add i64 %acc, %w
  ret i64 %r
}
define i64 @subtract(i64 %acc, i32 %a, i32 %b) {
  %p = mul nsw i32 %a, %b
  %w = sext i32 %p to i64
  %r = sub i64 %acc, %w
  ret i64 %r
}
define i64 @alone(i32 %a, i32 %b) {
  %p = mul nsw i32 %a, %b
  %w = sext i32 %p to i64
  ret i64 %w
}
define i64 @unsigned_accumulate(i64 %acc, i32 %a, i32 %b) {
  %p = mul nuw i32 %a, %b
  %w = zext i32 %p to i64
  %r = add i64 %acc, %w
  ret i64 %r
}
define i64 @may_wrap(i64 %acc, i32 %a, i32 %b) {
  %p = mul i32 %a, %b
  %w = sext i32 %p to i64
  %r = add i64 %acc, %w
  ret i64 %r
}
