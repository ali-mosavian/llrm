declare void @g(i32)
declare i32 @p(...)
define i32 @f(i32 %a) personality ptr @p {
entry:
  invoke void @g(i32 %a) to label %ok unwind label %pad
ok:
  ret i32 1
pad:
  %l = landingpad { ptr, i32 } catch ptr null
  ret i32 2
}
