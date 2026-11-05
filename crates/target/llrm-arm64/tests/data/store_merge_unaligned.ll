target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"
define void @f() {
  %q = alloca [40 x i16], align 8
  %g0 = getelementptr inbounds i16, ptr %q, i64 2
  store i16 1, ptr %g0, align 2
  %g1 = getelementptr inbounds i16, ptr %q, i64 3
  store i16 2, ptr %g1, align 2
  %g2 = getelementptr inbounds i16, ptr %q, i64 4
  store i16 3, ptr %g2, align 2
  %g3 = getelementptr inbounds i16, ptr %q, i64 5
  store i16 1, ptr %g3, align 2
  %g4 = getelementptr inbounds i16, ptr %q, i64 6
  store i16 2, ptr %g4, align 2
  %g5 = getelementptr inbounds i16, ptr %q, i64 7
  store i16 3, ptr %g5, align 2
  %g6 = getelementptr inbounds i16, ptr %q, i64 8
  store i16 1, ptr %g6, align 2
  %g7 = getelementptr inbounds i16, ptr %q, i64 9
  store i16 2, ptr %g7, align 2
  call void @use(ptr %q)
  ret void
}
declare void @use(ptr)
