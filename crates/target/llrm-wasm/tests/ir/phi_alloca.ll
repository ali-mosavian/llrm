target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"

declare void @use(ptr)
define i32 @f(i32 %n) {
e:
  %a = alloca [16 x i32]
  call void @use(ptr %a)
  br label %l
l:
  %p = phi ptr [ %a, %e ], [ %q, %l ]
  %v = load i32, ptr %p
  %q = getelementptr i8, ptr %p, i32 4
  %c = icmp ne i32 %v, 5
  br i1 %c, label %x, label %l
x:
  ret i32 %v
}
