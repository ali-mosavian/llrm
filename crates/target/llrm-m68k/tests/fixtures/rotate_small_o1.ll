target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"

@bytes = internal constant [3 x i8] c"\01\A5\FF", align 1
@words = internal constant [3 x i16] [i16 1, i16 4660, i16 -32767], align 2

define i32 @main() nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" {
  %1 = alloca i32, align 2
  %2 = alloca i32, align 2
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  %5 = alloca i32, align 2
  call void @llvm.lifetime.start.p0(i64 -1, ptr %1) nounwind
  store i32 0, ptr %1, align 2, !tbaa !8
  call void @llvm.lifetime.start.p0(i64 -1, ptr %2) nounwind
  store i32 0, ptr %2, align 2, !tbaa !8
  call void @llvm.lifetime.start.p0(i64 -1, ptr %3) nounwind
  store i32 0, ptr %3, align 2, !tbaa !8
  call void @llvm.lifetime.start.p0(i64 -1, ptr %4) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr %5) nounwind
  store i32 0, ptr %4, align 2, !tbaa !10
  br label %6

6:
  %7 = load i32, ptr %4, align 2, !tbaa !10
  %8 = icmp ult i32 %7, 3
  br i1 %8, label %9, label %83

9:
  store i32 0, ptr %5, align 2, !tbaa !10
  br label %10

10:
  %11 = load i32, ptr %5, align 2, !tbaa !10
  %12 = icmp ult i32 %11, 16
  br i1 %12, label %13, label %51

13:
  %14 = load i32, ptr %1, align 2, !tbaa !8
  %15 = mul i32 %14, 31
  %16 = load i32, ptr %4, align 2, !tbaa !10
  %17 = getelementptr inbounds nuw [3 x i8], ptr @bytes, i32 0, i32 %16
  %18 = load i8, ptr %17, align 1, !tbaa !11
  %19 = load i32, ptr %5, align 2, !tbaa !10
  %20 = call zeroext i8 @rotl8(i8 noundef zeroext %18, i32 noundef %19) nobuiltin "no-builtins"
  %21 = zext i8 %20 to i32
  %22 = add i32 %15, %21
  %23 = load i32, ptr %4, align 2, !tbaa !10
  %24 = getelementptr inbounds nuw [3 x i8], ptr @bytes, i32 0, i32 %23
  %25 = load i8, ptr %24, align 1, !tbaa !11
  %26 = load i32, ptr %5, align 2, !tbaa !10
  %27 = call zeroext i8 @rotr8(i8 noundef zeroext %25, i32 noundef %26) nobuiltin "no-builtins"
  %28 = zext i8 %27 to i32
  %29 = mul i32 3, %28
  %30 = add i32 %22, %29
  store i32 %30, ptr %1, align 2, !tbaa !8
  %31 = load i32, ptr %2, align 2, !tbaa !8
  %32 = mul i32 %31, 31
  %33 = load i32, ptr %4, align 2, !tbaa !10
  %34 = getelementptr inbounds nuw [3 x i16], ptr @words, i32 0, i32 %33
  %35 = load i16, ptr %34, align 2, !tbaa !12
  %36 = load i32, ptr %5, align 2, !tbaa !10
  %37 = call zeroext i16 @rotl16(i16 noundef zeroext %35, i32 noundef %36) nobuiltin "no-builtins"
  %38 = zext i16 %37 to i32
  %39 = add i32 %32, %38
  %40 = load i32, ptr %4, align 2, !tbaa !10
  %41 = getelementptr inbounds nuw [3 x i16], ptr @words, i32 0, i32 %40
  %42 = load i16, ptr %41, align 2, !tbaa !12
  %43 = load i32, ptr %5, align 2, !tbaa !10
  %44 = call zeroext i16 @rotr16(i16 noundef zeroext %42, i32 noundef %43) nobuiltin "no-builtins"
  %45 = zext i16 %44 to i32
  %46 = mul i32 3, %45
  %47 = add i32 %39, %46
  store i32 %47, ptr %2, align 2, !tbaa !8
  br label %48

