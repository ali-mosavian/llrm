; ModuleID = 'tests/run/c/spill_store_order.c'
source_filename = "tests/run/c/spill_store_order.c"
target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
target triple = "m68k"

@seed = internal global i32 7, align 2
@main.modes = internal constant [5 x i32] [i32 19, i32 4, i32 6, i32 16, i32 13], align 2

; Function Attrs: nounwind
define dso_local i32 @gd_read(i32 noundef %0, i32 noundef %1) #0 {
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  store i32 %0, ptr %3, align 2, !tbaa !6
  store i32 %1, ptr %4, align 2, !tbaa !6
  %5 = load i32, ptr @seed, align 2, !tbaa !6
  %6 = mul i32 %5, 1103515245
  %7 = add i32 %6, 12345
  store i32 %7, ptr @seed, align 2, !tbaa !6
  %8 = load i32, ptr @seed, align 2, !tbaa !6
  %9 = lshr i32 %8, 16
  %10 = and i32 %9, 15
  %11 = load i32, ptr %3, align 2, !tbaa !6
  %12 = mul i32 %11, 3
  %13 = load i32, ptr %4, align 2, !tbaa !6
  %14 = add i32 %12, %13
  %15 = xor i32 %10, %14
  ret i32 %15
}

; Function Attrs: nounwind
define dso_local void @pack_ref(i32 noundef %0, i32 noundef %1, i32 noundef %2, i32 noundef %3, i32 noundef %4, ptr noundef %5) #0 {
  %7 = alloca i32, align 2
  %8 = alloca i32, align 2
  %9 = alloca i32, align 2
  %10 = alloca i32, align 2
  %11 = alloca i32, align 2
  %12 = alloca ptr, align 2
  %13 = alloca i32, align 2
  %14 = alloca i32, align 2
  %15 = alloca i32, align 2
  %16 = alloca i32, align 2
  %17 = alloca i32, align 2
  %18 = alloca i32, align 2
  %19 = alloca i32, align 2
  %20 = alloca i32, align 2
  %21 = alloca i32, align 2
  store i32 %0, ptr %7, align 2, !tbaa !6
  store i32 %1, ptr %8, align 2, !tbaa !6
  store i32 %2, ptr %9, align 2, !tbaa !6
  store i32 %3, ptr %10, align 2, !tbaa !6
  store i32 %4, ptr %11, align 2, !tbaa !6
  store ptr %5, ptr %12, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %13) #3
  %22 = load i32, ptr %7, align 2, !tbaa !6
  %23 = icmp eq i32 %22, 16
  br i1 %23, label %27, label %24

24:                                               ; preds = %6
  %25 = load i32, ptr %7, align 2, !tbaa !6
  %26 = icmp eq i32 %25, 13
  br label %27

27:                                               ; preds = %24, %6
  %28 = phi i1 [ true, %6 ], [ %26, %24 ]
  %29 = zext i1 %28 to i64
  %30 = select i1 %28, i32 4, i32 1
  store i32 %30, ptr %13, align 2, !tbaa !6
  call void @llvm.lifetime.start.p0(ptr %14) #3
  %31 = load i32, ptr %7, align 2, !tbaa !6
  %32 = icmp eq i32 %31, 19
  br i1 %32, label %33, label %34

33:                                               ; preds = %27
  br label %39

34:                                               ; preds = %27
  %35 = load i32, ptr %7, align 2, !tbaa !6
  %36 = icmp eq i32 %35, 4
  %37 = zext i1 %36 to i64
  %38 = select i1 %36, i32 2, i32 1
  br label %39

39:                                               ; preds = %34, %33
  %40 = phi i32 [ 8, %33 ], [ %38, %34 ]
  store i32 %40, ptr %14, align 2, !tbaa !6
  call void @llvm.lifetime.start.p0(ptr %15) #3
  %41 = load i32, ptr %10, align 2, !tbaa !6
  %42 = load i32, ptr %14, align 2, !tbaa !6
  %43 = mul i32 %41, %42
  %44 = add i32 %43, 7
  %45 = udiv i32 %44, 8
  store i32 %45, ptr %15, align 2, !tbaa !6
  call void @llvm.lifetime.start.p0(ptr %16) #3
  call void @llvm.lifetime.start.p0(ptr %17) #3
  call void @llvm.lifetime.start.p0(ptr %18) #3
  store i32 0, ptr %17, align 2, !tbaa !6
  br label %46

