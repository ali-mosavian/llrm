target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx"
define i32 @big(i32 %a) { %c = icmp slt i32 %a, 4097
 %r = zext i1 %c to i32
 ret i32 %r }
