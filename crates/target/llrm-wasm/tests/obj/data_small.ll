@init = global i32 5, align 4
@zero = global i32 0, align 4
@arr = global [3 x i32] [i32 1, i32 2, i32 3], align 4
define i32 @f() {
  %a = load i32, ptr @init
  %b = load i32, ptr @zero
  %p = getelementptr [3 x i32], ptr @arr, i32 0, i32 2
  %c = load i32, ptr %p
  %s = add i32 %a, %b
  %t = add i32 %s, %c
  store i32 %t, ptr @zero
  ret i32 %t
}
