declare void @die(i16 signext) noreturn
define i32 @f(i32 %a) {
entry:
  %c = icmp eq i32 %a, 0
  br i1 %c, label %bad, label %ok
bad:
  call void @die(i16 signext 11)
  unreachable
ok:
  ret i32 %a
}
define i32 @g(i32 %a) personality ptr @__gxx_personality_v0 {
entry:
  invoke void @die(i16 signext 11) to label %bad unwind label %pad
bad:
  unreachable
pad:
  %l = landingpad { ptr, i32 } cleanup
  ret i32 2
}
declare i32 @__gxx_personality_v0(...)
