declare void @g(i32)
declare i32 @__gxx_personality_v0(...)
define i32 @plain(i32 %a) personality ptr @__gxx_personality_v0 {
entry:
  invoke void @g(i32 %a) to label %ok unwind label %pad
ok:
  ret i32 1
pad:
  %l = landingpad { ptr, i32 } cleanup
  ret i32 2
}
