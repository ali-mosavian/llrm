target datalayout = "e-m:e-p:32:32-f64:32:64-f80:32-n8:16:32-S128"
target triple = "i486-unknown-linux-gnu"

@str = private unnamed_addr constant [12 x i8] c"hello world\00"
@tab = private unnamed_addr constant [4 x i32] [i32 1, i32 2, i32 3, i32 4]
@zero = private unnamed_addr constant [8 x i8] zeroinitializer
@pair = private unnamed_addr constant { i32, i32 } { i32 5, i32 6 }
@dbl = private unnamed_addr constant [2 x double] [double 1.5, double -2.0]
@ptrs = private unnamed_addr constant [2 x ptr] [ptr @str, ptr @tab]
@var = global [8 x i8] c"abcdefg\00"
declare void @llvm.memcpy.p0.p0.i32(ptr, ptr, i32, i1)

define void @from_string(ptr %d) {
  call void @llvm.memcpy.p0.p0.i32(ptr %d, ptr @str, i32 12, i1 false)
  ret void
}

define void @from_table(ptr %d) {
  call void @llvm.memcpy.p0.p0.i32(ptr %d, ptr @tab, i32 16, i1 false)
  ret void
}

define void @odd_size(ptr %d) {
  call void @llvm.memcpy.p0.p0.i32(ptr %d, ptr @str, i32 7, i1 false)
  ret void
}

define void @from_offset(ptr %d) {
  %p = getelementptr inbounds i8, ptr @str, i32 3
  call void @llvm.memcpy.p0.p0.i32(ptr %d, ptr %p, i32 8, i1 false)
  ret void
}

define void @from_zero(ptr %d) {
  call void @llvm.memcpy.p0.p0.i32(ptr %d, ptr @zero, i32 8, i1 false)
  ret void
}

define void @from_struct(ptr %d) {
  call void @llvm.memcpy.p0.p0.i32(ptr %d, ptr @pair, i32 8, i1 false)
  ret void
}

define void @from_doubles(ptr %d) {
  call void @llvm.memcpy.p0.p0.i32(ptr %d, ptr @dbl, i32 16, i1 false)
  ret void
}

define void @from_variable(ptr %d) {
  call void @llvm.memcpy.p0.p0.i32(ptr %d, ptr @var, i32 8, i1 false)
  ret void
}

define void @from_pointers(ptr %d) {
  call void @llvm.memcpy.p0.p0.i32(ptr %d, ptr @ptrs, i32 8, i1 false)
  ret void
}
