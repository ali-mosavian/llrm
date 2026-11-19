target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32"

@p = private unnamed_addr constant [2 x i8] c"p\00", align 1
@a = private unnamed_addr constant [2 x i8] c"a\00", align 1
@b = private unnamed_addr constant [2 x i8] c"b\00", align 1

declare void @report(i32)

define void @message(ptr %text) {
  %c = load i8, ptr %text, align 1
  %e = sext i8 %c to i32
  call void @report(i32 %e)
  ret void
}

; Three sites of a callee that stays: alone each copy is no smaller than the call, together the
; copies end in one `report` fed by the constants (the pin inlines all three at -Os).
define i32 @scan(i32 %m) {
entry:
  %is_p = icmp eq i32 %m, 194
  br i1 %is_p, label %p, label %not_p
p:
  call void @message(ptr @p)
  ret i32 0
not_p:
  %is_a = icmp eq i32 %m, 201
  br i1 %is_a, label %a, label %not_a
a:
  call void @message(ptr @a)
  ret i32 0
not_a:
  %is_b = icmp eq i32 %m, 193
  br i1 %is_b, label %b, label %out
b:
  call void @message(ptr @b)
  ret i32 0
out:
  ret i32 1
}
