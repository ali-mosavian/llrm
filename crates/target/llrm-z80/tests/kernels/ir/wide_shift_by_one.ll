; ModuleID = '/home/alim/work/personal/llrm/.claude/worktrees/funny-chatelet-66ad45/tests/run/c/wide_shift_by_one.c'
source_filename = "/home/alim/work/personal/llrm/.claude/worktrees/funny-chatelet-66ad45/tests/run/c/wide_shift_by_one.c"
target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"

@v = internal global [14 x i32] [i32 0, i32 1, i32 32767, i32 32768, i32 32769, i32 65535, i32 65536, i32 2147483647, i32 -2147483648, i32 -2147483647, i32 -65536, i32 -1, i32 305419896, i32 -1698898192], align 2

; Function Attrs: nounwind
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  %2 = alloca i16, align 2
  %3 = alloca i16, align 2
  %4 = alloca i32, align 2
  %5 = alloca i32, align 2
  %6 = alloca i64, align 2
  %7 = alloca i64, align 2
  %8 = alloca i64, align 2
  %9 = alloca i64, align 2
  store i16 0, ptr %1, align 2
  call void @llvm.lifetime.start.p0(ptr %2) #3
  call void @llvm.lifetime.start.p0(ptr %3) #3
  store i16 0, ptr %2, align 2, !tbaa !7
  br label %10

10:                                               ; preds = %95, %0
  %11 = load i16, ptr %2, align 2, !tbaa !7
  %12 = icmp slt i16 %11, 14
  br i1 %12, label %13, label %98

13:                                               ; preds = %10
  call void @llvm.lifetime.start.p0(ptr %4) #3
  %14 = load i16, ptr %2, align 2, !tbaa !7
  %15 = getelementptr inbounds [14 x i32], ptr @v, i16 0, i16 %14
  %16 = load volatile i32, ptr %15, align 2, !tbaa !8
  store i32 %16, ptr %4, align 2, !tbaa !8
  call void @llvm.lifetime.start.p0(ptr %5) #3
  store i32 0, ptr %5, align 2, !tbaa !8
  %17 = load i32, ptr %5, align 2, !tbaa !8
  %18 = load i32, ptr %4, align 2, !tbaa !8
  %19 = shl i32 %18, 1
  %20 = call i32 @fold32(i32 noundef %17, i32 noundef %19)
  store i32 %20, ptr %5, align 2, !tbaa !8
  %21 = load i32, ptr %5, align 2, !tbaa !8
  %22 = load i32, ptr %4, align 2, !tbaa !8
  %23 = lshr i32 %22, 1
  %24 = call i32 @fold32(i32 noundef %21, i32 noundef %23)
  store i32 %24, ptr %5, align 2, !tbaa !8
  %25 = load i32, ptr %5, align 2, !tbaa !8
  %26 = load i32, ptr %4, align 2, !tbaa !8
  %27 = ashr i32 %26, 1
  %28 = call i32 @fold32(i32 noundef %25, i32 noundef %27)
  store i32 %28, ptr %5, align 2, !tbaa !8
  %29 = load i32, ptr %5, align 2, !tbaa !8
  %30 = load i32, ptr %4, align 2, !tbaa !8
  %31 = shl i32 %30, 1
  %32 = load i32, ptr %4, align 2, !tbaa !8
  %33 = lshr i32 %32, 31
  %34 = or i32 %31, %33
  %35 = call i32 @fold32(i32 noundef %29, i32 noundef %34)
  store i32 %35, ptr %5, align 2, !tbaa !8
  store i16 0, ptr %3, align 2, !tbaa !7
  br label %36

36:                                               ; preds = %90, %13
  %37 = load i16, ptr %3, align 2, !tbaa !7
  %38 = icmp slt i16 %37, 14
  br i1 %38, label %39, label %93

