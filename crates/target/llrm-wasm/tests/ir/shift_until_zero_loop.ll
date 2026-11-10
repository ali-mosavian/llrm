target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32-unknown-wasi"

; int n = 0; while (x >>= 1) n++; return n;
define i32 @log2_32(i32 %x) {
entry:
  br label %loop

loop:
  %x1 = phi i32 [ %x, %entry ], [ %x2, %loop ]
  %n1 = phi i32 [ 0, %entry ], [ %n2, %loop ]
  %x2 = lshr i32 %x1, 1
  %done = icmp eq i32 %x2, 0
  %n2 = add nuw nsw i32 %n1, 1
  br i1 %done, label %exit, label %loop

exit:
  %n = phi i32 [ %n1, %loop ]
  ret i32 %n
}

; while (x <<= 1) n++; return n;
define i32 @shl_count(i32 %x) {
entry:
  br label %loop

loop:
  %x1 = phi i32 [ %x, %entry ], [ %x2, %loop ]
  %n1 = phi i32 [ 0, %entry ], [ %n2, %loop ]
  %x2 = shl i32 %x1, 1
  %done = icmp eq i32 %x2, 0
  %n2 = add nuw nsw i32 %n1, 1
  br i1 %done, label %exit, label %loop

exit:
  %n = phi i32 [ %n1, %loop ]
  ret i32 %n
}

; if (!x) return 0; do { x >>= 1; n++; } while (x); return n;
; The counter after the loop is the incremented one, so the zero test above
; the loop is what keeps `ctlz` defined.
define i32 @guarded(i32 %x) {
entry:
  %zero = icmp eq i32 %x, 0
  br i1 %zero, label %end, label %ph

ph:
  br label %loop

loop:
  %x1 = phi i32 [ %x, %ph ], [ %x2, %loop ]
  %n1 = phi i32 [ 0, %ph ], [ %n2, %loop ]
  %x2 = lshr i32 %x1, 1
  %n2 = add nuw nsw i32 %n1, 1
  %done = icmp eq i32 %x2, 0
  br i1 %done, label %exit, label %loop

exit:
  br label %end

end:
  %n = phi i32 [ 0, %entry ], [ %n2, %exit ]
  ret i32 %n
}

; int n = 0; while (x >= 2) { x >>= 1; n++; } return n;  (x >= 2 is a floor log2)
define i32 @floor_log2_ult2(i32 %x) {
entry:
  br label %loop

loop:
  %x1 = phi i32 [ %x, %entry ], [ %x2, %loop ]
  %n1 = phi i32 [ 0, %entry ], [ %n2, %loop ]
  %x2 = lshr i32 %x1, 1
  %n2 = add nuw nsw i32 %n1, 1
  %lt = icmp ult i32 %x1, 2
  br i1 %lt, label %exit, label %loop

exit:
  %n = phi i32 [ %n1, %loop ]
  ret i32 %n
}

; if (x < 2) return 0; do { x >>= 1; n++; } while (x >= 4); return n;  (floor log2)
define i32 @floor_log2_ult4(i32 %x) {
entry:
  %small = icmp ult i32 %x, 2
  br i1 %small, label %end, label %ph

ph:
  br label %loop

loop:
  %x1 = phi i32 [ %x, %ph ], [ %x2, %loop ]
  %n1 = phi i32 [ 0, %ph ], [ %n2, %loop ]
  %x2 = lshr i32 %x1, 1
  %n2 = add nuw nsw i32 %n1, 1
  %lt = icmp ult i32 %x1, 4
  br i1 %lt, label %exit, label %loop

exit:
  br label %end

end:
  %n = phi i32 [ 0, %entry ], [ %n2, %exit ]
  ret i32 %n
}