46:                                               ; preds = %137, %39
  %47 = load i32, ptr %17, align 2, !tbaa !6
  %48 = load i32, ptr %11, align 2, !tbaa !6
  %49 = icmp ult i32 %47, %48
  br i1 %49, label %50, label %145

50:                                               ; preds = %46
  store i32 0, ptr %18, align 2, !tbaa !6
  br label %51

51:                                               ; preds = %61, %50
  %52 = load i32, ptr %18, align 2, !tbaa !6
  %53 = load i32, ptr %13, align 2, !tbaa !6
  %54 = load i32, ptr %15, align 2, !tbaa !6
  %55 = mul i32 %53, %54
  %56 = icmp ult i32 %52, %55
  br i1 %56, label %57, label %64

57:                                               ; preds = %51
  %58 = load ptr, ptr %12, align 2, !tbaa !7
  %59 = load i32, ptr %18, align 2, !tbaa !6
  %60 = getelementptr inbounds nuw i8, ptr %58, i32 %59
  store i8 0, ptr %60, align 1, !tbaa !10
  br label %61

61:                                               ; preds = %57
  %62 = load i32, ptr %18, align 2, !tbaa !6
  %63 = add i32 %62, 1
  store i32 %63, ptr %18, align 2, !tbaa !6
  br label %51, !llvm.loop !11

64:                                               ; preds = %51
  store i32 0, ptr %16, align 2, !tbaa !6
  br label %65

65:                                               ; preds = %133, %64
  %66 = load i32, ptr %16, align 2, !tbaa !6
  %67 = load i32, ptr %10, align 2, !tbaa !6
  %68 = icmp ult i32 %66, %67
  br i1 %68, label %69, label %136

69:                                               ; preds = %65
  call void @llvm.lifetime.start.p0(ptr %19) #3
  %70 = load i32, ptr %8, align 2, !tbaa !6
  %71 = load i32, ptr %16, align 2, !tbaa !6
  %72 = add i32 %70, %71
  %73 = load i32, ptr %9, align 2, !tbaa !6
  %74 = load i32, ptr %17, align 2, !tbaa !6
  %75 = add i32 %73, %74
  %76 = call i32 @gd_read(i32 noundef %72, i32 noundef %75) #4
  store i32 %76, ptr %19, align 2, !tbaa !6
  call void @llvm.lifetime.start.p0(ptr %20) #3
  call void @llvm.lifetime.start.p0(ptr %21) #3
  %77 = load i32, ptr %16, align 2, !tbaa !6
  %78 = load i32, ptr %14, align 2, !tbaa !6
  %79 = mul i32 %77, %78
  store i32 %79, ptr %21, align 2, !tbaa !6
  %80 = load i32, ptr %13, align 2, !tbaa !6
  %81 = icmp ugt i32 %80, 1
  br i1 %81, label %82, label %114

82:                                               ; preds = %69
  store i32 0, ptr %20, align 2, !tbaa !6
  br label %83

83:                                               ; preds = %110, %82
  %84 = load i32, ptr %20, align 2, !tbaa !6
  %85 = load i32, ptr %13, align 2, !tbaa !6
  %86 = icmp ult i32 %84, %85
  br i1 %86, label %87, label %113

87:                                               ; preds = %83
  %88 = load i32, ptr %19, align 2, !tbaa !6
  %89 = load i32, ptr %20, align 2, !tbaa !6
  %90 = lshr i32 %88, %89
  %91 = and i32 %90, 1
  %92 = icmp ne i32 %91, 0
  br i1 %92, label %93, label %109

93:                                               ; preds = %87
  %94 = load i32, ptr %16, align 2, !tbaa !6
  %95 = and i32 %94, 7
  %96 = ashr i32 128, %95
  %97 = load ptr, ptr %12, align 2, !tbaa !7
  %98 = load i32, ptr %20, align 2, !tbaa !6
  %99 = load i32, ptr %15, align 2, !tbaa !6
  %100 = mul i32 %98, %99
  %101 = load i32, ptr %16, align 2, !tbaa !6
  %102 = lshr i32 %101, 3
  %103 = add i32 %100, %102
  %104 = getelementptr inbounds nuw i8, ptr %97, i32 %103
  %105 = load i8, ptr %104, align 1, !tbaa !10
  %106 = zext i8 %105 to i32
  %107 = or i32 %106, %96
  %108 = trunc i32 %107 to i8
  store i8 %108, ptr %104, align 1, !tbaa !10
  br label %109

