target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx27.2.0"

declare i32 @g(ptr)
declare i64 @g64(ptr)
declare i8 @g8(ptr)
declare void @one()
declare void @two()

; The result of a call is tested where the call left it.
define i32 @int(ptr %s) {
  %r = call i32 @g(ptr %s)
  %z = icmp eq i32 %r, 0
  br i1 %z, label %a, label %b
a:
  call void @one()
  ret i32 1
b:
  call void @two()
  ret i32 2
}

define i32 @long(ptr %s) {
  %r = call i64 @g64(ptr %s)
  %z = icmp ne i64 %r, 0
  br i1 %z, label %a, label %b
a:
  call void @one()
  ret i32 1
b:
  call void @two()
  ret i32 2
}

; A byte has garbage above it: it is cleared before the test.
define i32 @byte(ptr %s) {
  %r = call i8 @g8(ptr %s)
  %z = icmp eq i8 %r, 0
  br i1 %z, label %a, label %b
a:
  call void @one()
  ret i32 1
b:
  call void @two()
  ret i32 2
}
