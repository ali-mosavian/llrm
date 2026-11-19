target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"

; `for (iv = start; (val >> (iv + off)) != 0; iv++)`: the mid-end puts the step `iv + 1` (a long increment through `a`
; and the flags) between the exit test and its branch.
define i32 @offset_count(i32 %val, i16 %start, i16 %off) {
entry:
  %s = sext i16 %start to i32
  %o = sext i16 %off to i32
  br label %loop
loop:
  %iv = phi i32 [ %s, %entry ], [ %next, %loop ]
  %a = add nsw i32 %iv, %o
  %sh = lshr i32 %val, %a
  %z = icmp eq i32 %sh, 0
  %next = add nsw i32 %iv, 1
  br i1 %z, label %exit, label %loop
exit:
  ret i32 %iv
}
