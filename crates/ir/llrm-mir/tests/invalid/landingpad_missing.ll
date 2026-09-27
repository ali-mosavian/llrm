; invalid: which does not begin with a landingpad
declare i32 @personality(...)
declare void @g()
define void @f() personality ptr @personality {
entry:
  invoke void @g() to label %ok unwind label %bad
ok:
  ret void
bad:
  ret void
}
