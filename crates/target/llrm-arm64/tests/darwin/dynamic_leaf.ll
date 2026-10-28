; A function with a dynamic alloca and no call still needs a frame record: its slots are addressed from x29.
; At -O0 nothing inlines it (tests/llc/vla at -O0 panicked: "a moving stack has a frame record").
source_filename = "dynamic_leaf.c"
target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx10.4.0"

@.str = private unnamed_addr constant [4 x i8] c"%d\0A\00", align 1

define internal i32 @leaf(i32 %n) {
  %m = zext i32 %n to i64
  %a = alloca i32, i64 %m, align 16
  store i32 7, ptr %a, align 4
  %v = load i32, ptr %a, align 4
  %r = add i32 %v, %n
  ret i32 %r
}

define i32 @main() {
  %v = call i32 @leaf(i32 5)
  %p = call i32 (ptr, ...) @printf(ptr @.str, i32 %v)
  ret i32 0
}

declare i32 @printf(ptr, ...)
