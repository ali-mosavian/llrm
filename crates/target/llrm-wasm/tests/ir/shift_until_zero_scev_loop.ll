target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32-unknown-wasi"

; for (iv = start; (val >> (iv + off)) != 0; iv++);  returns iv.
define i32 @offset_shift(i32 %val, i32 %start, i32 %off) {
entry:
  br label %loop

loop:
  %iv = phi i32 [ %start, %entry ], [ %iv.next, %loop ]
  %nbits = add nsw i32 %iv, %off
  %shifted = lshr i32 %val, %nbits
  %iszero = icmp eq i32 %shifted, 0
  %iv.next = add i32 %iv, 1
  br i1 %iszero, label %end, label %loop

end:
  ret i32 %iv
}

; for (iv = 0; (val << iv) != 0; iv++);  returns iv.
define i32 @plain_shl(i32 %val) {
entry:
  br label %loop

loop:
  %iv = phi i32 [ 0, %entry ], [ %iv.next, %loop ]
  %shifted = shl i32 %val, %iv
  %iszero = icmp eq i32 %shifted, 0
  %iv.next = add i32 %iv, 1
  br i1 %iszero, label %end, label %loop

end:
  ret i32 %iv
}
