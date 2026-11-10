target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32-unknown-wasi"

; for (; x; x &= x - 1) n++;
define i32 @ones32(i32 %x) {
entry:
  %zero = icmp eq i32 %x, 0
  br i1 %zero, label %end, label %ph

ph:
  br label %loop

loop:
  %x1 = phi i32 [ %x, %ph ], [ %x2, %loop ]
  %n1 = phi i32 [ 0, %ph ], [ %n2, %loop ]
  %n2 = add nuw nsw i32 %n1, 1
  %m = add i32 %x1, -1
  %x2 = and i32 %x1, %m
  %done = icmp eq i32 %x2, 0
  br i1 %done, label %exit, label %loop

exit:
  br label %end

end:
  %n = phi i32 [ 0, %entry ], [ %n2, %exit ]
  ret i32 %n
}

; The same on 64 bits, the count starting at 5 and the decrement spelled `sub`.
define i32 @ones64(i64 %x) {
entry:
  %zero = icmp eq i64 %x, 0
  br i1 %zero, label %end, label %ph

ph:
  br label %loop

loop:
  %x1 = phi i64 [ %x, %ph ], [ %x2, %loop ]
  %n1 = phi i32 [ 5, %ph ], [ %n2, %loop ]
  %n2 = add nuw nsw i32 %n1, 1
  %m = sub i64 %x1, 1
  %x2 = and i64 %m, %x1
  %done = icmp eq i64 %x2, 0
  br i1 %done, label %exit, label %loop

exit:
  br label %end

end:
  %n = phi i32 [ 5, %entry ], [ %n2, %exit ]
  ret i32 %n
}
