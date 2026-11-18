; ModuleID = 'gb2.c'
source_filename = "gb2.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

@bitcnt = internal unnamed_addr global i32 0, align 4
@bitbuf = internal unnamed_addr global i32 0, align 4
@bad = internal unnamed_addr global i1 false, align 4
@inpos = internal unnamed_addr global i32 0, align 4
@inlen = internal unnamed_addr global i32 0, align 4
@inbuf = internal global [512 x i8] zeroinitializer, align 1

; Function Attrs: noinline nounwind
define dso_local range(i32 0, 2) i32 @getbit() local_unnamed_addr #0 {
  %1 = load i32, ptr @bitcnt, align 4, !tbaa !14
  %2 = icmp eq i32 %1, 0
  br i1 %2, label %3, label %34

3:                                                ; preds = %0
  %4 = load i32, ptr @inpos, align 4, !tbaa !14
  %5 = load i32, ptr @inlen, align 4, !tbaa !14
  %6 = icmp eq i32 %4, %5
  br i1 %6, label %7, label %11

7:                                                ; preds = %3
  %8 = tail call i32 @bench_read(i32 noundef 0, ptr noundef nonnull @inbuf, i32 noundef 512) #3
  store i32 %8, ptr @inlen, align 4, !tbaa !14
  store i32 0, ptr @inpos, align 4, !tbaa !14
  %9 = icmp slt i32 %8, 1
  br i1 %9, label %10, label %11

10:                                               ; preds = %7
  store i32 0, ptr @inlen, align 4, !tbaa !14
  store i1 true, ptr @bad, align 4
  store i32 0, ptr @bitbuf, align 4, !tbaa !14
  br label %34

11:                                               ; preds = %3, %7
  %12 = phi i32 [ %8, %7 ], [ %5, %3 ]
  %13 = phi i32 [ 0, %7 ], [ %4, %3 ]
  %14 = add nsw i32 %13, 1
  store i32 %14, ptr @inpos, align 4, !tbaa !14
  %15 = sext i32 %13 to i64
  %16 = getelementptr inbounds i8, ptr @inbuf, i64 %15
  %17 = load i8, ptr %16, align 1, !tbaa !15
  %18 = zext i8 %17 to i32
  store i32 %18, ptr @bitbuf, align 4, !tbaa !14
  %19 = icmp eq i8 %17, -1
  br i1 %19, label %20, label %34

20:                                               ; preds = %11
  %21 = icmp eq i32 %14, %12
  br i1 %21, label %22, label %26

22:                                               ; preds = %20
  %23 = tail call i32 @bench_read(i32 noundef 0, ptr noundef nonnull @inbuf, i32 noundef 512) #3
  store i32 %23, ptr @inlen, align 4, !tbaa !14
  store i32 0, ptr @inpos, align 4, !tbaa !14
  %24 = icmp slt i32 %23, 1
  br i1 %24, label %25, label %26

25:                                               ; preds = %22
  store i32 0, ptr @inlen, align 4, !tbaa !14
  store i1 true, ptr @bad, align 4
  br label %34

26:                                               ; preds = %20, %22
  %27 = phi i32 [ 0, %22 ], [ %14, %20 ]
  %28 = add nsw i32 %27, 1
  store i32 %28, ptr @inpos, align 4, !tbaa !14
  %29 = sext i32 %27 to i64
  %30 = getelementptr inbounds i8, ptr @inbuf, i64 %29
  %31 = load i8, ptr %30, align 1, !tbaa !15
  %32 = icmp eq i8 %31, 0
  br i1 %32, label %34, label %33

33:                                               ; preds = %26
  store i1 true, ptr @bad, align 4
  br label %34

34:                                               ; preds = %11, %26, %33, %10, %25, %0
  %35 = phi i32 [ %1, %0 ], [ 8, %25 ], [ 8, %10 ], [ 8, %33 ], [ 8, %26 ], [ 8, %11 ]
  %36 = add nsw i32 %35, -1
  store i32 %36, ptr @bitcnt, align 4, !tbaa !14
  %37 = load i32, ptr @bitbuf, align 4, !tbaa !14
  %38 = lshr i32 %37, %36
  %39 = and i32 %38, 1
  ret i32 %39
}

; Function Attrs: nounwind
define dso_local range(i32 0, 3) i32 @use() local_unnamed_addr #1 {
  %1 = tail call i32 @getbit() #4
  %2 = load i1, ptr @bad, align 4
  %3 = zext i1 %2 to i32
  %4 = add nuw nsw i32 %1, %3
  ret i32 %4
}

declare i32 @bench_read(i32 noundef, ptr noundef, i32 noundef) local_unnamed_addr #2

attributes #0 = { noinline nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #1 = { nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #2 = { "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #3 = { nobuiltin nounwind "no-builtins" }
attributes #4 = { nobuiltin "no-builtins" }

!llvm.module.flags = !{!0, !1, !2, !3, !4, !5, !6, !7}
!llvm.ident = !{!8}
!llvm.errno.tbaa = !{!9}

!0 = !{i32 1, !"ptrauth-elf-got", i32 0}
!1 = !{i32 1, !"ptrauth-init-fini", i32 0}
!2 = !{i32 1, !"ptrauth-init-fini-address-discrimination", i32 0}
!3 = !{i32 1, !"ptrauth-sign-personality", i32 0}
!4 = !{i32 1, !"aarch64-elf-pauthabi-platform", i32 268435458}
!5 = !{i32 1, !"aarch64-elf-pauthabi-version", i32 0}
!6 = !{i32 8, !"PIC Level", i32 2}
!7 = !{i32 7, !"PIE Level", i32 2}
!8 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!9 = !{!10, !11, i64 0}
!10 = !{!"__libc_errno", !11, i64 0}
!11 = !{!"int", !12, i64 0}
!12 = !{!"omnipotent char", !13, i64 0}
!13 = !{!"Simple C/C++ TBAA"}
!14 = !{!11, !11, i64 0}
!15 = !{!12, !12, i64 0}
