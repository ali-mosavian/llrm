declare void @g(i32)
declare i32 @p(...)
@slot = global i32 0
define i32 @f(i32 %n) personality ptr @p {
entry:
  invoke void @g(i32 1) to label %s1 unwind label %pad
s1:
  invoke void @g(i32 2) to label %s2 unwind label %pad
s2:
  ret i32 7
pad:
  %l = landingpad { ptr, i32 } catch ptr null
  %k = load i32, ptr @slot
  switch i32 %k, label %s2 [
    i32 1, label %s1
  ]
}
