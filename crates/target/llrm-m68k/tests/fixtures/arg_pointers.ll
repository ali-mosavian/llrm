target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
declare void @g()
define i32 @f(ptr %p0, ptr %p1, ptr %p2, ptr %p3, ptr %p4, ptr %p5, ptr %p6, ptr %p7, ptr %p8, ptr %p9, ptr %p10, ptr %p11, ptr %p12, ptr %p13) {
  store i32 0, ptr %p0
  store i32 1, ptr %p1
  store i32 2, ptr %p2
  store i32 3, ptr %p3
  store i32 4, ptr %p4
  store i32 5, ptr %p5
  store i32 6, ptr %p6
  store i32 7, ptr %p7
  store i32 8, ptr %p8
  store i32 9, ptr %p9
  store i32 10, ptr %p10
  store i32 11, ptr %p11
  store i32 12, ptr %p12
  store i32 13, ptr %p13
  call void @g()
  %v0 = load i32, ptr %p0
  %s0 = add i32 0, %v0
  %v1 = load i32, ptr %p1
  %s1 = add i32 %s0, %v1
  %v2 = load i32, ptr %p2
  %s2 = add i32 %s1, %v2
  %v3 = load i32, ptr %p3
  %s3 = add i32 %s2, %v3
  %v4 = load i32, ptr %p4
  %s4 = add i32 %s3, %v4
  %v5 = load i32, ptr %p5
  %s5 = add i32 %s4, %v5
  %v6 = load i32, ptr %p6
  %s6 = add i32 %s5, %v6
  %v7 = load i32, ptr %p7
  %s7 = add i32 %s6, %v7
  %v8 = load i32, ptr %p8
  %s8 = add i32 %s7, %v8
  %v9 = load i32, ptr %p9
  %s9 = add i32 %s8, %v9
  %v10 = load i32, ptr %p10
  %s10 = add i32 %s9, %v10
  %v11 = load i32, ptr %p11
  %s11 = add i32 %s10, %v11
  %v12 = load i32, ptr %p12
  %s12 = add i32 %s11, %v12
  %v13 = load i32, ptr %p13
  %s13 = add i32 %s12, %v13
  ret i32 %s13
}
