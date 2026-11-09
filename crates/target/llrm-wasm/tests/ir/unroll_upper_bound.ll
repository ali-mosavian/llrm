target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32"

@values = global [64 x i16] zeroinitializer, align 2

; The inner loop runs at most three times (at = i, i-16, ... while at >= 16, i < 64) and may leave earlier.
define void @pass_gap() {
entry:
  br label %outer

outer:
  %i = phi i32 [ 16, %entry ], [ %i1, %latch ]
  %vp = getelementptr inbounds i16, ptr @values, i32 %i
  %value = load i16, ptr %vp, align 2
  br label %inner

inner:
  %at = phi i32 [ %i, %outer ], [ %prev, %step ]
  %ge = icmp uge i32 %at, 16
  br i1 %ge, label %cmp, label %latch

cmp:
  %prev = sub i32 %at, 16
  %pp = getelementptr inbounds i16, ptr @values, i32 %prev
  %pv = load i16, ptr %pp, align 2
  %gt = icmp ugt i16 %pv, %value
  br i1 %gt, label %step, label %latch

step:
  %ap = getelementptr inbounds i16, ptr @values, i32 %at
  store i16 %pv, ptr %ap, align 2
  br label %inner

latch:
  %end = phi i32 [ %at, %inner ], [ %at, %cmp ]
  %ep = getelementptr inbounds i16, ptr @values, i32 %end
  store i16 %value, ptr %ep, align 2
  %i1 = add nuw nsw i32 %i, 1
  %more = icmp ult i32 %i1, 64
  br i1 %more, label %outer, label %exit

exit:
  ret void
}
