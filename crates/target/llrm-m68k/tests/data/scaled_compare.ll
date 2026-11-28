target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"

@maxcode = external global [4 x [18 x i32]], align 2

define i32 @decode(i32 noundef %0) noinline nounwind "frame-pointer"="all" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68020" "target-features"="+isa-68881" {
  %2 = alloca i32, align 2
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  store i32 %0, ptr %2, align 2
  %5 = call i32 @getbit()
  store i32 %5, ptr %3, align 2
  store i32 1, ptr %4, align 2
  br label %6

6:
  %7 = load i32, ptr %3, align 2
  %8 = load i32, ptr %2, align 2
  %9 = getelementptr inbounds [4 x [18 x i32]], ptr @maxcode, i32 0, i32 %8
  %10 = load i32, ptr %4, align 2
  %11 = getelementptr inbounds [18 x i32], ptr %9, i32 0, i32 %10
  %12 = load i32, ptr %11, align 2
  %13 = icmp sgt i32 %7, %12
  br i1 %13, label %14, label %21

14:
  %15 = load i32, ptr %3, align 2
  %16 = mul nsw i32 %15, 2
  %17 = call i32 @getbit()
  %18 = add nsw i32 %16, %17
  store i32 %18, ptr %3, align 2
  %19 = load i32, ptr %4, align 2
  %20 = add nsw i32 %19, 1
  store i32 %20, ptr %4, align 2
  br label %6, !llvm.loop !4

21:
  %22 = load i32, ptr %3, align 2
  ret i32 %22
}

declare i32 @getbit() "frame-pointer"="all" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68020" "target-features"="+isa-68881"

!llvm.module.flags = !{!0, !1, !2}

!0 = !{i32 8, !"PIC Level", i32 2}
!1 = !{i32 7, !"PIE Level", i32 2}
!2 = !{i32 7, !"frame-pointer", i32 2}
!4 = distinct !{!4, !5}
!5 = !{!"llvm.loop.mustprogress"}
