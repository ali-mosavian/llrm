; ModuleID = 'float_chains_in_loop_block'
target datalayout = "@m32-datalayout@"

; Function Attrs: nounwind uwtable
define dso_local i32 @entry(i32 noundef %0) #0 {
  %2 = alloca i32, align 4
  store i32 %0, ptr %2, align 4
  %3 = load i32, ptr %2, align 4
  %4 = call double @decay(double noundef 1.000000e+02, double noundef 0.000000e+00, i32 noundef %3)
  %5 = call double @llvm.fmuladd.f64(double %4, double 1.000000e+03, double 5.000000e-01)
  %6 = fptosi double %5 to i32
  ret i32 %6
}

; Function Attrs: nounwind uwtable
define internal double @decay(double noundef %0, double noundef %1, i32 noundef %2) #0 {
  %4 = alloca double, align 4
  %5 = alloca double, align 8
  %6 = alloca double, align 8
  %7 = alloca i32, align 4
  store double %0, ptr %5, align 8
  store double %1, ptr %6, align 8
  store i32 %2, ptr %7, align 4
  %8 = load i32, ptr %7, align 4
  %9 = icmp eq i32 %8, 0
  br i1 %9, label %10, label %12

10:                                               ; preds = %3
  %11 = load double, ptr %6, align 8
  store double %11, ptr %4, align 4
  br label %21

12:                                               ; preds = %3
  %13 = load double, ptr %5, align 8
  %14 = call double @llvm.fmuladd.f64(double %13, double 5.000000e-01, double 1.000000e+00)
  %15 = load double, ptr %6, align 8
  %16 = load double, ptr %5, align 8
  %17 = fadd double %15, %16
  %18 = load i32, ptr %7, align 4
  %19 = sub nsw i32 %18, 1
  %20 = call double @decay(double noundef %14, double noundef %17, i32 noundef %19)
  store double %20, ptr %4, align 4
  br label %21

21:                                               ; preds = %12, %10
  %22 = load double, ptr %4, align 4
  ret double %22
}

; Function Attrs: nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none)
declare double @llvm.fmuladd.f64(double, double, double) #1

; Function Attrs: nounwind uwtable
define dso_local i32 @main() #0 {
  %1 = alloca i32, align 4
  store i32 0, ptr %1, align 4
  %2 = call i32 @entry(i32 noundef 1000)
  call void @report(i32 noundef %2)
  ret i32 0
}

declare dso_local void @report(i32 noundef) #2

attributes #0 = { nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="i486" "target-features"="+x87" }
attributes #1 = { nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none) }
attributes #2 = { "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="i486" "target-features"="+x87" }


