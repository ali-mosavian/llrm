target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"

define i32 @once(i16 %a, i16 %b) {
  %x = zext i16 %a to i32
  %y = zext i16 %b to i32
  %p = mul i32 %x, %y
  ret i32 %p
}

define i32 @looped(ptr %v, i16 %n) {
entry:
  br label %loop

loop:
  %i = phi i16 [ 0, %entry ], [ %next, %loop ]
  %sum = phi i32 [ 0, %entry ], [ %add, %loop ]
  %p = getelementptr i16, ptr %v, i16 %i
  %w = load i16, ptr %p
  %x = zext i16 %w to i32
  %y = zext i16 %i to i32
  %m = mul i32 %x, %y
  %add = add i32 %sum, %m
  %next = add i16 %i, 1
  %done = icmp eq i16 %next, %n
  br i1 %done, label %exit, label %loop

exit:
  ret i32 %add
}
