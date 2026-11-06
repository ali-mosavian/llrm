; RUN: llrm-llc -O2 -march=i486 --isel=dag -S -o /dev/stdout %s
; m16 trades 1 clock for 2 B at speed, by the block's frequency: x*346 (a 22-clock chain of 9 steps
; against a 26-clock `imul`; the bytes it adds are not worth 4 clocks once) run once stays `imul`, inside a loop it is the chain.
target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

define i16 @once(i16 %x) {
  %r = mul i16 %x, 346
  ret i16 %r
}

define i16 @hot(ptr %p, i16 %n) {
entry:
  br label %body
body:
  %i = phi i16 [ 0, %entry ], [ %next, %body ]
  %s = phi i16 [ 0, %entry ], [ %sum, %body ]
  %a = getelementptr i16, ptr %p, i16 %i
  %v = load i16, ptr %a
  %m = mul i16 %v, 346
  %sum = add i16 %s, %m
  %next = add i16 %i, 1
  %done = icmp eq i16 %next, %n
  br i1 %done, label %out, label %body
out:
  ret i16 %sum
}

; CHECK-LABEL: _once@
; CHECK: imul
; CHECK-LABEL: _hot@
; CHECK-NOT: imul
; CHECK: shl
