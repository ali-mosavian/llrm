target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"

@v = internal global [14 x i32] [i32 0, i32 1, i32 2, i32 32767, i32 32768, i32 65535, i32 65536, i32 65537, i32 2147483647, i32 -2147483648, i32 -2147483647, i32 -65536, i32 -2, i32 -1], align 2

define i32 @main() nounwind "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68020" "target-features"="+isa-68881" {
  %1 = alloca i32, align 2
  %2 = alloca i32, align 2
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  store i32 0, ptr %1, align 2
  call void @llvm.lifetime.start.p0(i64 -1, ptr %2) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr %3) nounwind
  store i32 0, ptr %2, align 2, !tbaa !8
  br label %5

5:
  %6 = load i32, ptr %2, align 2, !tbaa !8
  %7 = icmp slt i32 %6, 14
  br i1 %7, label %8, label %40

8:
  call void @llvm.lifetime.start.p0(i64 -1, ptr %4) nounwind
  store i32 0, ptr %4, align 2, !tbaa !8
  store i32 0, ptr %3, align 2, !tbaa !8
  br label %9

9:
  %10 = load i32, ptr %3, align 2, !tbaa !8
  %11 = icmp slt i32 %10, 14
  br i1 %11, label %12, label %35

12:
  %13 = load i32, ptr %4, align 2, !tbaa !8
  %14 = mul i32 %13, 257
  %15 = load i32, ptr %2, align 2, !tbaa !8
  %16 = getelementptr inbounds [14 x i32], ptr @v, i32 0, i32 %15
  %17 = load volatile i32, ptr %16, align 2, !tbaa !8
  %18 = load i32, ptr %3, align 2, !tbaa !8
  %19 = getelementptr inbounds [14 x i32], ptr @v, i32 0, i32 %18
  %20 = load volatile i32, ptr %19, align 2, !tbaa !8
  %21 = call i32 @branches(i32 noundef %17, i32 noundef %20)
  %22 = add i32 %14, %21
  %23 = load i32, ptr %2, align 2, !tbaa !8
  %24 = getelementptr inbounds [14 x i32], ptr @v, i32 0, i32 %23
  %25 = load volatile i32, ptr %24, align 2, !tbaa !8
  %26 = load i32, ptr %3, align 2, !tbaa !8
  %27 = getelementptr inbounds [14 x i32], ptr @v, i32 0, i32 %26
  %28 = load volatile i32, ptr %27, align 2, !tbaa !8
  %29 = call i32 @selects(i32 noundef %25, i32 noundef %28)
  %30 = mul i32 %29, 65599
  %31 = add i32 %22, %30
  store i32 %31, ptr %4, align 2, !tbaa !8
  br label %32

32:
  %33 = load i32, ptr %3, align 2, !tbaa !8
  %34 = add nsw i32 %33, 1
  store i32 %34, ptr %3, align 2, !tbaa !8
  br label %9, !llvm.loop !9

35:
  %36 = load i32, ptr %4, align 2, !tbaa !8
  call void @report(i32 noundef %36)
  call void @llvm.lifetime.end.p0(i64 -1, ptr %4) nounwind
  br label %37

37:
  %38 = load i32, ptr %2, align 2, !tbaa !8
  %39 = add nsw i32 %38, 1
  store i32 %39, ptr %2, align 2, !tbaa !8
  br label %5, !llvm.loop !11

40:
  call void @llvm.lifetime.end.p0(i64 -1, ptr %3) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr %2) nounwind
  ret i32 0
}

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

