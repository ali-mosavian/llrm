; RUN: llrm-llc -Os -march=i486 --isel=dag -S -o /dev/stdout %s
; m16 -Os weighs bytes strictly. A 3-trip loop of constant stores became memsets, then 9 `mov [mem], imm` (72 B) against a ~42 B loop (lru bas/bq16, +33..35 B). At -O2 the rate (1 clk = 2 B) prices it instead: see constant_store_loop_is_stores_when_clocks_pay_on_m16.ll.
target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@a = global [3 x i16] zeroinitializer
@b = global [3 x i16] zeroinitializer
@c = global [3 x i32] zeroinitializer
@d = global [7 x i16] zeroinitializer

define void @init() {
entry:
  br label %loop

loop:
  %i = phi i16 [ 0, %entry ], [ %next, %loop ]
  %pa = getelementptr [3 x i16], ptr @a, i16 0, i16 %i
  store i16 0, ptr %pa
  %pb = getelementptr [3 x i16], ptr @b, i16 0, i16 %i
  store i16 -1, ptr %pb
  %pc = getelementptr [3 x i32], ptr @c, i16 0, i16 %i
  store i32 0, ptr %pc
  %pd = getelementptr [7 x i16], ptr @d, i16 0, i16 %i
  store i16 -1, ptr %pd
  %next = add nuw nsw i16 %i, 1
  %done = icmp eq i16 %next, 3
  br i1 %done, label %exit, label %loop

exit:
  ret void
}

; CHECK-LABEL: _init@
; CHECK: jne