39:                                               ; preds = %36
  call void @llvm.lifetime.start.p0(ptr %6) #3
  %40 = load i32, ptr %4, align 2, !tbaa !8
  %41 = zext i32 %40 to i64
  %42 = shl i64 %41, 32
  %43 = load i16, ptr %3, align 2, !tbaa !7
  %44 = getelementptr inbounds [14 x i32], ptr @v, i16 0, i16 %43
  %45 = load volatile i32, ptr %44, align 2, !tbaa !8
  %46 = zext i32 %45 to i64
  %47 = or i64 %42, %46
  store i64 %47, ptr %6, align 2, !tbaa !10
  call void @llvm.lifetime.start.p0(ptr %7) #3
  %48 = load i64, ptr %6, align 2, !tbaa !10
  store i64 %48, ptr %7, align 2, !tbaa !10
  call void @llvm.lifetime.start.p0(ptr %8) #3
  %49 = load i64, ptr %6, align 2, !tbaa !10
  %50 = shl i64 %49, 1
  store i64 %50, ptr %8, align 2, !tbaa !10
  call void @llvm.lifetime.start.p0(ptr %9) #3
  %51 = load i64, ptr %6, align 2, !tbaa !10
  %52 = lshr i64 %51, 1
  store i64 %52, ptr %9, align 2, !tbaa !10
  %53 = load i32, ptr %5, align 2, !tbaa !8
  %54 = load i64, ptr %8, align 2, !tbaa !10
  %55 = trunc i64 %54 to i32
  %56 = call i32 @fold32(i32 noundef %53, i32 noundef %55)
  store i32 %56, ptr %5, align 2, !tbaa !8
  %57 = load i32, ptr %5, align 2, !tbaa !8
  %58 = load i64, ptr %8, align 2, !tbaa !10
  %59 = lshr i64 %58, 32
  %60 = trunc i64 %59 to i32
  %61 = call i32 @fold32(i32 noundef %57, i32 noundef %60)
  store i32 %61, ptr %5, align 2, !tbaa !8
  %62 = load i32, ptr %5, align 2, !tbaa !8
  %63 = load i64, ptr %9, align 2, !tbaa !10
  %64 = trunc i64 %63 to i32
  %65 = call i32 @fold32(i32 noundef %62, i32 noundef %64)
  store i32 %65, ptr %5, align 2, !tbaa !8
  %66 = load i32, ptr %5, align 2, !tbaa !8
  %67 = load i64, ptr %9, align 2, !tbaa !10
  %68 = lshr i64 %67, 32
  %69 = trunc i64 %68 to i32
  %70 = call i32 @fold32(i32 noundef %66, i32 noundef %69)
  store i32 %70, ptr %5, align 2, !tbaa !8
  %71 = load i32, ptr %5, align 2, !tbaa !8
  %72 = load i64, ptr %7, align 2, !tbaa !10
  %73 = ashr i64 %72, 1
  %74 = trunc i64 %73 to i32
  %75 = call i32 @fold32(i32 noundef %71, i32 noundef %74)
  store i32 %75, ptr %5, align 2, !tbaa !8
  %76 = load i32, ptr %5, align 2, !tbaa !8
  %77 = load i64, ptr %7, align 2, !tbaa !10
  %78 = ashr i64 %77, 1
  %79 = ashr i64 %78, 32
  %80 = trunc i64 %79 to i32
  %81 = call i32 @fold32(i32 noundef %76, i32 noundef %80)
  store i32 %81, ptr %5, align 2, !tbaa !8
  %82 = load i32, ptr %5, align 2, !tbaa !8
  %83 = load i64, ptr %6, align 2, !tbaa !10
  %84 = shl i64 %83, 1
  %85 = load i64, ptr %6, align 2, !tbaa !10
  %86 = lshr i64 %85, 63
  %87 = or i64 %84, %86
  %88 = trunc i64 %87 to i32
  %89 = call i32 @fold32(i32 noundef %82, i32 noundef %88)
  store i32 %89, ptr %5, align 2, !tbaa !8
  call void @llvm.lifetime.end.p0(ptr %9) #3
  call void @llvm.lifetime.end.p0(ptr %8) #3
  call void @llvm.lifetime.end.p0(ptr %7) #3
  call void @llvm.lifetime.end.p0(ptr %6) #3
  br label %90

90:                                               ; preds = %39
  %91 = load i16, ptr %3, align 2, !tbaa !7
  %92 = add nsw i16 %91, 1
  store i16 %92, ptr %3, align 2, !tbaa !7
  br label %36, !llvm.loop !12

93:                                               ; preds = %36
  %94 = load i32, ptr %5, align 2, !tbaa !8
  call void @report(i32 noundef %94)
  call void @llvm.lifetime.end.p0(ptr %5) #3
  call void @llvm.lifetime.end.p0(ptr %4) #3
  br label %95

95:                                               ; preds = %93
  %96 = load i16, ptr %2, align 2, !tbaa !7
  %97 = add nsw i16 %96, 1
  store i16 %97, ptr %2, align 2, !tbaa !7
  br label %10, !llvm.loop !14

98:                                               ; preds = %10
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  ret i16 0
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nounwind
define internal i32 @fold32(i32 noundef %0, i32 noundef %1) #0 {
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  store i32 %0, ptr %3, align 2, !tbaa !8
  store i32 %1, ptr %4, align 2, !tbaa !8
  %5 = load i32, ptr %3, align 2, !tbaa !8
  %6 = mul i32 %5, 257
  %7 = load i32, ptr %4, align 2, !tbaa !8
  %8 = add i32 %6, %7
  ret i32 %8
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

declare dso_local void @report(i32 noundef) #2

attributes #0 = { nounwind "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #3 = { nounwind }

!llvm.module.flags = !{!0}
!llvm.ident = !{!1}
!llvm.errno.tbaa = !{!2}

!0 = !{i32 1, !"wchar_size", i32 2}
!1 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!2 = !{!3, !4, i64 0}
!3 = !{!"__libc_errno", !4, i64 0}
!4 = !{!"int", !5, i64 0}
!5 = !{!"omnipotent char", !6, i64 0}
!6 = !{!"Simple C/C++ TBAA"}
!7 = !{!4, !4, i64 0}
!8 = !{!9, !9, i64 0}
!9 = !{!"long", !5, i64 0}
!10 = !{!11, !11, i64 0}
!11 = !{!"long long", !5, i64 0}
!12 = distinct !{!12, !13}
!13 = !{!"llvm.loop.mustprogress"}
!14 = distinct !{!14, !13}
