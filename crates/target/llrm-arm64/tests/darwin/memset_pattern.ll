; llvm.experimental.memset.pattern (clang's loop-idiom fill of a repeated 16-bit cell) is a store loop, not a refusal.
source_filename = "memset_pattern.c"
target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx10.4.0"

@.str = private unnamed_addr constant [7 x i8] c"%d %d\0A\00", align 1

define i32 @main() {
  %a = alloca [8 x i16], align 2
  call void @llvm.experimental.memset.pattern.p0.i16.i64(ptr %a, i16 4660, i64 8, i1 false)
  %p0 = getelementptr inbounds i16, ptr %a, i64 0
  %p7 = getelementptr inbounds i16, ptr %a, i64 7
  %v0 = load i16, ptr %p0, align 2
  %v7 = load i16, ptr %p7, align 2
  %z0 = zext i16 %v0 to i32
  %z7 = zext i16 %v7 to i32
  %r = call i32 (ptr, ...) @printf(ptr @.str, i32 %z0, i32 %z7)
  ret i32 0
}

declare void @llvm.experimental.memset.pattern.p0.i16.i64(ptr, i16, i64, i1)
declare i32 @printf(ptr, ...)
