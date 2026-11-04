target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx10.4.0"

declare i64 @callee(i64, i64)
declare i64 @many(i64, i64, i64, i64, i64, i64, i64, i64, i64)
declare void @note(i64)
declare void @fill(ptr sret({ i64, i64 }), i64)
declare i64 @shape(ptr byval({ i64, i64 }))
declare coldcc i64 @cold(i64)

define i64 @forwards(i64 %a) {
  %r = tail call i64 @callee(i64 %a, i64 %a)
  ret i64 %r
}

define i64 @through_a_pointer(ptr %f, i64 %a) {
  %r = tail call i64 %f(i64 %a, i64 1)
  ret i64 %r
}

define i64 @unmarked(i64 %a) {
  %r = call i64 @callee(i64 %a, i64 %a)
  ret i64 %r
}

define i64 @on_the_stack(i64 %a) {
  %r = tail call i64 @many(i64 %a, i64 %a, i64 %a, i64 %a, i64 %a, i64 %a, i64 %a, i64 %a, i64 %a)
  ret i64 %r
}

define i64 @after_a_call(i64 %a, i64 %b) {
  call void @note(i64 %a)
  %r = tail call i64 @callee(i64 %b, i64 %b)
  ret i64 %r
}

define void @sret_callee(ptr sret({ i64, i64 }) %out, i64 %a) {
  tail call void @fill(ptr sret({ i64, i64 }) %out, i64 %a)
  ret void
}

define i64 @byval_argument(ptr byval({ i64, i64 }) %p) {
  %r = tail call i64 @shape(ptr byval({ i64, i64 }) %p)
  ret i64 %r
}

define i64 @other_convention(i64 %a) {
  %r = tail call coldcc i64 @cold(i64 %a)
  ret i64 %r
}