48:
  %49 = load i32, ptr %5, align 2, !tbaa !10
  %50 = add i32 %49, 1
  store i32 %50, ptr %5, align 2, !tbaa !10
  br label %10, !llvm.loop !14

51:
  %52 = load i32, ptr %3, align 2, !tbaa !8
  %53 = mul i32 %52, 31
  %54 = load i32, ptr %4, align 2, !tbaa !10
  %55 = getelementptr inbounds nuw [3 x i8], ptr @bytes, i32 0, i32 %54
  %56 = load i8, ptr %55, align 1, !tbaa !11
  %57 = call zeroext i8 @rotl8(i8 noundef zeroext %56, i32 noundef 3) nobuiltin "no-builtins"
  %58 = zext i8 %57 to i32
  %59 = add i32 %53, %58
  %60 = load i32, ptr %4, align 2, !tbaa !10
  %61 = getelementptr inbounds nuw [3 x i8], ptr @bytes, i32 0, i32 %60
  %62 = load i8, ptr %61, align 1, !tbaa !11
  %63 = call zeroext i8 @rotr8(i8 noundef zeroext %62, i32 noundef 1) nobuiltin "no-builtins"
  %64 = zext i8 %63 to i32
  %65 = add i32 %59, %64
  %66 = load i32, ptr %4, align 2, !tbaa !10
  %67 = getelementptr inbounds nuw [3 x i16], ptr @words, i32 0, i32 %66
  %68 = load i16, ptr %67, align 2, !tbaa !12
  %69 = call zeroext i16 @rotl16(i16 noundef zeroext %68, i32 noundef 5) nobuiltin "no-builtins"
  %70 = zext i16 %69 to i32
  %71 = mul i32 5, %70
  %72 = add i32 %65, %71
  %73 = load i32, ptr %4, align 2, !tbaa !10
  %74 = getelementptr inbounds nuw [3 x i16], ptr @words, i32 0, i32 %73
  %75 = load i16, ptr %74, align 2, !tbaa !12
  %76 = call zeroext i16 @rotr16(i16 noundef zeroext %75, i32 noundef 9) nobuiltin "no-builtins"
  %77 = zext i16 %76 to i32
  %78 = mul i32 7, %77
  %79 = add i32 %72, %78
  store i32 %79, ptr %3, align 2, !tbaa !8
  br label %80

80:
  %81 = load i32, ptr %4, align 2, !tbaa !10
  %82 = add i32 %81, 1
  store i32 %82, ptr %4, align 2, !tbaa !10
  br label %6, !llvm.loop !16

83:
  %84 = load i32, ptr %1, align 2, !tbaa !8
  %85 = and i32 %84, 65535
  call void @report(i32 noundef %85) nobuiltin "no-builtins"
  %86 = load i32, ptr %1, align 2, !tbaa !8
  %87 = lshr i32 %86, 16
  %88 = and i32 %87, 65535
  call void @report(i32 noundef %88) nobuiltin "no-builtins"
  %89 = load i32, ptr %2, align 2, !tbaa !8
  %90 = and i32 %89, 65535
  call void @report(i32 noundef %90) nobuiltin "no-builtins"
  %91 = load i32, ptr %2, align 2, !tbaa !8
  %92 = lshr i32 %91, 16
  %93 = and i32 %92, 65535
  call void @report(i32 noundef %93) nobuiltin "no-builtins"
  %94 = load i32, ptr %3, align 2, !tbaa !8
  %95 = and i32 %94, 65535
  call void @report(i32 noundef %95) nobuiltin "no-builtins"
  %96 = load i32, ptr %3, align 2, !tbaa !8
  %97 = lshr i32 %96, 16
  %98 = and i32 %97, 65535
  call void @report(i32 noundef %98) nobuiltin "no-builtins"
  call void @llvm.lifetime.end.p0(i64 -1, ptr %5) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr %4) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr %3) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr %2) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr %1) nounwind
  ret i32 0
}

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

