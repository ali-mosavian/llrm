; ModuleID = '/home/alim/scratch/w15-bar/out/src/lru.c'
source_filename = "/home/alim/scratch/w15-bar/out/src/lru.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

%struct.Cache = type { ptr, ptr, ptr, ptr, i64, [7 x i16], [7 x i16] }

@bord = internal global [3 x i16] zeroinitializer, align 4
@cache = internal unnamed_addr global %struct.Cache zeroinitializer, align 8
@bprev = internal global [3 x i16] zeroinitializer, align 4
@bnext = internal global [3 x i16] zeroinitializer, align 4
@bstamp = internal global [3 x i64] zeroinitializer, align 8
@opq0 = internal global i32 0, align 4

; Function Attrs: mustprogress nofree noinline norecurse nosync nounwind willreturn memory(readwrite, argmem: none, inaccessiblemem: none, target_mem: none)
define dso_local i64 @bench_lru(i16 noundef %0) local_unnamed_addr #0 {
  store ptr @bord, ptr @cache, align 8, !tbaa !13
  store ptr @bprev, ptr getelementptr inbounds nuw (i8, ptr @cache, i64 8), align 8, !tbaa !19
  store ptr @bnext, ptr getelementptr inbounds nuw (i8, ptr @cache, i64 16), align 8, !tbaa !20
  store ptr @bstamp, ptr getelementptr inbounds nuw (i8, ptr @cache, i64 24), align 8, !tbaa !21
  store i64 7, ptr getelementptr inbounds nuw (i8, ptr @cache, i64 32), align 8, !tbaa !22
  store i16 0, ptr @bord, align 4, !tbaa !23
  store i16 -1, ptr @bprev, align 4, !tbaa !23
  store i16 -1, ptr @bnext, align 4, !tbaa !23
  store i64 0, ptr @bstamp, align 8, !tbaa !25
  store i16 -1, ptr getelementptr inbounds nuw (i8, ptr @bprev, i64 2), align 2, !tbaa !23
  store i16 -1, ptr getelementptr inbounds nuw (i8, ptr @bnext, i64 2), align 2, !tbaa !23
  store i64 0, ptr getelementptr inbounds nuw (i8, ptr @bstamp, i64 8), align 8, !tbaa !25
  store i16 0, ptr getelementptr inbounds nuw (i8, ptr @bord, i64 4), align 4, !tbaa !23
  store i16 -1, ptr getelementptr inbounds nuw (i8, ptr @bprev, i64 4), align 4, !tbaa !23
  store i16 -1, ptr getelementptr inbounds nuw (i8, ptr @bnext, i64 4), align 4, !tbaa !23
  store i64 0, ptr getelementptr inbounds nuw (i8, ptr @bstamp, i64 16), align 8, !tbaa !25
  tail call void @llvm.memset.p0.i64(ptr noundef nonnull align 8 dereferenceable(28) getelementptr inbounds nuw (i8, ptr @cache, i64 40), i8 -1, i64 28, i1 false)
  store i16 2, ptr getelementptr inbounds nuw (i8, ptr @bord, i64 2), align 2, !tbaa !23
  store i16 0, ptr getelementptr inbounds nuw (i8, ptr @cache, i64 44), align 4, !tbaa !23
  store i16 0, ptr getelementptr inbounds nuw (i8, ptr @cache, i64 58), align 2, !tbaa !23
  %2 = icmp slt i16 %0, 0
  br i1 %2, label %3, label %10

3:                                                ; preds = %1
  %4 = sext i16 %0 to i64
  %5 = getelementptr inbounds [8 x i8], ptr @bstamp, i64 %4
  %6 = load i64, ptr %5, align 8, !tbaa !25
  %7 = getelementptr inbounds [2 x i8], ptr @bprev, i64 %4
  %8 = load i16, ptr %7, align 2, !tbaa !23
  %9 = mul nsw i64 %6, 3
  br label %45

10:                                               ; preds = %1
  store i64 8, ptr getelementptr inbounds nuw (i8, ptr @cache, i64 32), align 8, !tbaa !22
  %11 = zext nneg i16 %0 to i64
  %12 = getelementptr inbounds nuw [8 x i8], ptr @bstamp, i64 %11
  store i64 8, ptr %12, align 8, !tbaa !25
  %13 = getelementptr inbounds nuw [2 x i8], ptr @bord, i64 %11
  %14 = load i16, ptr %13, align 2, !tbaa !23
  %15 = getelementptr inbounds nuw [2 x i8], ptr @bprev, i64 %11
  %16 = load i16, ptr %15, align 2, !tbaa !23
  %17 = icmp sgt i16 %16, -1
  br i1 %17, label %45, label %18

18:                                               ; preds = %10
  %19 = getelementptr inbounds nuw [2 x i8], ptr @bnext, i64 %11
  %20 = load i16, ptr %19, align 2, !tbaa !23
  %21 = icmp sgt i16 %20, -1
  br i1 %21, label %45, label %22

22:                                               ; preds = %18
  %23 = sext i16 %14 to i64
  %24 = getelementptr inbounds [2 x i8], ptr getelementptr inbounds nuw (i8, ptr @cache, i64 40), i64 %23
  %25 = load i16, ptr %24, align 2, !tbaa !23
  %26 = icmp eq i16 %25, %0
  br i1 %26, label %45, label %27

