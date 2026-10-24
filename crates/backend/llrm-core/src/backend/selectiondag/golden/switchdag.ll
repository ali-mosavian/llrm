target datalayout = "e-m:e-p:32:32-f64:32:64-f80:32-n8:16:32-S128"
target triple = "i486-unknown-linux-gnu"

@g = global i32 0

define void @sparse4(i32 %x) {
e:
  switch i32 %x, label %d [
    i32 1, label %t0
    i32 100, label %t1
    i32 1000, label %t2
    i32 10000, label %t3
  ]
t0:
  store i32 0, ptr @g
  ret void
t1:
  store i32 1, ptr @g
  ret void
t2:
  store i32 2, ptr @g
  ret void
t3:
  store i32 3, ptr @g
  ret void
d:
  ret void
}

define void @sparse7(i32 %x) {
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
  store i32 1, ptr @g
  ret void
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

define void @ranges(i32 %x) {
e:
  switch i32 %x, label %d [
    i32 1, label %t0
    i32 2, label %t0
    i32 3, label %t0
    i32 100, label %t1
    i32 101, label %t1
    i32 1000, label %t2
    i32 10000, label %t3
  ]
t0:
  store i32 0, ptr @g
  ret void
t1:
  store i32 1, ptr @g
  ret void
t2:
  store i32 2, ptr @g
  ret void
t3:
  store i32 3, ptr @g
  ret void
d:
  ret void
}

define void @ortrick(i32 %x) {
e:
  switch i32 %x, label %d [
    i32 4, label %t0
    i32 6, label %t1
  ]
t0:
  store i32 0, ptr @g
  ret void
t1:
  store i32 1, ptr @g
  ret void
d:
  ret void
}

define void @sparse3(i32 %x) {
e:
  switch i32 %x, label %d [
    i32 7, label %t0
    i32 700, label %t1
    i32 70000, label %t2
  ]
t0:
  store i32 0, ptr @g
  ret void
t1:
  store i32 1, ptr @g
  ret void
t2:
  store i32 2, ptr @g
  ret void
d:
  ret void
}

define void @unreach(i32 %x) {
e:
  switch i32 %x, label %d [
    i32 1, label %t0
    i32 100, label %t1
    i32 10000, label %t2
  ]
t0:
  store i32 0, ptr @g
  ret void
t1:
  store i32 1, ptr @g
  ret void
t2:
  store i32 2, ptr @g
  ret void
d:
  unreachable
}

define void @two_dest(i32 %x) {
e:
  switch i32 %x, label %d [
    i32 1, label %t0
    i32 50, label %t1
    i32 500, label %t0
    i32 5000, label %t1
    i32 50000, label %t0
  ]
t0:
  store i32 0, ptr @g
  ret void
t1:
  store i32 1, ptr @g
  ret void
d:
  ret void
}

define void @i8s(i8 %x) {
e:
  switch i8 %x, label %d [
    i8 1, label %t0
    i8 20, label %t1
    i8 40, label %t2
    i8 90, label %t3
    i8 120, label %t4
  ]
t0:
  store i32 0, ptr @g
  ret void
t1:
  store i32 1, ptr @g
  ret void
t2:
  store i32 2, ptr @g
  ret void
t3:
  store i32 3, ptr @g
  ret void
t4:
  store i32 4, ptr @g
  ret void
d:
  ret void
}
