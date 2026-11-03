target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"

declare void @a()
declare void @b()
declare void @c()
declare void @d()
declare void @e()

define void @f(i32 %x) {
entry:
  switch i32 %x, label %other [
    i32 192, label %la
    i32 194, label %lb
    i32 196, label %lc
    i32 219, label %ld
  ]
la:
  call void @a()
  ret void
lb:
  call void @b()
  ret void
lc:
  call void @c()
  ret void
ld:
  call void @d()
  ret void
other:
  call void @e()
  ret void
}
