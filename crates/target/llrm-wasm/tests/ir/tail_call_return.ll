target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"

declare i32 @even(i32, i32)

; The result of a `tail` call is what the function returns on one path, `acc`
; on the other: one `ret` after a phi.
define i32 @odd(i32 %n, i32 %acc) {
entry:
  %z = icmp eq i32 %n, 0
  br i1 %z, label %done, label %rec

rec:
  %a = add nsw i32 %n, -1
  %b = add nsw i32 %acc, 3
  %r = tail call i32 @even(i32 %a, i32 %b)
  br label %done

done:
  %p = phi i32 [ %r, %rec ], [ %acc, %entry ]
  ret i32 %p
}
