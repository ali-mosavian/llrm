; llvm.objectsize of a pointer nothing folded is "unknown": -1, or 0 when the minimum is asked (macOS's _FORTIFY_SOURCE
; headers leave these in unoptimized IR).
source_filename = "objectsize.c"
target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx10.4.0"

@.str = private unnamed_addr constant [12 x i8] c"%ld %ld %d\0A\00", align 1
@g = internal global i32 9, align 4

define i32 @main() {
  %q = load volatile ptr, ptr @gp, align 8
  %a = call i64 @llvm.objectsize.i64.p0(ptr %q, i1 false, i1 true, i1 false)
  %b = call i64 @llvm.objectsize.i64.p0(ptr %q, i1 true, i1 true, i1 false)
  %c = call i32 @llvm.objectsize.i32.p0(ptr %q, i1 false, i1 true, i1 false)
  %p = call i32 (ptr, ...) @printf(ptr @.str, i64 %a, i64 %b, i32 %c)
  ret i32 0
}

@gp = internal global ptr @g, align 8

declare i64 @llvm.objectsize.i64.p0(ptr, i1, i1, i1)
declare i32 @llvm.objectsize.i32.p0(ptr, i1, i1, i1)
declare i32 @printf(ptr, ...)
