; RUN: llrm-llc -Os -march=i486 --isel=dag -S -o /dev/stdout %s
; lru nib `prev >= 0 || nxt >= 0 || ...`: the second test was `cmp ax, -1` / `jg` (3 B), the branch inversion having made
; `sgt x,-1` a `sle x,-1` that only the sign test (`test ax, ax` / `js`) catches. The pin canonicalizes it to `slt x,0`.
target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

; CHECK-LABEL: _lru_use
; CHECK-NOT: cmp ax, -1
define cc 1005 void @lru_use(ptr addrspace(5) nocapture nofree %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" optsize memory(readwrite, inaccessiblemem: none) willreturn nounwind nofree nosync norecurse {
b1:
  %1 = getelementptr inbounds nuw i8, ptr addrspace(5) %0, i16 16
  %2 = load i32, ptr addrspace(5) %1
  %3 = add i32 %2, 1
  store i32 %3, ptr addrspace(5) %1
  %4 = getelementptr inbounds nuw i8, ptr addrspace(5) %0, i16 12
  %5 = load ptr addrspace(1), ptr addrspace(5) %4
  %6 = getelementptr i8, ptr addrspace(1) %5, i16 4
  store i32 %3, ptr addrspace(1) %6
  %7 = getelementptr inbounds nuw i8, ptr addrspace(5) %0, i16 4
  %8 = load ptr addrspace(1), ptr addrspace(5) %7
  %9 = getelementptr i8, ptr addrspace(1) %8, i16 2
  %10 = load i16, ptr addrspace(1) %9
  %11 = getelementptr inbounds nuw i8, ptr addrspace(5) %0, i16 8
  %12 = load ptr addrspace(1), ptr addrspace(5) %11
  %13 = getelementptr i8, ptr addrspace(1) %12, i16 2
  %14 = load i16, ptr addrspace(1) %13
  %15 = icmp sgt i16 %10, -1
  %16 = icmp sgt i16 %14, -1
  %or.cond = select i1 %15, i1 true, i1 %16
  br i1 %or.cond, label %common.ret, label %b8

common.ret:
  ret void

b7:
  %17 = getelementptr inbounds nuw i8, ptr addrspace(5) %0, i16 34
  %18 = getelementptr inbounds i8, ptr addrspace(5) %17, i16 %27
  %19 = load i16, ptr addrspace(5) %18
  %20 = getelementptr i8, ptr addrspace(1) %8, i16 2
  store i16 %19, ptr addrspace(1) %20
  %21 = getelementptr i8, ptr addrspace(1) %12, i16 2
  store i16 -1, ptr addrspace(1) %21
  %22 = icmp sgt i16 %19, -1
  br i1 %22, label %b10, label %b11

b8:
  %23 = load ptr addrspace(1), ptr addrspace(5) %0
  %24 = getelementptr i8, ptr addrspace(1) %23, i16 2
  %25 = load i16, ptr addrspace(1) %24
  %26 = getelementptr inbounds nuw i8, ptr addrspace(5) %0, i16 20
  %27 = shl i16 %25, 1
  %28 = getelementptr inbounds i8, ptr addrspace(5) %26, i16 %27
  %29 = load i16, ptr addrspace(5) %28
  %30 = icmp eq i16 %29, 1
  br i1 %30, label %common.ret, label %b7

b10:
  %31 = shl nuw i16 %19, 1
  %32 = getelementptr i8, ptr addrspace(1) %12, i16 %31
  store i16 1, ptr addrspace(1) %32
  br label %b12

b11:
  %33 = getelementptr i8, ptr addrspace(5) %0, i16 %27
  %34 = getelementptr i8, ptr addrspace(5) %33, i16 20
  store i16 1, ptr addrspace(5) %34
  br label %b12

b12:
  %35 = getelementptr i8, ptr addrspace(5) %0, i16 %27
  %36 = getelementptr i8, ptr addrspace(5) %35, i16 34
  store i16 1, ptr addrspace(5) %36
  br label %common.ret
}
