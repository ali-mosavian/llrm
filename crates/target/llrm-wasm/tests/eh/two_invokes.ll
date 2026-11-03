declare void @g(i32)
declare i32 @p(...)
define i32 @f(i32 %a) personality ptr @p {
entry:
  invoke void @g(i32 %a) to label %b unwind label %pad
b:
  invoke void @g(i32 1) to label %c unwind label %pad
c:
  call void @g(i32 2)
  ret i32 1
pad:
  %l = landingpad { ptr, i32 } catch ptr null
  ret i32 2
}