27:                                               ; preds = %22
  %28 = getelementptr inbounds [2 x i8], ptr getelementptr inbounds nuw (i8, ptr @cache, i64 54), i64 %23
  %29 = load i16, ptr %28, align 2, !tbaa !23
  store i16 %29, ptr %15, align 2, !tbaa !23
  store i16 -1, ptr %19, align 2, !tbaa !23
  %30 = icmp sgt i16 %29, -1
  br i1 %30, label %31, label %34

31:                                               ; preds = %27
  %32 = zext nneg i16 %29 to i64
  %33 = getelementptr inbounds nuw [2 x i8], ptr @bnext, i64 %32
  store i16 %0, ptr %33, align 2, !tbaa !23
  br label %35

34:                                               ; preds = %27
  store i16 %0, ptr %24, align 2, !tbaa !23
  br label %35

35:                                               ; preds = %34, %31
  store i16 %0, ptr %28, align 2, !tbaa !23
  %36 = load i16, ptr @bnext, align 4, !tbaa !23
  %37 = load i16, ptr getelementptr inbounds nuw (i8, ptr @cache, i64 44), align 4, !tbaa !23
  %38 = load i16, ptr getelementptr inbounds nuw (i8, ptr @cache, i64 58), align 2, !tbaa !23
  %39 = sext i16 %36 to i64
  %40 = mul nsw i64 %39, 11
  %41 = sext i16 %37 to i64
  %42 = mul nsw i64 %41, 13
  %43 = sext i16 %38 to i64
  %44 = mul nsw i64 %43, 17
  br label %45

45:                                               ; preds = %3, %10, %18, %22, %35
  %46 = phi i64 [ 0, %3 ], [ 0, %10 ], [ 0, %18 ], [ 0, %22 ], [ %44, %35 ]
  %47 = phi i64 [ 0, %3 ], [ 0, %10 ], [ 0, %18 ], [ 0, %22 ], [ %42, %35 ]
  %48 = phi i64 [ -11, %3 ], [ -11, %10 ], [ -11, %18 ], [ -11, %22 ], [ %40, %35 ]
  %49 = phi i16 [ %8, %3 ], [ %16, %10 ], [ %16, %18 ], [ %16, %22 ], [ %29, %35 ]
  %50 = phi i64 [ %9, %3 ], [ 24, %10 ], [ 24, %18 ], [ 24, %22 ], [ 24, %35 ]
  %51 = phi i64 [ 14, %3 ], [ 15, %10 ], [ 15, %18 ], [ 15, %22 ], [ 15, %35 ]
  %52 = sext i16 %0 to i64
  %53 = sext i16 %49 to i64
  %54 = mul nsw i64 %53, 5
  %55 = getelementptr inbounds [2 x i8], ptr @bnext, i64 %52
  %56 = load i16, ptr %55, align 2, !tbaa !23
  %57 = sext i16 %56 to i64
  %58 = mul nsw i64 %57, 7
  %59 = add i64 %51, %50
  %60 = add i64 %59, %54
  %61 = add i64 %60, %58
  %62 = add nsw i64 %61, %48
  %63 = add nsw i64 %62, %47
  %64 = add nsw i64 %63, %46
  ret i64 %64

}

; Function Attrs: nounwind
define dso_local noundef i32 @main() local_unnamed_addr #1 {
  %1 = load volatile i32, ptr @opq0, align 4, !tbaa !26
  %2 = trunc i32 %1 to i16
  %3 = add i16 %2, 1
  %4 = tail call i64 @bench_lru(i16 noundef %3) #4
  tail call void @report(i64 noundef %4) #5
  ret i32 0
}

declare dso_local void @report(i64 noundef) local_unnamed_addr #2

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: write)
declare void @llvm.memset.p0.i64(ptr writeonly captures(none), i8, i64, i1 immarg) #3


attributes #0 = { mustprogress nofree noinline norecurse nosync nounwind willreturn memory(readwrite, argmem: none, inaccessiblemem: none, target_mem: none) "frame-pointer"="non-leaf-no-reserve" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #1 = { nounwind "frame-pointer"="non-leaf-no-reserve" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #2 = { "frame-pointer"="non-leaf-no-reserve" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #3 = { nocallback nofree nosync nounwind willreturn memory(argmem: write) }
attributes #4 = { nobuiltin "no-builtins" }
attributes #5 = { nobuiltin nounwind "no-builtins" }

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
!13 = !{!14, !15, i64 0}
!14 = !{!"", !15, i64 0, !15, i64 8, !15, i64 16, !17, i64 24, !18, i64 32, !11, i64 40, !11, i64 54}
!15 = !{!"p1 short", !16, i64 0}
!16 = !{!"any pointer", !11, i64 0}
!17 = !{!"p1 long", !16, i64 0}
!18 = !{!"long", !11, i64 0}
!19 = !{!14, !15, i64 8}
!20 = !{!14, !15, i64 16}
!21 = !{!14, !17, i64 24}
!22 = !{!14, !18, i64 32}
!23 = !{!24, !24, i64 0}
!24 = !{!"short", !11, i64 0}
!25 = !{!18, !18, i64 0}
!26 = !{!10, !10, i64 0}
