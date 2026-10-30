target datalayout = "e-p:32:32"
; RUN: opt < %s -disable-output "-passes=print<da>" -da-enable-dependence-test=all 2>&1 \
; RUN:     | FileCheck %s --check-prefix=CHECK-ALL
; RUN: opt < %s -disable-output "-passes=print<da>" -da-enable-dependence-test=strong-siv 2>&1 \
; RUN:     | FileCheck %s --check-prefix=CHECK-STRONG-SIV
; RUN: opt < %s -disable-output "-passes=print<da>" -da-enable-dependence-test=weak-crossing-siv 2>&1 \
; RUN:     | FileCheck %s --check-prefix=CHECK-WEAK-CROSSING-SIV
; RUN: opt < %s -disable-output "-passes=print<da>" -da-enable-dependence-test=exact-siv 2>&1 \
; RUN:     | FileCheck %s --check-prefix=CHECK-EXACT-SIV
; RUN: opt < %s -disable-output "-passes=print<da>" -da-enable-dependence-test=weak-zero-siv 2>&1 \
; RUN:     | FileCheck %s --check-prefix=CHECK-WEAK-ZERO-SIV
; RUN: opt < %s -disable-output "-passes=print<da>" -da-enable-dependence-test=exact-rdiv 2>&1 \
; RUN:     | FileCheck %s --check-prefix=CHECK-EXACT-RDIV
; RUN: opt < %s -disable-output "-passes=print<da>" -da-enable-dependence-test=gcd-miv 2>&1 \
; RUN:     | FileCheck %s --check-prefix=CHECK-GCD-MIV
; RUN: opt < %s -disable-output "-passes=print<da>" -da-enable-dependence-test=banerjee-miv 2>&1 \
; RUN:     | FileCheck %s --check-prefix=CHECK-BANERJEE-MIV

; for (i = 0; i < 10; i++)
;   a[i] = 0;
define void @strong_siv(ptr %a) {
;
;
;
;
;
;
;
;
entry:
  br label %loop

loop:
  %i = phi i32 [ 0, %entry ], [ %i.inc, %loop ]
  %gep = getelementptr i8, ptr %a, i32 %i
  store i8 0, ptr %gep
  %i.inc = add i32 %i, 1
  %ec = icmp eq i32 %i.inc, 10
  br i1 %ec, label %exit, label %loop

exit:
  ret void
}

; for (i = 0; i < 10; i++) {
;   a[i] = 1;
;   a[10 - i] = 2;
; }
define void @weak_crossing_siv(ptr %a) {
;
;
;
;
;
;
;
;
entry:
  br label %loop

loop:
  %i = phi i32 [ 0, %entry ], [ %i.inc, %loop ]
  %i.sub10 = sub i32 10, %i
  %gep.0 = getelementptr i8, ptr %a, i32 %i
  %gep.1 = getelementptr i8, ptr %a, i32 %i.sub10
  store i8 1, ptr %gep.0
  store i8 2, ptr %gep.1
  %i.inc = add i32 %i, 1
  %ec = icmp eq i32 %i.inc, 10
  br i1 %ec, label %exit, label %loop

exit:
  ret void
}

; for (i = 0; i < 10; i++) {
;   a[2 * i + 0] = 1;
;   a[2 * i + 1] = 2;
; }
define void @exact_siv(ptr %a) {
;
;
;
;
;
;
;
;
entry:
  br label %loop

loop:
  %i = phi i32 [ 0, %entry ], [ %i.inc, %loop ]
  %offset.0 = mul i32 2, %i
  %offset.1 = add i32 %offset.0, 1
  %gep.0 = getelementptr i8, ptr %a, i32 %offset.0
  %gep.1 = getelementptr i8, ptr %a, i32 %offset.1
  store i8 1, ptr %gep.0
  store i8 2, ptr %gep.1
  %i.inc = add i32 %i, 1
  %ec = icmp eq i32 %i.inc, 10
  br i1 %ec, label %exit, label %loop

exit:
  ret void
}

; for (i = 0; i < 10; i++) {
;   a[10] = 1;
;   a[i] = 2;
; }
define void @weak_zero_siv(ptr %a) {
;
;
;
;
;
;
;
;
entry:
  br label %loop

loop:
  %i = phi i32 [ 0, %entry ], [ %i.inc, %loop ]
  %gep.0 = getelementptr i8, ptr %a, i32 10
  %gep.1 = getelementptr i8, ptr %a, i32 %i
  store i8 1, ptr %gep.0
  store i8 2, ptr %gep.1
  %i.inc = add i32 %i, 1
  %ec = icmp eq i32 %i.inc, 10
  br i1 %ec, label %exit, label %loop

exit:
  ret void
}

