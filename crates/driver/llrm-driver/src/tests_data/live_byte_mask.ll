target datalayout = "@m32-datalayout@"

; x_adler's shape: the low byte of a counter that stays live, added each trip.
define dso_local i32 @sum_low_bytes(i32 noundef %n) #0 {
entry:
  %any = icmp sgt i32 %n, 0
  br i1 %any, label %loop, label %done

loop:
  %i = phi i32 [ 7, %entry ], [ %next, %loop ]
  %left = phi i32 [ %n, %entry ], [ %less, %loop ]
  %acc = phi i32 [ 0, %entry ], [ %sum, %loop ]
  %low = and i32 %i, 255
  %sum = add i32 %acc, %low
  %next = add i32 %i, 31
  %less = add i32 %left, -1
  %again = icmp ne i32 %less, 0
  br i1 %again, label %loop, label %done

done:
  %out = phi i32 [ 0, %entry ], [ %sum, %loop ]
  ret i32 %out
}

attributes #0 = { nounwind }