define internal i32 @branches(i32 noundef %0, i32 noundef %1) nounwind "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68020" "target-features"="+isa-68881" {
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  %5 = alloca i32, align 2
  %6 = alloca i32, align 2
  %7 = alloca i32, align 2
  store i32 %0, ptr %3, align 2, !tbaa !8
  store i32 %1, ptr %4, align 2, !tbaa !8
  call void @llvm.lifetime.start.p0(i64 -1, ptr %5) nounwind
  %8 = load i32, ptr %3, align 2, !tbaa !8
  store i32 %8, ptr %5, align 2, !tbaa !8
  call void @llvm.lifetime.start.p0(i64 -1, ptr %6) nounwind
  %9 = load i32, ptr %4, align 2, !tbaa !8
  store i32 %9, ptr %6, align 2, !tbaa !8
  call void @llvm.lifetime.start.p0(i64 -1, ptr %7) nounwind
  store i32 0, ptr %7, align 2, !tbaa !8
  %10 = load i32, ptr %3, align 2, !tbaa !8
  %11 = load i32, ptr %4, align 2, !tbaa !8
  %12 = icmp ult i32 %10, %11
  br i1 %12, label %13, label %16

13:
  %14 = load i32, ptr %7, align 2, !tbaa !8
  %15 = add i32 %14, 1
  store i32 %15, ptr %7, align 2, !tbaa !8
  br label %16

16:
  %17 = load i32, ptr %3, align 2, !tbaa !8
  %18 = load i32, ptr %4, align 2, !tbaa !8
  %19 = icmp ule i32 %17, %18
  br i1 %19, label %20, label %23

20:
  %21 = load i32, ptr %7, align 2, !tbaa !8
  %22 = add i32 %21, 2
  store i32 %22, ptr %7, align 2, !tbaa !8
  br label %23

23:
  %24 = load i32, ptr %3, align 2, !tbaa !8
  %25 = load i32, ptr %4, align 2, !tbaa !8
  %26 = icmp ugt i32 %24, %25
  br i1 %26, label %27, label %30

27:
  %28 = load i32, ptr %7, align 2, !tbaa !8
  %29 = add i32 %28, 4
  store i32 %29, ptr %7, align 2, !tbaa !8
  br label %30

30:
  %31 = load i32, ptr %3, align 2, !tbaa !8
  %32 = load i32, ptr %4, align 2, !tbaa !8
  %33 = icmp uge i32 %31, %32
  br i1 %33, label %34, label %37

34:
  %35 = load i32, ptr %7, align 2, !tbaa !8
  %36 = add i32 %35, 8
  store i32 %36, ptr %7, align 2, !tbaa !8
  br label %37

37:
  %38 = load i32, ptr %5, align 2, !tbaa !8
  %39 = load i32, ptr %6, align 2, !tbaa !8
  %40 = icmp slt i32 %38, %39
  br i1 %40, label %41, label %44

41:
  %42 = load i32, ptr %7, align 2, !tbaa !8
  %43 = add i32 %42, 16
  store i32 %43, ptr %7, align 2, !tbaa !8
  br label %44

44:
  %45 = load i32, ptr %5, align 2, !tbaa !8
  %46 = load i32, ptr %6, align 2, !tbaa !8
  %47 = icmp sle i32 %45, %46
  br i1 %47, label %48, label %51

48:
  %49 = load i32, ptr %7, align 2, !tbaa !8
  %50 = add i32 %49, 32
  store i32 %50, ptr %7, align 2, !tbaa !8
  br label %51

51:
  %52 = load i32, ptr %5, align 2, !tbaa !8
  %53 = load i32, ptr %6, align 2, !tbaa !8
  %54 = icmp sgt i32 %52, %53
  br i1 %54, label %55, label %58

55:
  %56 = load i32, ptr %7, align 2, !tbaa !8
  %57 = add i32 %56, 64
  store i32 %57, ptr %7, align 2, !tbaa !8
  br label %58

58:
  %59 = load i32, ptr %5, align 2, !tbaa !8
  %60 = load i32, ptr %6, align 2, !tbaa !8
  %61 = icmp sge i32 %59, %60
  br i1 %61, label %62, label %65

62:
  %63 = load i32, ptr %7, align 2, !tbaa !8
  %64 = add i32 %63, 128
  store i32 %64, ptr %7, align 2, !tbaa !8
  br label %65

65:
  %66 = load i32, ptr %7, align 2, !tbaa !8
  call void @llvm.lifetime.end.p0(i64 -1, ptr %7) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr %6) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr %5) nounwind
  ret i32 %66
}

define internal i32 @selects(i32 noundef %0, i32 noundef %1) nounwind "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68020" "target-features"="+isa-68881" {
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  %5 = alloca i32, align 2
  %6 = alloca i32, align 2
  store i32 %0, ptr %3, align 2, !tbaa !8
  store i32 %1, ptr %4, align 2, !tbaa !8
  call void @llvm.lifetime.start.p0(i64 -1, ptr %5) nounwind
  %7 = load i32, ptr %3, align 2, !tbaa !8
  store i32 %7, ptr %5, align 2, !tbaa !8
  call void @llvm.lifetime.start.p0(i64 -1, ptr %6) nounwind
  %8 = load i32, ptr %4, align 2, !tbaa !8
  store i32 %8, ptr %6, align 2, !tbaa !8
  %9 = load i32, ptr %3, align 2, !tbaa !8
  %10 = load i32, ptr %4, align 2, !tbaa !8
  %11 = icmp ult i32 %9, %10
  %12 = zext i1 %11 to i64
  %13 = select i1 %11, i32 3, i32 5
  %14 = load i32, ptr %3, align 2, !tbaa !8
  %15 = load i32, ptr %4, align 2, !tbaa !8
  %16 = icmp uge i32 %14, %15
  %17 = zext i1 %16 to i64
  %18 = select i1 %16, i32 7, i32 11
  %19 = mul i32 %18, 16
  %20 = add i32 %13, %19
  %21 = load i32, ptr %5, align 2, !tbaa !8
  %22 = load i32, ptr %6, align 2, !tbaa !8
  %23 = icmp slt i32 %21, %22
  %24 = zext i1 %23 to i64
  %25 = select i1 %23, i32 13, i32 17
  %26 = mul i32 %25, 256
  %27 = add i32 %20, %26
  %28 = load i32, ptr %5, align 2, !tbaa !8
  %29 = load i32, ptr %6, align 2, !tbaa !8
  %30 = icmp sge i32 %28, %29
  %31 = zext i1 %30 to i64
  %32 = select i1 %30, i32 19, i32 23
  %33 = mul i32 %32, 4096
  %34 = add i32 %27, %33
  call void @llvm.lifetime.end.p0(i64 -1, ptr %6) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr %5) nounwind
  ret i32 %34
}

declare void @report(i32 noundef) "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68020" "target-features"="+isa-68881"

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

!llvm.module.flags = !{!0, !1}
!llvm.ident = !{!2}
!llvm.errno.tbaa = !{!3}

!0 = !{i32 8, !"PIC Level", i32 2}
!1 = !{i32 7, !"PIE Level", i32 2}
!2 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!3 = !{!4, !5, i64 0}
!4 = !{!"__libc_errno", !5, i64 0}
!5 = !{!"int", !6, i64 0}
!6 = !{!"omnipotent char", !7, i64 0}
!7 = !{!"Simple C/C++ TBAA"}
!8 = !{!5, !5, i64 0}
!9 = distinct !{!9, !10}
!10 = !{!"llvm.loop.mustprogress"}
!11 = distinct !{!11, !10}
