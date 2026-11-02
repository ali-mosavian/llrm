declare void @g(i32)
declare i32 @p(...)
define i32 @f(i32 %a) personality ptr @p {
entry:
  invoke void @g(i32 %a) to label %b unwind label %pad1
b:
  invoke void @g(i32 2) to label %c unwind label %pad2
c:
  ret i32 1
pad1:
  %l1 = landingpad { ptr, i32 } catch ptr null
  ret i32 2
pad2:
  %l2 = landingpad { ptr, i32 } catch ptr null
  ret i32 3
}
