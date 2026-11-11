; ModuleID = 'src/lru.c'
source_filename = "src/lru.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

%struct.Cache = type { ptr, ptr, ptr, ptr, i64, [7 x i16], [7 x i16] }

@bord = internal global [3 x i16] zeroinitializer, align 2
@cache = internal global %struct.Cache zeroinitializer, align 8
@bprev = internal global [3 x i16] zeroinitializer, align 2
@bnext = internal global [3 x i16] zeroinitializer, align 2
@bstamp = internal global [3 x i64] zeroinitializer, align 8
@opq0 = internal global i32 0, align 4

; Function Attrs: noinline nounwind
define dso_local i64 @bench_lru(i16 noundef %0) #0 {
  %2 = alloca i16, align 2
  %3 = alloca i16, align 2
  store i16 %0, ptr %2, align 2, !tbaa !13
  call void @llvm.lifetime.start.p0(ptr %3) #4
  store ptr @bord, ptr @cache, align 8, !tbaa !15
  store ptr @bprev, ptr getelementptr inbounds nuw (i8, ptr @cache, i64 8), align 8, !tbaa !21
  store ptr @bnext, ptr getelementptr inbounds nuw (i8, ptr @cache, i64 16), align 8, !tbaa !22
  store ptr @bstamp, ptr getelementptr inbounds nuw (i8, ptr @cache, i64 24), align 8, !tbaa !23
  store i64 7, ptr getelementptr inbounds nuw (i8, ptr @cache, i64 32), align 8, !tbaa !24
  store i16 0, ptr %3, align 2, !tbaa !13
  br label %4

4:                                                ; preds = %21, %1
  %5 = load i16, ptr %3, align 2, !tbaa !13
  %6 = sext i16 %5 to i32
  %7 = icmp slt i32 %6, 3
  br i1 %7, label %8, label %24

8:                                                ; preds = %4
  %9 = load i16, ptr %3, align 2, !tbaa !13
  %10 = sext i16 %9 to i64
  %11 = getelementptr inbounds [3 x i16], ptr @bord, i64 0, i64 %10
  store i16 0, ptr %11, align 2, !tbaa !13
  %12 = load i16, ptr %3, align 2, !tbaa !13
  %13 = sext i16 %12 to i64
  %14 = getelementptr inbounds [3 x i16], ptr @bprev, i64 0, i64 %13
  store i16 -1, ptr %14, align 2, !tbaa !13
  %15 = load i16, ptr %3, align 2, !tbaa !13
  %16 = sext i16 %15 to i64
  %17 = getelementptr inbounds [3 x i16], ptr @bnext, i64 0, i64 %16
  store i16 -1, ptr %17, align 2, !tbaa !13
  %18 = load i16, ptr %3, align 2, !tbaa !13
  %19 = sext i16 %18 to i64
  %20 = getelementptr inbounds [3 x i64], ptr @bstamp, i64 0, i64 %19
  store i64 0, ptr %20, align 8, !tbaa !25
  br label %21

21:                                               ; preds = %8
  %22 = load i16, ptr %3, align 2, !tbaa !13
  %23 = add i16 %22, 1
  store i16 %23, ptr %3, align 2, !tbaa !13
  br label %4, !llvm.loop !26

24:                                               ; preds = %4
  store i16 0, ptr %3, align 2, !tbaa !13
  br label %25

25:                                               ; preds = %36, %24
  %26 = load i16, ptr %3, align 2, !tbaa !13
  %27 = sext i16 %26 to i32
  %28 = icmp slt i32 %27, 7
  br i1 %28, label %29, label %39

29:                                               ; preds = %25
  %30 = load i16, ptr %3, align 2, !tbaa !13
  %31 = sext i16 %30 to i64
  %32 = getelementptr inbounds [7 x i16], ptr getelementptr inbounds nuw (i8, ptr @cache, i64 40), i64 0, i64 %31
  store i16 -1, ptr %32, align 2, !tbaa !13
  %33 = load i16, ptr %3, align 2, !tbaa !13
  %34 = sext i16 %33 to i64
  %35 = getelementptr inbounds [7 x i16], ptr getelementptr inbounds nuw (i8, ptr @cache, i64 54), i64 0, i64 %34
  store i16 -1, ptr %35, align 2, !tbaa !13
  br label %36

36:                                               ; preds = %29
  %37 = load i16, ptr %3, align 2, !tbaa !13
  %38 = add i16 %37, 1
  store i16 %38, ptr %3, align 2, !tbaa !13
  br label %25, !llvm.loop !28

