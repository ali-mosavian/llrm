target datalayout = "e-p:64:64-i64:64"

%T = type { i32, i64 }

@ga = global i32 0, align 16
@gn = global i64 0
@gl = global [40 x i8] zeroinitializer
@gw = weak global [40 x i8] zeroinitializer
@gd = external global [40 x i8]
@gu = global %T zeroinitializer, align 2

declare range(i8 2, 10) i8 @ret_range(i8)
declare i32 @ret_id(i32 returned)
declare align 32 ptr @ret_align(ptr)

define i32 @args(i32 range(i32 4, 20) %a, i8 range(i8 -4, 4) %b, ptr align 8 %p, ptr sret(%T) %s, ptr byval(i64) %v) {
  ret i32 %a
}

define i8 @calls(i8 %x, i32 %y, ptr %p) {
  %a = call i8 @ret_range(i8 %x)
  %b = call range(i8 0, 4) i8 @ret_range(i8 %x)
  %c = call i32 @ret_id(i32 %y)
  %yy = and i32 %y, 255
  %d = call i32 @ret_id(i32 %yy)
  %e = call i32 @ret_id(i32 range(i32 0, 100) 7)
  %f = call range(i8 1, 3) i8 @ret_range(i8 %x)
  %g = call ptr @ret_align(ptr %p)
  %h = call align 64 ptr @ret_align(ptr %p)
  ret i8 %a
}

define i32 @metadata(ptr %p) {
  %a = load i32, ptr %p, !range !0
  %b = load i8, ptr %p, !range !1
  %c = load i16, ptr %p, !range !2
  %d = load ptr, ptr %p, !align !3
  ret i32 %a
}

define ptr @aligns(i32 %i) {
  %a = alloca i32, align 8
  %b = alloca i64
  %c = alloca [3 x i8], align 4
  %d = alloca %T
  %e = alloca i8
  %f = alloca i64, align 64
  ret ptr %a
}

define ptr @globals() {
  %a = getelementptr i8, ptr @ga, i64 1
  %b = getelementptr i8, ptr @gn, i64 1
  %c = getelementptr i8, ptr @gl, i64 1
  %d = getelementptr i8, ptr @gw, i64 1
  %e = getelementptr i8, ptr @gd, i64 1
  %f = getelementptr i8, ptr @gu, i64 1
  ret ptr %a
}

!0 = !{i32 3, i32 7}
!1 = !{i8 0, i8 2, i8 8, i8 10}
!2 = !{i16 -8, i16 -4}
!3 = !{i64 16}
