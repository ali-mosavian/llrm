target datalayout = "@m32-datalayout@"

declare void @use(i32)

; x_int64_division's shape: the masked byte is wanted again after two calls.
define dso_local i32 @low_bytes_across_calls(i32 noundef %n) #0 {
entry:
  br label %loop

loop:
  %i = phi i32 [ 7, %entry ], [ %next, %loop ]
  %left = phi i32 [ %n, %entry ], [ %less, %loop ]
  %low = and i32 %i, 255
  call void @use(i32 %low)
  call void @use(i32 %low)
  %next = add i32 %i, 31
  %less = add i32 %left, -1
  %again = icmp ne i32 %less, 0
  br i1 %again, label %loop, label %done

done:
  ret i32 %next
}

attributes #0 = { nounwind }