39:                                               ; preds = %25
  store i16 2, ptr getelementptr inbounds nuw (i8, ptr @bord, i64 2), align 2, !tbaa !13
  store i16 0, ptr getelementptr inbounds nuw (i8, ptr @cache, i64 44), align 4, !tbaa !13
  store i16 0, ptr getelementptr inbounds nuw (i8, ptr @cache, i64 58), align 2, !tbaa !13
  %40 = load i16, ptr %2, align 2, !tbaa !13
  call void @lruUse(ptr noundef @cache, i16 noundef %40) #5
  %41 = load i64, ptr getelementptr inbounds nuw (i8, ptr @cache, i64 32), align 8, !tbaa !24
  %42 = load i16, ptr %2, align 2, !tbaa !13
  %43 = sext i16 %42 to i64
  %44 = getelementptr inbounds [3 x i64], ptr @bstamp, i64 0, i64 %43
  %45 = load i64, ptr %44, align 8, !tbaa !25
  %46 = mul nsw i64 %45, 3
  %47 = add nsw i64 %41, %46
  %48 = load i16, ptr %2, align 2, !tbaa !13
  %49 = sext i16 %48 to i64
  %50 = getelementptr inbounds [3 x i16], ptr @bprev, i64 0, i64 %49
  %51 = load i16, ptr %50, align 2, !tbaa !13
  %52 = sext i16 %51 to i64
  %53 = mul nsw i64 %52, 5
  %54 = add nsw i64 %47, %53
  %55 = load i16, ptr %2, align 2, !tbaa !13
  %56 = sext i16 %55 to i64
  %57 = getelementptr inbounds [3 x i16], ptr @bnext, i64 0, i64 %56
  %58 = load i16, ptr %57, align 2, !tbaa !13
  %59 = sext i16 %58 to i32
  %60 = add nsw i32 %59, 1
  %61 = sext i32 %60 to i64
  %62 = mul nsw i64 %61, 7
  %63 = add nsw i64 %54, %62
  %64 = load i16, ptr @bnext, align 2, !tbaa !13
  %65 = sext i16 %64 to i64
  %66 = mul nsw i64 %65, 11
  %67 = add nsw i64 %63, %66
  %68 = load i16, ptr getelementptr inbounds nuw (i8, ptr @cache, i64 44), align 4, !tbaa !13
  %69 = sext i16 %68 to i64
  %70 = mul nsw i64 %69, 13
  %71 = add nsw i64 %67, %70
  %72 = load i16, ptr getelementptr inbounds nuw (i8, ptr @cache, i64 58), align 2, !tbaa !13
  %73 = sext i16 %72 to i64
  %74 = mul nsw i64 %73, 17
  %75 = add nsw i64 %71, %74
  call void @llvm.lifetime.end.p0(ptr %3) #4
  ret i64 %75
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nounwind
define internal void @lruUse(ptr noundef %0, i16 noundef %1) #2 {
  %3 = alloca ptr, align 8
  %4 = alloca i16, align 2
  %5 = alloca i16, align 2
  %6 = alloca i16, align 2
  %7 = alloca i32, align 4
  store ptr %0, ptr %3, align 8, !tbaa !29
  store i16 %1, ptr %4, align 2, !tbaa !13
  call void @llvm.lifetime.start.p0(ptr %5) #4
  call void @llvm.lifetime.start.p0(ptr %6) #4
  %8 = load i16, ptr %4, align 2, !tbaa !13
  %9 = sext i16 %8 to i32
  %10 = icmp slt i32 %9, 0
  br i1 %10, label %11, label %12

11:                                               ; preds = %2
  store i32 1, ptr %7, align 4
  br label %106

12:                                               ; preds = %2
  %13 = load ptr, ptr %3, align 8, !tbaa !29
  %14 = getelementptr inbounds nuw %struct.Cache, ptr %13, i32 0, i32 4
  %15 = load i64, ptr %14, align 8, !tbaa !24
  %16 = add nsw i64 %15, 1
  store i64 %16, ptr %14, align 8, !tbaa !24
  %17 = load ptr, ptr %3, align 8, !tbaa !29
  %18 = getelementptr inbounds nuw %struct.Cache, ptr %17, i32 0, i32 3
  %19 = load ptr, ptr %18, align 8, !tbaa !23
  %20 = load i16, ptr %4, align 2, !tbaa !13
  %21 = sext i16 %20 to i64
  %22 = getelementptr inbounds i64, ptr %19, i64 %21
  store i64 %16, ptr %22, align 8, !tbaa !25
  %23 = load ptr, ptr %3, align 8, !tbaa !29
  %24 = getelementptr inbounds nuw %struct.Cache, ptr %23, i32 0, i32 0
  %25 = load ptr, ptr %24, align 8, !tbaa !15
  %26 = load i16, ptr %4, align 2, !tbaa !13
  %27 = sext i16 %26 to i64
  %28 = getelementptr inbounds i16, ptr %25, i64 %27
  %29 = load i16, ptr %28, align 2, !tbaa !13
  store i16 %29, ptr %5, align 2, !tbaa !13
  %30 = load ptr, ptr %3, align 8, !tbaa !29
  %31 = getelementptr inbounds nuw %struct.Cache, ptr %30, i32 0, i32 1
  %32 = load ptr, ptr %31, align 8, !tbaa !21
  %33 = load i16, ptr %4, align 2, !tbaa !13
  %34 = sext i16 %33 to i64
  %35 = getelementptr inbounds i16, ptr %32, i64 %34
  %36 = load i16, ptr %35, align 2, !tbaa !13
  %37 = sext i16 %36 to i32
  %38 = icmp sge i32 %37, 0
  br i1 %38, label %60, label %39

