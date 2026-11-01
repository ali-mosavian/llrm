declare void @g(i32)
declare i32 @p(...)
define i32 @f(i32 %a) personality ptr @p {
entry:
  invoke void @g(i32 %a) to label %b unwind label %pad
b:
  invoke void @g(i32 2) to label %c unwind label %pad
c:
  call void @g(i32 9)
  invoke void @g(i32 5) to label %d unwind label %pad
d:
  ret i32 1
pad:
  %l = landingpad { ptr, i32 } catch ptr null
  call void @g(i32 7)
  ret i32 2
}
