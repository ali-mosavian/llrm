; Three inlined copies of a loop that differ in one constant: clang -Os -Xclang -disable-llvm-passes (QB synth.c voice_run, reduced).
; ModuleID = 's.c'
source_filename = "s.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

%struct.Osc = type { i32, i32, i32, i32, i32, i32, i32, i32 }

; Function Attrs: nounwind optsize uwtable
define dso_local void @voice(ptr noundef %0, ptr noundef %1, i32 noundef %2) #0 {
  %4 = alloca ptr, align 8
  %5 = alloca ptr, align 8
  %6 = alloca i32, align 4
  store ptr %0, ptr %4, align 8, !tbaa !16
  store ptr %1, ptr %5, align 8, !tbaa !18
  store i32 %2, ptr %6, align 4, !tbaa !20
  %7 = load ptr, ptr %4, align 8, !tbaa !16
  %8 = getelementptr inbounds nuw %struct.Osc, ptr %7, i32 0, i32 5
  %9 = load i32, ptr %8, align 4, !tbaa !21
  %10 = and i32 %9, 3
  switch i32 %10, label %19 [
    i32 0, label %11
    i32 1, label %15
  ]

11:                                               ; preds = %3
  %12 = load ptr, ptr %4, align 8, !tbaa !16
  %13 = load ptr, ptr %5, align 8, !tbaa !18
  %14 = load i32, ptr %6, align 4, !tbaa !20
  call void @voice_run(ptr noundef %12, ptr noundef %13, i32 noundef %14, i32 noundef 0) #3
  br label %23

15:                                               ; preds = %3
  %16 = load ptr, ptr %4, align 8, !tbaa !16
  %17 = load ptr, ptr %5, align 8, !tbaa !18
  %18 = load i32, ptr %6, align 4, !tbaa !20
  call void @voice_run(ptr noundef %16, ptr noundef %17, i32 noundef %18, i32 noundef 1) #3
  br label %23

19:                                               ; preds = %3
  %20 = load ptr, ptr %4, align 8, !tbaa !16
  %21 = load ptr, ptr %5, align 8, !tbaa !18
  %22 = load i32, ptr %6, align 4, !tbaa !20
  call void @voice_run(ptr noundef %20, ptr noundef %21, i32 noundef %22, i32 noundef 2) #3
  br label %23

23:                                               ; preds = %19, %15, %11
  ret void
}

; Function Attrs: alwaysinline nounwind optsize uwtable
define internal void @voice_run(ptr noundef %0, ptr noundef %1, i32 noundef %2, i32 noundef %3) #1 {
  %5 = alloca ptr, align 8
  %6 = alloca ptr, align 8
  %7 = alloca i32, align 4
  %8 = alloca i32, align 4
  %9 = alloca %struct.Osc, align 4
  %10 = alloca i32, align 4
  %11 = alloca i32, align 4
  %12 = alloca i32, align 4
  %13 = alloca i32, align 4
  %14 = alloca i32, align 4
  store ptr %0, ptr %5, align 8, !tbaa !16
  store ptr %1, ptr %6, align 8, !tbaa !18
  store i32 %2, ptr %7, align 4, !tbaa !20
  store i32 %3, ptr %8, align 4, !tbaa !20
  call void @llvm.lifetime.start.p0(ptr %9) #4
  %15 = load ptr, ptr %5, align 8, !tbaa !16
  call void @llvm.memcpy.p0.p0.i64(ptr align 4 %9, ptr align 4 %15, i64 32, i1 false), !tbaa.struct !23
  call void @llvm.lifetime.start.p0(ptr %10) #4
  store i32 0, ptr %10, align 4, !tbaa !20
  br label %16

16:                                               ; preds = %126, %4
  %17 = load i32, ptr %10, align 4, !tbaa !20
  %18 = load i32, ptr %7, align 4, !tbaa !20
  %19 = icmp slt i32 %17, %18
  br i1 %19, label %21, label %20

20:                                               ; preds = %16
  store i32 2, ptr %11, align 4
  br label %127

21:                                               ; preds = %16
  %22 = getelementptr inbounds nuw %struct.Osc, ptr %9, i32 0, i32 2
  %23 = load i32, ptr %22, align 4, !tbaa !24
  %24 = icmp eq i32 %23, 0
  br i1 %24, label %25, label %30

25:                                               ; preds = %21
  %26 = getelementptr inbounds nuw %struct.Osc, ptr %9, i32 0, i32 6
  %27 = load i32, ptr %26, align 4, !tbaa !25
  %28 = icmp eq i32 %27, 3
  br i1 %28, label %29, label %30

29:                                               ; preds = %25
  store i32 2, ptr %11, align 4
  br label %127