; for (i = 0; i < 10; i++)
;   a[2 * i + 0] = 1;
; for (i = 0; i < 20; i++)
;   a[2 * i + 1] = 2;
define void @exact_rdiv(ptr %a) {
;
;
;
;
;
;
;
;
entry:
  br label %loop.0

loop.0:
  %i.0 = phi i32 [ 0, %entry ], [ %i.0.inc, %loop.0 ]
  %offset.0 = phi i32 [ 0, %entry ], [ %offset.0.next, %loop.0 ]
  %gep.0 = getelementptr i8, ptr %a, i32 %offset.0
  store i8 1, ptr %gep.0
  %i.0.inc = add i32 %i.0, 1
  %offset.0.next = add i32 %offset.0, 2
  %ec.0 = icmp eq i32 %i.0.inc, 10
  br i1 %ec.0, label %loop.1, label %loop.0

loop.1:
  %i.1 = phi i32 [ 0, %loop.0 ], [ %i.1.inc, %loop.1 ]
  %offset.1 = phi i32 [ 1, %loop.0 ], [ %offset.1.next, %loop.1 ]
  %gep.1 = getelementptr i8, ptr %a, i32 %offset.1
  store i8 2, ptr %gep.1
  %i.1.inc = add i32 %i.1, 1
  %offset.1.next = add i32 %offset.1, 2
  %ec.1 = icmp eq i32 %i.1.inc, 20
  br i1 %ec.1, label %exit, label %loop.1

exit:
  ret void
}

; for (i = 0; i < 10; i++)
;   a[i + 1] = 1;
; for (i = 0; i < 20; i++)
;   a[-i] = 2;
define void @symbolic_rdiv(ptr %a) {
;
;
;
;
;
;
;
;
entry:
  br label %loop.0

loop.0:
  %i.0 = phi i32 [ 0, %entry ], [ %i.0.inc, %loop.0 ]
  %i.0.inc = add i32 %i.0, 1
  %gep.0 = getelementptr i8, ptr %a, i32 %i.0.inc
  store i8 1, ptr %gep.0
  %ec.0 = icmp eq i32 %i.0.inc, 10
  br i1 %ec.0, label %loop.1, label %loop.0

loop.1:
  %i.1 = phi i32 [ 0, %loop.0 ], [ %i.1.inc, %loop.1 ]
  %i.1.neg = sub i32 0, %i.1
  %gep.1 = getelementptr i8, ptr %a, i32 %i.1.neg
  store i8 2, ptr %gep.1
  %i.1.inc = add i32 %i.1, 1
  %ec.1 = icmp eq i32 %i.1.inc, 20
  br i1 %ec.1, label %exit, label %loop.1

exit:
  ret void
}

; for (i = 0; i < 10; i++)
;   for (j = 0; j < 10; j++) {
;     a[2*i + 2*j + 0] = 1;
;     a[2*i + 2*j + 1] = 2;
;   }
define void @gcd_miv(ptr %a) {
;
;
;
;
;
;
;
;
entry:
  br label %loop.i.header

loop.i.header:
  %i = phi i32 [ 0, %entry ], [ %i.inc, %loop.i.latch ]
  %offset.0.i = phi i32 [ 0, %entry ], [ %offset.0.i.next, %loop.i.latch ]
  %offset.1.i = phi i32 [ 1, %entry ], [ %offset.1.i.next, %loop.i.latch ]
  br label %loop.j

loop.j:
  %j = phi i32 [ 0, %loop.i.header ], [ %j.inc, %loop.j ]
  %offset.0 = phi i32 [ %offset.0.i, %loop.i.header ], [ %offset.0.next, %loop.j ]
  %offset.1 = phi i32 [ %offset.1.i, %loop.i.header ], [ %offset.1.next, %loop.j ]
  %gep.0 = getelementptr i8, ptr %a, i32 %offset.0
  %gep.1 = getelementptr i8, ptr %a, i32 %offset.1
  store i8 1, ptr %gep.0
  store i8 2, ptr %gep.1
  %j.inc = add i32 %j, 1
  %offset.0.next = add i32 %offset.0, 2
  %offset.1.next = add i32 %offset.1, 2
  %ec.j = icmp eq i32 %j.inc, 10
  br i1 %ec.j, label %loop.i.latch, label %loop.j

loop.i.latch:
  %i.inc = add i32 %i, 1
  %offset.0.i.next = add i32 %offset.0.i, 2
  %offset.1.i.next = add i32 %offset.1.i, 2
  %ec.i = icmp eq i32 %i.inc, 10
  br i1 %ec.i, label %exit, label %loop.i.header

exit:
  ret void
}

; for (i = 0; i < 10; i++)
;   for (j = 0; j < 10; j++) {
;     a[i + j] = 1;
;     a[i + j + 100] = 2;
;   }
define void @banerjee_miv(ptr %a) {
;
;
;
;
;
;
;
;
entry:
  br label %loop.i.header

loop.i.header:
  %i = phi i32 [ 0, %entry ], [ %i.inc, %loop.i.latch ]
  br label %loop.j

loop.j:
  %j = phi i32 [ 0, %loop.i.header ], [ %j.inc, %loop.j ]
  %offset.0 = add i32 %i, %j
  %offset.1 = add i32 %offset.0, 100
  %gep.0 = getelementptr i8, ptr %a, i32 %offset.0
  %gep.1 = getelementptr i8, ptr %a, i32 %offset.1
  store i8 1, ptr %gep.0
  store i8 2, ptr %gep.1
  %j.inc = add i32 %j, 1
  %ec.j = icmp eq i32 %j.inc, 10
  br i1 %ec.j, label %loop.i.latch, label %loop.j

loop.i.latch:
  %i.inc = add i32 %i, 1
  %ec.i = icmp eq i32 %i.inc, 10
  br i1 %ec.i, label %exit, label %loop.i.header

exit:
  ret void
}