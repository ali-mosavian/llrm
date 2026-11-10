target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
define i32 @k(ptr noundef nocapture nofree %0, i32 noundef %1) nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" nofree nosync norecurse memory(argmem: readwrite) willreturn {
  %3 = getelementptr i8, ptr %0, i32 -4
  %4 = getelementptr [4 x i8], ptr %3, i32 %1
  %5 = load i32, ptr %4, align 2
  %6 = icmp sgt i32 %1, 1
  br i1 %6, label %7, label %10
7:
  br label %18
8:
  %9 = load i32, ptr %4, align 2
  br label %10
10:
  %11 = phi i32 [ %5, %2 ], [ %9, %8 ]
  %12 = phi i32 [ 0, %2 ], [ %28, %8 ]
  %13 = getelementptr inbounds [4 x i8], ptr %0, i32 %12
  %14 = load i32, ptr %13, align 2
  store i32 %11, ptr %13, align 2
  store i32 %14, ptr %4, align 2
  %15 = load i32, ptr %0, align 2
  %16 = shl nsw i32 %15, 4
  %17 = add nsw i32 %16, %12
  ret i32 %17
18:
  %19 = phi ptr [ %0, %7 ], [ %29, %27 ]
  %20 = phi i32 [ %28, %27 ], [ 0, %7 ]
  %21 = load i32, ptr %19, align 2
  %22 = icmp slt i32 %21, %5
  br i1 %22, label %23, label %27
23:
  %24 = getelementptr inbounds [4 x i8], ptr %0, i32 %20
  %25 = load i32, ptr %24, align 2
  store i32 %21, ptr %24, align 2
  store i32 %25, ptr %19, align 2
  %26 = add nsw i32 %20, 1
  br label %27
27:
  %28 = phi i32 [ %26, %23 ], [ %20, %18 ]
  %29 = getelementptr i8, ptr %19, i32 4
  %lsr_fold_term_cond.replaced_term_cond = icmp eq ptr %29, %4
  br i1 %lsr_fold_term_cond.replaced_term_cond, label %8, label %18
}
