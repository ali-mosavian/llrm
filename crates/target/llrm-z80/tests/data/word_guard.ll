target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"
target triple = "msp430"

declare void @a()
declare void @b()

define void @gt(i16 %x) {
  %c = icmp sgt i16 %x, 0
  br i1 %c, label %t, label %f
t:
  call void @a()
  ret void
f:
  call void @b()
  ret void
}

define void @le(i16 %x) {
  %c = icmp sle i16 %x, 0
  br i1 %c, label %t, label %f
t:
  call void @a()
  ret void
f:
  call void @b()
  ret void
}

define i16 @sel(i16 %x, i16 %a, i16 %b) {
  %c = icmp sgt i16 %x, 0
  %r = select i1 %c, i16 %a, i16 %b
  ret i16 %r
}

define i16 @flag(i16 %x) {
  %c = icmp sle i16 %x, 0
  %r = zext i1 %c to i16
  ret i16 %r
}
