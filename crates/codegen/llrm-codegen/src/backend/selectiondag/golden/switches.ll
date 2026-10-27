target datalayout = "e-m:e-p:32:32-f64:32:64-f80:32-n8:16:32-S128"
target triple = "i486-unknown-linux-gnu"

define void @dense6(i32 %x) {
  switch i32 %x, label %d [
    i32 0, label %t0
    i32 1, label %t1
    i32 2, label %t2
    i32 3, label %t3
    i32 4, label %t4
    i32 5, label %t5
  ]
t0:
  ret void
t1:
  ret void
t2:
  ret void
t3:
  ret void
t4:
  ret void
t5:
  ret void
d:
  ret void
}

define void @sparse(i32 %x) {
  switch i32 %x, label %d [
    i32 1, label %t0
    i32 100, label %t1
    i32 1000, label %t2
    i32 10000, label %t3
  ]
t0:
  ret void
t1:
  ret void
t2:
  ret void
t3:
  ret void
d:
  ret void
}

define void @gaps(i32 %x) {
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
  ret void
t1:
  ret void
t2:
  ret void
t3:
  ret void
t4:
  ret void
t5:
  ret void
t6:
  ret void
t7:
  ret void
t8:
  ret void
t9:
  ret void
d:
  ret void
}

define void @bits3(i32 %x) {
  switch i32 %x, label %d [
    i32 1, label %t0
    i32 2, label %t1
    i32 3, label %t0
    i32 10, label %t1
    i32 11, label %t0
    i32 12, label %t1
  ]
t0:
  ret void
t1:
  ret void
d:
  ret void
}

define void @bits_one(i32 %x) {
  switch i32 %x, label %d [
    i32 0, label %t0
    i32 2, label %t0
    i32 4, label %t0
    i32 6, label %t0
    i32 8, label %t0
    i32 10, label %t0
  ]
t0:
  ret void
d:
  ret void
}

define void @neg(i32 %x) {
  switch i32 %x, label %d [
    i32 -3, label %t0
    i32 -2, label %t1
    i32 -1, label %t2
    i32 0, label %t3
    i32 1, label %t4
    i32 2, label %t5
  ]
t0:
  ret void
t1:
  ret void
t2:
  ret void
t3:
  ret void
t4:
  ret void
t5:
  ret void
d:
  ret void
}

define void @i8dense(i8 %x) {
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
  ret void
t1:
  ret void
t2:
  ret void
t3:
  ret void
t4:
  ret void
t5:
  ret void
t6:
  ret void
d:
  ret void
}

define void @two(i32 %x) {
  switch i32 %x, label %d [
    i32 5, label %t0
    i32 9, label %t1
  ]
t0:
  ret void
t1:
  ret void
d:
  ret void
}

define void @three(i32 %x) {
  switch i32 %x, label %d [
    i32 1, label %t0
    i32 2, label %t1
    i32 3, label %t2
  ]
t0:
  ret void
t1:
  ret void
t2:
  ret void
d:
  ret void
}

define void @mixed(i32 %x) {
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
  ret void
t1:
  ret void
t2:
  ret void
t3:
  ret void
t4:
  ret void
t5:
  ret void
t6:
  ret void
t7:
  ret void
d:
  ret void
}

define void @big(i32 %x) {
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
  ret void
t1:
  ret void
t2:
  ret void
t3:
  ret void
t4:
  ret void
d:
  ret void
}

define void @wide(i32 %x) {
  switch i32 %x, label %d [
    i32 0, label %t0
    i32 1, label %t1
    i32 2, label %t2
    i32 3, label %t3
    i32 4, label %t4
    i32 5, label %t5
    i32 6, label %t6
    i32 7, label %t7
    i32 8, label %t8
    i32 9, label %t9
    i32 10, label %t10
    i32 11, label %t11
    i32 12, label %t12
    i32 13, label %t13
    i32 14, label %t14
    i32 15, label %t15
    i32 16, label %t16
    i32 17, label %t17
    i32 18, label %t18
    i32 19, label %t19
    i32 20, label %t20
    i32 21, label %t21
    i32 22, label %t22
    i32 23, label %t23
    i32 24, label %t24
    i32 25, label %t25
    i32 26, label %t26
    i32 27, label %t27
    i32 28, label %t28
    i32 29, label %t29
    i32 30, label %t30
    i32 31, label %t31
    i32 32, label %t32
    i32 33, label %t33
    i32 34, label %t34
    i32 35, label %t35
    i32 36, label %t36
    i32 37, label %t37
    i32 38, label %t38
    i32 39, label %t39
    i32 40, label %t40
    i32 41, label %t41
    i32 42, label %t42
    i32 43, label %t43
    i32 44, label %t44
    i32 45, label %t45
    i32 46, label %t46
    i32 47, label %t47
    i32 48, label %t48
    i32 49, label %t49
    i32 50, label %t50
    i32 51, label %t51
    i32 52, label %t52
    i32 53, label %t53
    i32 54, label %t54
    i32 55, label %t55
    i32 56, label %t56
    i32 57, label %t57
    i32 58, label %t58
    i32 59, label %t59
    i32 60, label %t60
    i32 61, label %t61
    i32 62, label %t62
    i32 63, label %t63
    i32 64, label %t64
    i32 65, label %t65
    i32 66, label %t66
    i32 67, label %t67
    i32 68, label %t68
    i32 69, label %t69
    i32 70, label %t70
  ]
t0:
  ret void
t1:
  ret void
t2:
  ret void
t3:
  ret void
t4:
  ret void
t5:
  ret void
t6:
  ret void
t7:
  ret void
t8:
  ret void
t9:
  ret void
t10:
  ret void
t11:
  ret void
t12:
  ret void
t13:
  ret void
t14:
  ret void
t15:
  ret void
t16:
  ret void
t17:
  ret void
t18:
  ret void
t19:
  ret void
t20:
  ret void
t21:
  ret void
t22:
  ret void
t23:
  ret void
t24:
  ret void
t25:
  ret void
t26:
  ret void
t27:
  ret void
t28:
  ret void
t29:
  ret void
t30:
  ret void
t31:
  ret void
t32:
  ret void
t33:
  ret void
t34:
  ret void
t35:
  ret void
t36:
  ret void
t37:
  ret void
t38:
  ret void
t39:
  ret void
t40:
  ret void
t41:
  ret void
t42:
  ret void
t43:
  ret void
t44:
  ret void
t45:
  ret void
t46:
  ret void
t47:
  ret void
t48:
  ret void
t49:
  ret void
t50:
  ret void
t51:
  ret void
t52:
  ret void
t53:
  ret void
t54:
  ret void
t55:
  ret void
t56:
  ret void
t57:
  ret void
t58:
  ret void
t59:
  ret void
t60:
  ret void
t61:
  ret void
t62:
  ret void
t63:
  ret void
t64:
  ret void
t65:
  ret void
t66:
  ret void
t67:
  ret void
t68:
  ret void
t69:
  ret void
t70:
  ret void
d:
  ret void
}