30:                                               ; preds = %25, %21
  call void @llvm.lifetime.start.p0(ptr %12) #4
  %31 = load i32, ptr %7, align 4, !tbaa !20
  %32 = load i32, ptr %10, align 4, !tbaa !20
  %33 = sub nsw i32 %31, %32
  %34 = call i32 @run_len(ptr noundef %9, i32 noundef %33) #3
  store i32 %34, ptr %12, align 4, !tbaa !20
  %35 = load i32, ptr %12, align 4, !tbaa !20
  %36 = getelementptr inbounds nuw %struct.Osc, ptr %9, i32 0, i32 7
  %37 = load i32, ptr %36, align 4, !tbaa !26
  %38 = add i32 %37, %35
  store i32 %38, ptr %36, align 4, !tbaa !26
  call void @llvm.lifetime.start.p0(ptr %13) #4
  %39 = load i32, ptr %10, align 4, !tbaa !20
  %40 = load i32, ptr %12, align 4, !tbaa !20
  %41 = add nsw i32 %39, %40
  store i32 %41, ptr %13, align 4, !tbaa !20
  br label %42

42:                                               ; preds = %124, %30
  call void @llvm.lifetime.start.p0(ptr %14) #4
  %43 = getelementptr inbounds nuw %struct.Osc, ptr %9, i32 0, i32 0
  %44 = load i32, ptr %43, align 4, !tbaa !27
  %45 = lshr i32 %44, 16
  %46 = getelementptr inbounds nuw %struct.Osc, ptr %9, i32 0, i32 2
  %47 = load i32, ptr %46, align 4, !tbaa !24
  %48 = ashr i32 %47, 9
  %49 = mul nsw i32 %45, %48
  %50 = ashr i32 %49, 15
  store i32 %50, ptr %14, align 4, !tbaa !20
  %51 = getelementptr inbounds nuw %struct.Osc, ptr %9, i32 0, i32 1
  %52 = load i32, ptr %51, align 4, !tbaa !28
  %53 = getelementptr inbounds nuw %struct.Osc, ptr %9, i32 0, i32 0
  %54 = load i32, ptr %53, align 4, !tbaa !27
  %55 = add i32 %54, %52
  store i32 %55, ptr %53, align 4, !tbaa !27
  %56 = load i32, ptr %8, align 4, !tbaa !20
  %57 = icmp ne i32 %56, 0
  br i1 %57, label %58, label %100

58:                                               ; preds = %42
  %59 = getelementptr inbounds nuw %struct.Osc, ptr %9, i32 0, i32 4
  %60 = load i32, ptr %59, align 4, !tbaa !29
  %61 = ashr i32 %60, 3
  %62 = getelementptr inbounds nuw %struct.Osc, ptr %9, i32 0, i32 3
  %63 = load i32, ptr %62, align 4, !tbaa !30
  %64 = add nsw i32 %63, %61
  store i32 %64, ptr %62, align 4, !tbaa !30
  %65 = load i32, ptr %14, align 4, !tbaa !20
  %66 = getelementptr inbounds nuw %struct.Osc, ptr %9, i32 0, i32 3
  %67 = load i32, ptr %66, align 4, !tbaa !30
  %68 = sub nsw i32 %65, %67
  %69 = getelementptr inbounds nuw %struct.Osc, ptr %9, i32 0, i32 4
  %70 = load i32, ptr %69, align 4, !tbaa !29
  %71 = ashr i32 %70, 2
  %72 = sub nsw i32 %68, %71
  %73 = ashr i32 %72, 1
  %74 = getelementptr inbounds nuw %struct.Osc, ptr %9, i32 0, i32 4
  %75 = load i32, ptr %74, align 4, !tbaa !29
  %76 = add nsw i32 %75, %73
  store i32 %76, ptr %74, align 4, !tbaa !29
  %77 = load i32, ptr %8, align 4, !tbaa !20
  %78 = icmp eq i32 %77, 1
  br i1 %78, label %79, label %82

79:                                               ; preds = %58
  %80 = getelementptr inbounds nuw %struct.Osc, ptr %9, i32 0, i32 3
  %81 = load i32, ptr %80, align 4, !tbaa !30
  br label %85

82:                                               ; preds = %58
  %83 = getelementptr inbounds nuw %struct.Osc, ptr %9, i32 0, i32 4
  %84 = load i32, ptr %83, align 4, !tbaa !29
  br label %85

