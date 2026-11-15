target datalayout = "@m64-datalayout@"
target triple = "x86_64-unknown-linux-gnu"

@a = internal global double 1.500000e+00, align 8
@b = internal global double 2.500000e-01, align 8
@c = internal global double 3.000000e+00, align 8
@d = internal global double 1.250000e-01, align 8

define i32 @f() nounwind uwtable "target-cpu"="x86-64" "tune-cpu"="generic" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" {
  %1 = load volatile double, ptr @a, align 8
  %2 = load volatile double, ptr @b, align 8
  %3 = fadd double %1, %2
  %4 = fmul double %3, 1.000000e+02
  %5 = fptosi double %4 to i64
  tail call void @report(i64 noundef %5)
  %6 = load volatile double, ptr @c, align 8
  %7 = load volatile double, ptr @d, align 8
  %8 = fadd double %6, %7
  %9 = fmul double %8, 1.000000e+03
  %10 = fptosi double %9 to i64
  tail call void @report(i64 noundef %10)
  %11 = load volatile double, ptr @a, align 8
  %12 = load volatile double, ptr @b, align 8
  %13 = fsub double %11, %12
  %14 = fmul double %13, 1.000000e+02
  %15 = fptosi double %14 to i64
  tail call void @report(i64 noundef %15)
  ret i32 0
}

declare void @report(i64 noundef)