109:                                              ; preds = %93, %87
  br label %110

110:                                              ; preds = %109
  %111 = load i32, ptr %20, align 2, !tbaa !6
  %112 = add i32 %111, 1
  store i32 %112, ptr %20, align 2, !tbaa !6
  br label %83, !llvm.loop !14

113:                                              ; preds = %83
  br label %132

114:                                              ; preds = %69
  %115 = load i32, ptr %19, align 2, !tbaa !6
  %116 = load i32, ptr %14, align 2, !tbaa !6
  %117 = sub i32 8, %116
  %118 = load i32, ptr %21, align 2, !tbaa !6
  %119 = and i32 %118, 7
  %120 = sub i32 %117, %119
  %121 = shl i32 %115, %120
  %122 = trunc i32 %121 to i8
  %123 = zext i8 %122 to i32
  %124 = load ptr, ptr %12, align 2, !tbaa !7
  %125 = load i32, ptr %21, align 2, !tbaa !6
  %126 = lshr i32 %125, 3
  %127 = getelementptr inbounds nuw i8, ptr %124, i32 %126
  %128 = load i8, ptr %127, align 1, !tbaa !10
  %129 = zext i8 %128 to i32
  %130 = or i32 %129, %123
  %131 = trunc i32 %130 to i8
  store i8 %131, ptr %127, align 1, !tbaa !10
  br label %132

132:                                              ; preds = %114, %113
  call void @llvm.lifetime.end.p0(ptr %21) #3
  call void @llvm.lifetime.end.p0(ptr %20) #3
  call void @llvm.lifetime.end.p0(ptr %19) #3
  br label %133

133:                                              ; preds = %132
  %134 = load i32, ptr %16, align 2, !tbaa !6
  %135 = add i32 %134, 1
  store i32 %135, ptr %16, align 2, !tbaa !6
  br label %65, !llvm.loop !15

136:                                              ; preds = %65
  br label %137

137:                                              ; preds = %136
  %138 = load i32, ptr %17, align 2, !tbaa !6
  %139 = add i32 %138, 1
  store i32 %139, ptr %17, align 2, !tbaa !6
  %140 = load i32, ptr %13, align 2, !tbaa !6
  %141 = load i32, ptr %15, align 2, !tbaa !6
  %142 = mul i32 %140, %141
  %143 = load ptr, ptr %12, align 2, !tbaa !7
  %144 = getelementptr inbounds nuw i8, ptr %143, i32 %142
  store ptr %144, ptr %12, align 2, !tbaa !7
  br label %46, !llvm.loop !16

145:                                              ; preds = %46
  call void @llvm.lifetime.end.p0(ptr %18) #3
  call void @llvm.lifetime.end.p0(ptr %17) #3
  call void @llvm.lifetime.end.p0(ptr %16) #3
  call void @llvm.lifetime.end.p0(ptr %15) #3
  call void @llvm.lifetime.end.p0(ptr %14) #3
  call void @llvm.lifetime.end.p0(ptr %13) #3
  ret void
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

; Function Attrs: nounwind
define dso_local i32 @main() #0 {
  %1 = alloca [400 x i8], align 1
  %2 = alloca i32, align 2
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #3
  call void @llvm.lifetime.start.p0(ptr %2) #3
  call void @llvm.lifetime.start.p0(ptr %3) #3
  call void @llvm.lifetime.start.p0(ptr %4) #3
  store i32 0, ptr %2, align 2, !tbaa !6
  br label %5

5:                                                ; preds = %40, %0
  %6 = load i32, ptr %2, align 2, !tbaa !6
  %7 = icmp ult i32 %6, 5
  br i1 %7, label %8, label %43

8:                                                ; preds = %5
  store i32 0, ptr %3, align 2, !tbaa !6
  br label %9

