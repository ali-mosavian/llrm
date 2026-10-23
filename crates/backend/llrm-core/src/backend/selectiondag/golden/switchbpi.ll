target datalayout = "e-m:e-p:32:32-f64:32:64-f80:32-n8:16:32-S128"
target triple = "i486-unknown-linux-gnu"

@g = global i32 0
declare void @abort() noreturn

define void @unreach_case(i32 %x) {
e:
  switch i32 %x, label %d [
    i32 1, label %t0
    i32 100, label %t1
    i32 1000, label %t2
    i32 10000, label %t3
    i32 100000, label %t4
    i32 1000000, label %t5
  ]
t0:
  store i32 0, ptr @g
  ret void
t1:
  store i32 1, ptr @g
  ret void
t2:
  unreachable
t3:
  store i32 3, ptr @g
  ret void
t4:
  store i32 4, ptr @g
  ret void
t5:
  store i32 5, ptr @g
  ret void
d:
  ret void
}

define void @noreturn_case(i32 %x) {
e:
  switch i32 %x, label %d [
    i32 1, label %t0
    i32 100, label %t1
    i32 1000, label %t2
    i32 10000, label %t3
    i32 100000, label %t4
    i32 1000000, label %t5
    i32 10000000, label %t6
  ]
t0:
  store i32 0, ptr @g
  ret void
t1:
  call void @abort()
  unreachable
t2:
  store i32 2, ptr @g
  ret void
t3:
  store i32 3, ptr @g
  ret void
t4:
  store i32 4, ptr @g
  ret void
t5:
  store i32 5, ptr @g
  ret void
t6:
  store i32 6, ptr @g
  ret void
d:
  ret void
}

define void @two_cold(i32 %x) {
e:
  switch i32 %x, label %d [
    i32 3, label %t0
    i32 30, label %t1
    i32 300, label %t2
    i32 3000, label %t3
    i32 30000, label %t4
  ]
t0:
  unreachable
t1:
  store i32 1, ptr @g
  ret void
t2:
  call void @abort()
  unreachable
t3:
  store i32 3, ptr @g
  ret void
t4:
  store i32 4, ptr @g
  ret void
d:
  ret void
}
