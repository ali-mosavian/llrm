@a = global i32 5, align 4
@b = global i32 0, align 4
@c = constant [3 x i32] [i32 1, i32 2, i32 3], align 4
@d = global i64 7, align 8
@e = global i8 1, align 1
@unused = global i16 9, align 2
@s = global { i8, i32 } { i8 1, i32 2 }, align 4
define i32 @f() {
  %x = load i32, ptr @e
  %y = load i32, ptr @d
  %p = getelementptr [3 x i32], ptr @c, i32 0, i32 2
  %z = load i32, ptr %p
  %t = load i32, ptr @a
  %u = add i32 %x, %y
  %v = add i32 %u, %z
  %w = add i32 %v, %t
  store i32 %w, ptr @b
  %q = load i32, ptr @s
  ret i32 %q
}
