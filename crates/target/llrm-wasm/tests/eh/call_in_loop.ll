declare void @g(i32)
declare i32 @p(...)
define i32 @f(i32 %n) personality ptr @p {
entry:
  br label %loop
loop:
  %i = phi i32 [ 0, %entry ], [ %j, %body ]
  call void @g(i32 %i)
  invoke void @g(i32 1) to label %body unwind label %pad
body:
  call void @g(i32 2)
  %j = add i32 %i, 1
  %c = icmp slt i32 %j, %n
  br i1 %c, label %loop, label %done
done:
  ret i32 1
pad:
  %l = landingpad { ptr, i32 } catch ptr null
  ret i32 2
}