39:                                               ; preds = %12
  %40 = load ptr, ptr %3, align 8, !tbaa !29
  %41 = getelementptr inbounds nuw %struct.Cache, ptr %40, i32 0, i32 2
  %42 = load ptr, ptr %41, align 8, !tbaa !22
  %43 = load i16, ptr %4, align 2, !tbaa !13
  %44 = sext i16 %43 to i64
  %45 = getelementptr inbounds i16, ptr %42, i64 %44
  %46 = load i16, ptr %45, align 2, !tbaa !13
  %47 = sext i16 %46 to i32
  %48 = icmp sge i32 %47, 0
  br i1 %48, label %60, label %49

49:                                               ; preds = %39
  %50 = load ptr, ptr %3, align 8, !tbaa !29
  %51 = getelementptr inbounds nuw %struct.Cache, ptr %50, i32 0, i32 5
  %52 = load i16, ptr %5, align 2, !tbaa !13
  %53 = sext i16 %52 to i64
  %54 = getelementptr inbounds [7 x i16], ptr %51, i64 0, i64 %53
  %55 = load i16, ptr %54, align 2, !tbaa !13
  %56 = sext i16 %55 to i32
  %57 = load i16, ptr %4, align 2, !tbaa !13
  %58 = sext i16 %57 to i32
  %59 = icmp eq i32 %56, %58
  br i1 %59, label %60, label %61

60:                                               ; preds = %49, %39, %12
  store i32 1, ptr %7, align 4
  br label %106

61:                                               ; preds = %49
  %62 = load ptr, ptr %3, align 8, !tbaa !29
  %63 = getelementptr inbounds nuw %struct.Cache, ptr %62, i32 0, i32 6
  %64 = load i16, ptr %5, align 2, !tbaa !13
  %65 = sext i16 %64 to i64
  %66 = getelementptr inbounds [7 x i16], ptr %63, i64 0, i64 %65
  %67 = load i16, ptr %66, align 2, !tbaa !13
  store i16 %67, ptr %6, align 2, !tbaa !13
  %68 = load i16, ptr %6, align 2, !tbaa !13
  %69 = load ptr, ptr %3, align 8, !tbaa !29
  %70 = getelementptr inbounds nuw %struct.Cache, ptr %69, i32 0, i32 1
  %71 = load ptr, ptr %70, align 8, !tbaa !21
  %72 = load i16, ptr %4, align 2, !tbaa !13
  %73 = sext i16 %72 to i64
  %74 = getelementptr inbounds i16, ptr %71, i64 %73
  store i16 %68, ptr %74, align 2, !tbaa !13
  %75 = load ptr, ptr %3, align 8, !tbaa !29
  %76 = getelementptr inbounds nuw %struct.Cache, ptr %75, i32 0, i32 2
  %77 = load ptr, ptr %76, align 8, !tbaa !22
  %78 = load i16, ptr %4, align 2, !tbaa !13
  %79 = sext i16 %78 to i64
  %80 = getelementptr inbounds i16, ptr %77, i64 %79
  store i16 -1, ptr %80, align 2, !tbaa !13
  %81 = load i16, ptr %6, align 2, !tbaa !13
  %82 = sext i16 %81 to i32
  %83 = icmp sge i32 %82, 0
  br i1 %83, label %84, label %92

84:                                               ; preds = %61
  %85 = load i16, ptr %4, align 2, !tbaa !13
  %86 = load ptr, ptr %3, align 8, !tbaa !29
  %87 = getelementptr inbounds nuw %struct.Cache, ptr %86, i32 0, i32 2
  %88 = load ptr, ptr %87, align 8, !tbaa !22
  %89 = load i16, ptr %6, align 2, !tbaa !13
  %90 = sext i16 %89 to i64
  %91 = getelementptr inbounds i16, ptr %88, i64 %90
  store i16 %85, ptr %91, align 2, !tbaa !13
  br label %99