define internal zeroext i8 @rotl8(i8 noundef zeroext %0, i32 noundef %1) nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" {
  %3 = alloca i8, align 1
  %4 = alloca i32, align 2
  store i8 %0, ptr %3, align 1, !tbaa !11
  store i32 %1, ptr %4, align 2, !tbaa !10
  %5 = load i8, ptr %3, align 1, !tbaa !11
  %6 = zext i8 %5 to i32
  %7 = load i32, ptr %4, align 2, !tbaa !10
  %8 = and i32 %7, 7
  %9 = shl i32 %6, %8
  %10 = load i8, ptr %3, align 1, !tbaa !11
  %11 = zext i8 %10 to i32
  %12 = load i32, ptr %4, align 2, !tbaa !10
  %13 = sub i32 0, %12
  %14 = and i32 %13, 7
  %15 = ashr i32 %11, %14
  %16 = or i32 %9, %15
  %17 = trunc i32 %16 to i8
  ret i8 %17
}

define internal zeroext i8 @rotr8(i8 noundef zeroext %0, i32 noundef %1) nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" {
  %3 = alloca i8, align 1
  %4 = alloca i32, align 2
  store i8 %0, ptr %3, align 1, !tbaa !11
  store i32 %1, ptr %4, align 2, !tbaa !10
  %5 = load i8, ptr %3, align 1, !tbaa !11
  %6 = zext i8 %5 to i32
  %7 = load i32, ptr %4, align 2, !tbaa !10
  %8 = and i32 %7, 7
  %9 = ashr i32 %6, %8
  %10 = load i8, ptr %3, align 1, !tbaa !11
  %11 = zext i8 %10 to i32
  %12 = load i32, ptr %4, align 2, !tbaa !10
  %13 = sub i32 0, %12
  %14 = and i32 %13, 7
  %15 = shl i32 %11, %14
  %16 = or i32 %9, %15
  %17 = trunc i32 %16 to i8
  ret i8 %17
}

define internal zeroext i16 @rotl16(i16 noundef zeroext %0, i32 noundef %1) nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" {
  %3 = alloca i16, align 2
  %4 = alloca i32, align 2
  store i16 %0, ptr %3, align 2, !tbaa !12
  store i32 %1, ptr %4, align 2, !tbaa !10
  %5 = load i16, ptr %3, align 2, !tbaa !12
  %6 = zext i16 %5 to i32
  %7 = load i32, ptr %4, align 2, !tbaa !10
  %8 = and i32 %7, 15
  %9 = shl i32 %6, %8
  %10 = load i16, ptr %3, align 2, !tbaa !12
  %11 = zext i16 %10 to i32
  %12 = load i32, ptr %4, align 2, !tbaa !10
  %13 = sub i32 0, %12
  %14 = and i32 %13, 15
  %15 = ashr i32 %11, %14
  %16 = or i32 %9, %15
  %17 = trunc i32 %16 to i16
  ret i16 %17
}

define internal zeroext i16 @rotr16(i16 noundef zeroext %0, i32 noundef %1) nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" {
  %3 = alloca i16, align 2
  %4 = alloca i32, align 2
  store i16 %0, ptr %3, align 2, !tbaa !12
  store i32 %1, ptr %4, align 2, !tbaa !10
  %5 = load i16, ptr %3, align 2, !tbaa !12
  %6 = zext i16 %5 to i32
  %7 = load i32, ptr %4, align 2, !tbaa !10
  %8 = and i32 %7, 15
  %9 = ashr i32 %6, %8
  %10 = load i16, ptr %3, align 2, !tbaa !12
  %11 = zext i16 %10 to i32
  %12 = load i32, ptr %4, align 2, !tbaa !10
  %13 = sub i32 0, %12
  %14 = and i32 %13, 15
  %15 = shl i32 %11, %14
  %16 = or i32 %9, %15
  %17 = trunc i32 %16 to i16
  ret i16 %17
}

declare void @report(i32 noundef) "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000"

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
!8 = !{!9, !9, i64 0}
!9 = !{!"long", !6, i64 0}
!10 = !{!5, !5, i64 0}
!11 = !{!6, !6, i64 0}
!12 = !{!13, !13, i64 0}
!13 = !{!"short", !6, i64 0}
!14 = distinct !{!14, !15}
!15 = !{!"llvm.loop.mustprogress"}
!16 = distinct !{!16, !15}
