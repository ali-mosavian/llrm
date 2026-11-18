target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"

declare void @sink(ptr, ptr)

; a[j] = x + j + 1 interleaved with stores to another object (matmul's a and b).
define void @f(i16 %x) {
entry:
  %a = alloca [8 x i16], align 16
  %b = alloca [8 x i16], align 16
  %v0 = add i16 %x, 1
  %pa0 = getelementptr inbounds i16, ptr %a, i64 0
  store i16 %v0, ptr %pa0, align 2
  %pb0 = getelementptr inbounds i16, ptr %b, i64 0
  store i16 0, ptr %pb0, align 2
  %v1 = add i16 %x, 2
  %pa1 = getelementptr inbounds i16, ptr %a, i64 1
  store i16 %v1, ptr %pa1, align 2
  %pb1 = getelementptr inbounds i16, ptr %b, i64 1
  store i16 1, ptr %pb1, align 2
  %v2 = add i16 %x, 3
  %pa2 = getelementptr inbounds i16, ptr %a, i64 2
  store i16 %v2, ptr %pa2, align 2
  %pb2 = getelementptr inbounds i16, ptr %b, i64 2
  store i16 2, ptr %pb2, align 2
  %v3 = add i16 %x, 4
  %pa3 = getelementptr inbounds i16, ptr %a, i64 3
  store i16 %v3, ptr %pa3, align 2
  %pb3 = getelementptr inbounds i16, ptr %b, i64 3
  store i16 0, ptr %pb3, align 2
  %v4 = add i16 %x, 5
  %pa4 = getelementptr inbounds i16, ptr %a, i64 4
  store i16 %v4, ptr %pa4, align 2
  %pb4 = getelementptr inbounds i16, ptr %b, i64 4
  store i16 1, ptr %pb4, align 2
  %v5 = add i16 %x, 6
  %pa5 = getelementptr inbounds i16, ptr %a, i64 5
  store i16 %v5, ptr %pa5, align 2
  %pb5 = getelementptr inbounds i16, ptr %b, i64 5
  store i16 2, ptr %pb5, align 2
  %v6 = add i16 %x, 7
  %pa6 = getelementptr inbounds i16, ptr %a, i64 6
  store i16 %v6, ptr %pa6, align 2
  %pb6 = getelementptr inbounds i16, ptr %b, i64 6
  store i16 0, ptr %pb6, align 2
  %v7 = add i16 %x, 8
  %pa7 = getelementptr inbounds i16, ptr %a, i64 7
  store i16 %v7, ptr %pa7, align 2
  %pb7 = getelementptr inbounds i16, ptr %b, i64 7
  store i16 1, ptr %pb7, align 2
  call void @sink(ptr %a, ptr %b)
  ret void
}