9:                                                ; preds = %15, %8
  %10 = load i32, ptr %3, align 2, !tbaa !6
  %11 = icmp ult i32 %10, 400
  br i1 %11, label %12, label %18

12:                                               ; preds = %9
  %13 = load i32, ptr %3, align 2, !tbaa !6
  %14 = getelementptr inbounds nuw [400 x i8], ptr %1, i32 0, i32 %13
  store i8 -18, ptr %14, align 1, !tbaa !10
  br label %15

15:                                               ; preds = %12
  %16 = load i32, ptr %3, align 2, !tbaa !6
  %17 = add i32 %16, 1
  store i32 %17, ptr %3, align 2, !tbaa !6
  br label %9, !llvm.loop !17

18:                                               ; preds = %9
  %19 = load i32, ptr %2, align 2, !tbaa !6
  %20 = getelementptr inbounds nuw [5 x i32], ptr @main.modes, i32 0, i32 %19
  %21 = load i32, ptr %20, align 2, !tbaa !6
  %22 = getelementptr inbounds [400 x i8], ptr %1, i32 0, i32 0
  call void @pack_ref(i32 noundef %21, i32 noundef 20, i32 noundef 7, i32 noundef 19, i32 noundef 7, ptr noundef %22) #4
  store i32 0, ptr %4, align 2, !tbaa !6
  store i32 0, ptr %3, align 2, !tbaa !6
  br label %23

23:                                               ; preds = %34, %18
  %24 = load i32, ptr %3, align 2, !tbaa !6
  %25 = icmp ult i32 %24, 400
  br i1 %25, label %26, label %37

26:                                               ; preds = %23
  %27 = load i32, ptr %4, align 2, !tbaa !6
  %28 = mul i32 %27, 31
  %29 = load i32, ptr %3, align 2, !tbaa !6
  %30 = getelementptr inbounds nuw [400 x i8], ptr %1, i32 0, i32 %29
  %31 = load i8, ptr %30, align 1, !tbaa !10
  %32 = zext i8 %31 to i32
  %33 = add i32 %28, %32
  store i32 %33, ptr %4, align 2, !tbaa !6
  br label %34

34:                                               ; preds = %26
  %35 = load i32, ptr %3, align 2, !tbaa !6
  %36 = add i32 %35, 1
  store i32 %36, ptr %3, align 2, !tbaa !6
  br label %23, !llvm.loop !18

37:                                               ; preds = %23
  %38 = load i32, ptr %4, align 2, !tbaa !6
  %39 = and i32 %38, 2147483647
  call void @report(i32 noundef %39) #4
  br label %40

40:                                               ; preds = %37
  %41 = load i32, ptr %2, align 2, !tbaa !6
  %42 = add i32 %41, 1
  store i32 %42, ptr %2, align 2, !tbaa !6
  br label %5, !llvm.loop !19

43:                                               ; preds = %5
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  call void @llvm.lifetime.end.p0(ptr %1) #3
  ret i32 0
}

declare dso_local void @report(i32 noundef) #2

attributes #0 = { nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #3 = { nounwind }
attributes #4 = { nobuiltin "no-builtins" }

!llvm.ident = !{!0}
!llvm.errno.tbaa = !{!1}

!0 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!1 = !{!2, !3, i64 0}
!2 = !{!"__libc_errno", !3, i64 0}
!3 = !{!"int", !4, i64 0}
!4 = !{!"omnipotent char", !5, i64 0}
!5 = !{!"Simple C/C++ TBAA"}
!6 = !{!3, !3, i64 0}
!7 = !{!8, !8, i64 0}
!8 = !{!"p1 omnipotent char", !9, i64 0}
!9 = !{!"any pointer", !4, i64 0}
!10 = !{!4, !4, i64 0}
!11 = distinct !{!11, !12, !13}
!12 = !{!"llvm.loop.mustprogress"}
!13 = !{!"llvm.loop.unroll.disable"}
!14 = distinct !{!14, !12, !13}
!15 = distinct !{!15, !12, !13}
!16 = distinct !{!16, !12, !13}
!17 = distinct !{!17, !12, !13}
!18 = distinct !{!18, !12, !13}
!19 = distinct !{!19, !12, !13}
