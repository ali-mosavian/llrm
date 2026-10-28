target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx"

@.hi = private unnamed_addr constant [4 x i8] c"hi\0A\00"
@.c = private unnamed_addr constant [3 x i8] c"%c\00"
@.nl = private unnamed_addr constant [2 x i8] c"\0A\00"

declare i32 @printf(ptr, ...)

define void @f(i32 %x) {
  %a = call i32 (ptr, ...) @printf(ptr @.hi)
  %b = call i32 (ptr, ...) @printf(ptr @.c, i32 %x)
  %c = call i32 (ptr, ...) @printf(ptr @.nl)
  ret void
}
