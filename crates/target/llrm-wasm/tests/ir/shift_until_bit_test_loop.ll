target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32-unknown-wasi"

; while (!(x & (1 << pos))) x <<= 1;   returns the shifted values.
define i32 @variable_bit(i32 %x, i32 %pos) {
entry:
  %mask = shl i32 1, %pos
  br label %loop

loop:
  %xc = phi i32 [ %x, %entry ], [ %xn, %loop ]
  %m = and i32 %xc, %mask
  %unset = icmp eq i32 %m, 0
  %xn = shl i32 %xc, 1
  br i1 %unset, label %loop, label %end

end:
  %s = add i32 %xc, %xn
  ret i32 %s
}

; while (!(x & 256)) x <<= 1;   the branch leaves on a set bit.
define i32 @constant_bit(i32 %x) {
entry:
  br label %loop

loop:
  %xc = phi i32 [ %x, %entry ], [ %xn, %loop ]
  %m = and i32 %xc, 256
  %set = icmp ne i32 %m, 0
  %xn = shl i32 %xc, 1
  br i1 %set, label %end, label %loop

end:
  %s = add i32 %xc, %xn
  ret i32 %s
}
