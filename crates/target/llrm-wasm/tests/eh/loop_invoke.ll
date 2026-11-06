declare i32 @g(i32)
declare i32 @p(...)
define i32 @f(i32 %n) personality ptr @p {
entry:
  br label %head
head:
  %i = phi i32 [ 0, %entry ], [ %next, %body ]
  %c = icmp slt i32 %i, %n
  br i1 %c, label %body, label %done
body:
  %next = invoke i32 @g(i32 %i) to label %head unwind label %pad
done:
  ret i32 %i
pad:
  %l = landingpad { ptr, i32 } catch ptr null
  ret i32 -1
}
