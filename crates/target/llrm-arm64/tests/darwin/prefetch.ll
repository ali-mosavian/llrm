; llvm.prefetch (__builtin_prefetch) is a prfm hint, not a refusal; the program's result does not depend on it.
source_filename = "prefetch.c"
target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx10.4.0"

@.str = private unnamed_addr constant [4 x i8] c"%d\0A\00", align 1
@table = internal global [4 x i32] [i32 3, i32 4, i32 5, i32 6], align 4

define i32 @main() {
  %p = getelementptr inbounds i32, ptr @table, i64 1
  call void @llvm.prefetch.p0(ptr %p, i32 0, i32 3, i32 1)
  call void @llvm.prefetch.p0(ptr %p, i32 1, i32 0, i32 1)
  %v = load i32, ptr %p, align 4
  %r = call i32 (ptr, ...) @printf(ptr @.str, i32 %v)
  ret i32 0
}

declare void @llvm.prefetch.p0(ptr, i32, i32, i32)
declare i32 @printf(ptr, ...)
