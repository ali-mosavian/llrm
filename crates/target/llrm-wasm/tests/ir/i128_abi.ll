target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32-unknown-unknown"
define i128 @add(i128 %a, i128 %b) { %r = add i128 %a, %b
 ret i128 %r }
define i32 @cmp(i128 %a, i128 %b) { %r = icmp ult i128 %a, %b
 %z = zext i1 %r to i32
 ret i32 %z }
define i128 @shl(i128 %a, i128 %b) { %r = shl i128 %a, %b
 ret i128 %r }
