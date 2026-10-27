target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx"

@.hi = private unnamed_addr constant [3 x i8] c"hi\00"
@.c = private unnamed_addr constant [3 x i8] c"%c\00"
@.s = private unnamed_addr constant [3 x i8] c"%s\00"
@.d = private unnamed_addr constant [3 x i8] c"%d\00"

declare i32 @fprintf(ptr, ptr, ...)
declare i32 @fputs(ptr, ptr)
declare i64 @fwrite(ptr, i64, i64, ptr)

define void @f(ptr %out, i32 %x, ptr %text) {
  %a = call i32 (ptr, ptr, ...) @fprintf(ptr %out, ptr @.hi)
  %b = call i32 (ptr, ptr, ...) @fprintf(ptr %out, ptr @.c, i32 %x)
  %c = call i32 (ptr, ptr, ...) @fprintf(ptr %out, ptr @.s, ptr %text)
  %d = call i32 @fputs(ptr @.hi, ptr %out)
  %e = call i64 @fwrite(ptr %text, i64 1, i64 1, ptr %out)
  %g = call i32 (ptr, ptr, ...) @fprintf(ptr %out, ptr @.d, i32 %x)
  ret void
}
