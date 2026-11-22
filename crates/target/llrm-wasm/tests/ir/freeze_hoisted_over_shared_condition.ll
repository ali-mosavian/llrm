target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32-unknown-unknown"
declare i32 @f(i32)
define i32 @pick(i32 %n, i32 %bad0, i32 %first0, i32 %idx0) {
entry:
  br label %loop
loop:
  %i = phi i32 [ 0, %entry ], [ %i1, %loop ]
  %bad = phi i32 [ %bad0, %entry ], [ %bad2, %loop ]
  %first = phi i32 [ %first0, %entry ], [ %first2, %loop ]
  %idx = phi i32 [ %idx0, %entry ], [ %idx1, %loop ]
  %r = call i32 @f(i32 %i)
  %s = call i32 @f(i32 %r)
  %a = icmp ne i32 %s, %r
  %b = icmp eq i32 %bad, 0
  %z = zext i1 %a to i32
  %bad1 = add nsw i32 %bad, %z
  %c = select i1 %a, i1 %b, i1 false
  %first1 = select i1 %c, i32 64, i32 %first
  %t = call i32 @f(i32 %r)
  %a2 = icmp ne i32 %t, %r
  %b2 = icmp eq i32 %bad1, 0
  %z2 = zext i1 %a2 to i32
  %bad2 = add nsw i32 %bad1, %z2
  %c2 = select i1 %a2, i1 %b2, i1 false
  %first2 = select i1 %c2, i32 32, i32 %first1
  %any = select i1 %c2, i1 true, i1 %c
  %idx1 = select i1 %any, i32 %i, i32 %idx
  %i1 = add nuw nsw i32 %i, 1
  %d = icmp eq i32 %i1, %n
  br i1 %d, label %exit, label %loop
exit:
  %o = add i32 %bad2, %first2
  %p = add i32 %o, %idx1
  ret i32 %p
}