85:                                               ; preds = %82, %79
  %86 = phi i32 [ %81, %79 ], [ %84, %82 ]
  %87 = sext i32 %86 to i64
  %88 = getelementptr inbounds nuw %struct.Osc, ptr %9, i32 0, i32 5
  %89 = load i32, ptr %88, align 4, !tbaa !21
  %90 = sext i32 %89 to i64
  %91 = mul nsw i64 %87, %90
  %92 = ashr i64 %91, 15
  %93 = trunc i64 %92 to i32
  %94 = load ptr, ptr %6, align 8, !tbaa !18
  %95 = load i32, ptr %10, align 4, !tbaa !20
  %96 = sext i32 %95 to i64
  %97 = getelementptr inbounds i32, ptr %94, i64 %96
  %98 = load i32, ptr %97, align 4, !tbaa !20
  %99 = add nsw i32 %98, %93
  store i32 %99, ptr %97, align 4, !tbaa !20
  br label %112

100:                                              ; preds = %42
  %101 = load i32, ptr %14, align 4, !tbaa !20
  %102 = getelementptr inbounds nuw %struct.Osc, ptr %9, i32 0, i32 5
  %103 = load i32, ptr %102, align 4, !tbaa !21
  %104 = mul nsw i32 %101, %103
  %105 = ashr i32 %104, 15
  %106 = load ptr, ptr %6, align 8, !tbaa !18
  %107 = load i32, ptr %10, align 4, !tbaa !20
  %108 = sext i32 %107 to i64
  %109 = getelementptr inbounds i32, ptr %106, i64 %108
  %110 = load i32, ptr %109, align 4, !tbaa !20
  %111 = add nsw i32 %110, %105
  store i32 %111, ptr %109, align 4, !tbaa !20
  br label %112

112:                                              ; preds = %100, %85
  %113 = load i32, ptr %10, align 4, !tbaa !20
  %114 = add nsw i32 %113, 1
  store i32 %114, ptr %10, align 4, !tbaa !20
  %115 = load i32, ptr %13, align 4, !tbaa !20
  %116 = icmp eq i32 %114, %115
  br i1 %116, label %117, label %118

117:                                              ; preds = %112
  store i32 4, ptr %11, align 4
  br label %122

118:                                              ; preds = %112
  %119 = getelementptr inbounds nuw %struct.Osc, ptr %9, i32 0, i32 2
  %120 = load i32, ptr %119, align 4, !tbaa !24
  %121 = add nsw i32 %120, 1
  store i32 %121, ptr %119, align 4, !tbaa !24
  store i32 0, ptr %11, align 4
  br label %122

122:                                              ; preds = %118, %117
  call void @llvm.lifetime.end.p0(ptr %14) #4
  %123 = load i32, ptr %11, align 4
  switch i32 %123, label %125 [
    i32 0, label %124
  ]

124:                                              ; preds = %122
  br label %42

125:                                              ; preds = %122
  call void @llvm.lifetime.end.p0(ptr %13) #4
  br label %126

126:                                              ; preds = %125
  call void @llvm.lifetime.end.p0(ptr %12) #4
  br label %16, !llvm.loop !31

127:                                              ; preds = %29, %20
  call void @llvm.lifetime.end.p0(ptr %10) #4
  br label %128

128:                                              ; preds = %127
  %129 = getelementptr inbounds nuw %struct.Osc, ptr %9, i32 0, i32 0
  %130 = load i32, ptr %129, align 4, !tbaa !27
  %131 = load ptr, ptr %5, align 8, !tbaa !16
  %132 = getelementptr inbounds nuw %struct.Osc, ptr %131, i32 0, i32 0
  store i32 %130, ptr %132, align 4, !tbaa !27
  %133 = getelementptr inbounds nuw %struct.Osc, ptr %9, i32 0, i32 2
  %134 = load i32, ptr %133, align 4, !tbaa !24
  %135 = load ptr, ptr %5, align 8, !tbaa !16
  %136 = getelementptr inbounds nuw %struct.Osc, ptr %135, i32 0, i32 2
  store i32 %134, ptr %136, align 4, !tbaa !24
  %137 = getelementptr inbounds nuw %struct.Osc, ptr %9, i32 0, i32 6
  %138 = load i32, ptr %137, align 4, !tbaa !25
  %139 = load ptr, ptr %5, align 8, !tbaa !16
  %140 = getelementptr inbounds nuw %struct.Osc, ptr %139, i32 0, i32 6
  store i32 %138, ptr %140, align 4, !tbaa !25
  %141 = getelementptr inbounds nuw %struct.Osc, ptr %9, i32 0, i32 3
  %142 = load i32, ptr %141, align 4, !tbaa !30
  %143 = load ptr, ptr %5, align 8, !tbaa !16
  %144 = getelementptr inbounds nuw %struct.Osc, ptr %143, i32 0, i32 3
  store i32 %142, ptr %144, align 4, !tbaa !30
  %145 = getelementptr inbounds nuw %struct.Osc, ptr %9, i32 0, i32 4
  %146 = load i32, ptr %145, align 4, !tbaa !29
  %147 = load ptr, ptr %5, align 8, !tbaa !16
  %148 = getelementptr inbounds nuw %struct.Osc, ptr %147, i32 0, i32 4
  store i32 %146, ptr %148, align 4, !tbaa !29
  %149 = getelementptr inbounds nuw %struct.Osc, ptr %9, i32 0, i32 7
  %150 = load i32, ptr %149, align 4, !tbaa !26
  %151 = load ptr, ptr %5, align 8, !tbaa !16
  %152 = getelementptr inbounds nuw %struct.Osc, ptr %151, i32 0, i32 7
  store i32 %150, ptr %152, align 4, !tbaa !26
  call void @llvm.lifetime.end.p0(ptr %9) #4
  ret void
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #2

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.memcpy.p0.p0.i64(ptr noalias writeonly captures(none), ptr noalias readonly captures(none), i64, i1 immarg) #2