92:                                               ; preds = %61
  %93 = load i16, ptr %4, align 2, !tbaa !13
  %94 = load ptr, ptr %3, align 8, !tbaa !29
  %95 = getelementptr inbounds nuw %struct.Cache, ptr %94, i32 0, i32 5
  %96 = load i16, ptr %5, align 2, !tbaa !13
  %97 = sext i16 %96 to i64
  %98 = getelementptr inbounds [7 x i16], ptr %95, i64 0, i64 %97
  store i16 %93, ptr %98, align 2, !tbaa !13
  br label %99

99:                                               ; preds = %92, %84
  %100 = load i16, ptr %4, align 2, !tbaa !13
  %101 = load ptr, ptr %3, align 8, !tbaa !29
  %102 = getelementptr inbounds nuw %struct.Cache, ptr %101, i32 0, i32 6
  %103 = load i16, ptr %5, align 2, !tbaa !13
  %104 = sext i16 %103 to i64
  %105 = getelementptr inbounds [7 x i16], ptr %102, i64 0, i64 %104
  store i16 %100, ptr %105, align 2, !tbaa !13
  store i32 0, ptr %7, align 4
  br label %106

106:                                              ; preds = %99, %60, %11
  call void @llvm.lifetime.end.p0(ptr %6) #4
  call void @llvm.lifetime.end.p0(ptr %5) #4
  %107 = load i32, ptr %7, align 4
  switch i32 %107, label %109 [
    i32 0, label %108
    i32 1, label %108
  ]

108:                                              ; preds = %106, %106
  ret void

109:                                              ; preds = %106
  unreachable
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

; Function Attrs: nounwind
define dso_local i32 @main() #2 {
  %1 = load volatile i32, ptr @opq0, align 4, !tbaa !30
  %2 = add nsw i32 1, %1
  %3 = trunc i32 %2 to i16
  %4 = call i64 @bench_lru(i16 noundef %3) #5
  call void @report(i64 noundef %4) #5
  ret i32 0
}

declare dso_local void @report(i64 noundef) #3

attributes #0 = { noinline nounwind "frame-pointer"="non-leaf-no-reserve" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { nounwind "frame-pointer"="non-leaf-no-reserve" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #3 = { "frame-pointer"="non-leaf-no-reserve" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #4 = { nounwind }
attributes #5 = { nobuiltin "no-builtins" }

!llvm.module.flags = !{!0, !1, !2, !3, !4, !5, !6}
!llvm.ident = !{!7}
!llvm.errno.tbaa = !{!8}

!0 = !{i32 1, !"ptrauth-elf-got", i32 0}
!1 = !{i32 1, !"ptrauth-init-fini", i32 0}
!2 = !{i32 1, !"ptrauth-init-fini-address-discrimination", i32 0}
!3 = !{i32 1, !"ptrauth-sign-personality", i32 0}
!4 = !{i32 1, !"aarch64-elf-pauthabi-platform", i32 268435458}
!5 = !{i32 1, !"aarch64-elf-pauthabi-version", i32 0}
!6 = !{i32 7, !"frame-pointer", i32 4}
!7 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!8 = !{!9, !10, i64 0}
!9 = !{!"__libc_errno", !10, i64 0}
!10 = !{!"int", !11, i64 0}
!11 = !{!"omnipotent char", !12, i64 0}
!12 = !{!"Simple C/C++ TBAA"}
!13 = !{!14, !14, i64 0}
!14 = !{!"short", !11, i64 0}
!15 = !{!16, !17, i64 0}
!16 = !{!"", !17, i64 0, !17, i64 8, !17, i64 16, !19, i64 24, !20, i64 32, !11, i64 40, !11, i64 54}
!17 = !{!"p1 short", !18, i64 0}
!18 = !{!"any pointer", !11, i64 0}
!19 = !{!"p1 long", !18, i64 0}
!20 = !{!"long", !11, i64 0}
!21 = !{!16, !17, i64 8}
!22 = !{!16, !17, i64 16}
!23 = !{!16, !19, i64 24}
!24 = !{!16, !20, i64 32}
!25 = !{!20, !20, i64 0}
!26 = distinct !{!26, !27}
!27 = !{!"llvm.loop.mustprogress"}
!28 = distinct !{!28, !27}
!29 = !{!18, !18, i64 0}
!30 = !{!10, !10, i64 0}
