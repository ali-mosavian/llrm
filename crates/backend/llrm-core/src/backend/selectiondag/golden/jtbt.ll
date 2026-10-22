target datalayout = "e-m:e-p:32:32-f64:32:64-f80:32-n8:16:32-S128"
target triple = "i486-unknown-linux-gnu"

@g = global i32 0

define void @dense6(i32 %x) {
e:
  switch i32 %x, label %d [
    i32 0, label %t0
    i32 1, label %t1
    i32 2, label %t2
    i32 3, label %t3
    i32 4, label %t4
    i32 5, label %t5
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
d:
  ret void
}

define void @jt5(i32 %x) {
e:
  switch i32 %x, label %d [
    i32 0, label %t0
    i32 1, label %t1
    i32 2, label %t2
    i32 3, label %t3
    i32 4, label %t0
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

define void @gaps(i32 %x) {
e:
  switch i32 %x, label %d [
    i32 0, label %t0
    i32 1, label %t1
    i32 2, label %t2
    i32 3, label %t3
    i32 10, label %t4
    i32 11, label %t5
    i32 12, label %t6
    i32 13, label %t7
    i32 14, label %t8
    i32 15, label %t9
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
t7:
  store i32 7, ptr @g
  ret void
t8:
  store i32 8, ptr @g
  ret void
t9:
  store i32 9, ptr @g
  ret void
d:
  ret void
}

define void @neg(i32 %x) {
e:
  switch i32 %x, label %d [
    i32 -3, label %t0
    i32 -2, label %t1
    i32 -1, label %t2
    i32 0, label %t3
    i32 1, label %t4
    i32 2, label %t5
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
d:
  ret void
}

define void @i8dense(i8 %x) {
e:
  switch i8 %x, label %d [
    i8 10, label %t0
    i8 11, label %t1
    i8 12, label %t2
    i8 13, label %t3
    i8 14, label %t4
    i8 15, label %t5
    i8 16, label %t6
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

define void @unreachjt(i32 %x) {
e:
  switch i32 %x, label %d [
    i32 0, label %t0
    i32 1, label %t1
    i32 2, label %t2
    i32 3, label %t3
    i32 4, label %t4
    i32 5, label %t5
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
d:
  unreachable
}

define void @bits3(i32 %x) {
e:
  switch i32 %x, label %d [
    i32 1, label %t0
    i32 2, label %t1
    i32 3, label %t0
    i32 10, label %t1
    i32 11, label %t0
    i32 12, label %t1
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

define void @bits_one(i32 %x) {
e:
  switch i32 %x, label %d [
    i32 0, label %t0
    i32 2, label %t0
    i32 4, label %t0
    i32 6, label %t0
    i32 8, label %t0
    i32 10, label %t0
  ]
t0:
  store i32 0, ptr @g
  ret void
d:
  ret void
}

define void @mixed(i32 %x) {
e:
  switch i32 %x, label %d [
    i32 0, label %t0
    i32 1, label %t1
    i32 2, label %t2
    i32 3, label %t3
    i32 4, label %t4
    i32 5, label %t5
    i32 6, label %t6
    i32 7, label %t7
    i32 100, label %t0
    i32 101, label %t1
    i32 102, label %t2
    i32 103, label %t3
    i32 104, label %t4
    i32 1000, label %t5
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
t7:
  store i32 7, ptr @g
  ret void
d:
  ret void
}

define void @big(i32 %x) {
e:
  switch i32 %x, label %d [
    i32 0, label %t0
    i32 2, label %t1
    i32 4, label %t2
    i32 6, label %t3
    i32 8, label %t4
    i32 10, label %t0
    i32 12, label %t1
    i32 14, label %t2
    i32 16, label %t3
    i32 18, label %t4
    i32 20, label %t0
    i32 22, label %t1
    i32 24, label %t2
    i32 26, label %t3
    i32 28, label %t4
    i32 30, label %t0
    i32 32, label %t1
    i32 34, label %t2
    i32 36, label %t3
    i32 38, label %t4
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