; Function Attrs: nounwind optsize uwtable
define internal i32 @run_len(ptr noundef %0, i32 noundef %1) #0 {
  %3 = alloca ptr, align 8
  %4 = alloca i32, align 4
  store ptr %0, ptr %3, align 8, !tbaa !16
  store i32 %1, ptr %4, align 4, !tbaa !20
  %5 = load ptr, ptr %3, align 8, !tbaa !16
  %6 = getelementptr inbounds nuw %struct.Osc, ptr %5, i32 0, i32 2
  %7 = load i32, ptr %6, align 4, !tbaa !24
  %8 = load i32, ptr %4, align 4, !tbaa !20
  %9 = icmp sgt i32 %7, %8
  br i1 %9, label %10, label %12

10:                                               ; preds = %2
  %11 = load i32, ptr %4, align 4, !tbaa !20
  br label %17

12:                                               ; preds = %2
  %13 = load ptr, ptr %3, align 8, !tbaa !16
  %14 = getelementptr inbounds nuw %struct.Osc, ptr %13, i32 0, i32 2
  %15 = load i32, ptr %14, align 4, !tbaa !24
  %16 = add nsw i32 %15, 1
  br label %17

17:                                               ; preds = %12, %10
  %18 = phi i32 [ %11, %10 ], [ %16, %12 ]
  ret i32 %18
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #2

attributes #0 = { nounwind optsize uwtable "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #1 = { alwaysinline nounwind optsize uwtable "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #2 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #3 = { optsize }
attributes #4 = { nounwind }

!llvm.module.flags = !{!0, !1, !2, !3, !4, !5, !6, !7, !8, !9}
!llvm.ident = !{!10}
!llvm.errno.tbaa = !{!11}

!0 = !{i32 1, !"ptrauth-elf-got", i32 0}
!1 = !{i32 1, !"ptrauth-init-fini", i32 0}
!2 = !{i32 1, !"ptrauth-init-fini-address-discrimination", i32 0}
!3 = !{i32 1, !"ptrauth-sign-personality", i32 0}
!4 = !{i32 1, !"aarch64-elf-pauthabi-platform", i32 268435458}
!5 = !{i32 1, !"aarch64-elf-pauthabi-version", i32 0}
!6 = !{i32 8, !"PIC Level", i32 2}
!7 = !{i32 7, !"PIE Level", i32 2}
!8 = !{i32 7, !"uwtable", i32 2}
!9 = !{i32 7, !"frame-pointer", i32 4}
!10 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!11 = !{!12, !13, i64 0}
!12 = !{!"__libc_errno", !13, i64 0}
!13 = !{!"int", !14, i64 0}
!14 = !{!"omnipotent char", !15, i64 0}
!15 = !{!"Simple C/C++ TBAA"}
!16 = !{!17, !17, i64 0}
!17 = !{!"any pointer", !14, i64 0}
!18 = !{!19, !19, i64 0}
!19 = !{!"p1 int", !17, i64 0}
!20 = !{!13, !13, i64 0}
!21 = !{!22, !13, i64 20}
!22 = !{!"", !13, i64 0, !13, i64 4, !13, i64 8, !13, i64 12, !13, i64 16, !13, i64 20, !13, i64 24, !13, i64 28}
!23 = !{i64 0, i64 4, !20, i64 4, i64 4, !20, i64 8, i64 4, !20, i64 12, i64 4, !20, i64 16, i64 4, !20, i64 20, i64 4, !20, i64 24, i64 4, !20, i64 28, i64 4, !20}
!24 = !{!22, !13, i64 8}
!25 = !{!22, !13, i64 24}
!26 = !{!22, !13, i64 28}
!27 = !{!22, !13, i64 0}
!28 = !{!22, !13, i64 4}
!29 = !{!22, !13, i64 16}
!30 = !{!22, !13, i64 12}
!31 = distinct !{!31, !32}
!32 = !{!"llvm.loop.mustprogress"}
