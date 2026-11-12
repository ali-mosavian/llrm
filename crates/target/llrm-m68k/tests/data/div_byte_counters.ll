; ModuleID = '/tmp/claude-1000/-home-alim-work-personal-llrm--claude-worktrees-jolly-rubin-cc7730/10f7b61f-7486-46fe-b0bd-31e18feff05b/scratchpad/u8b.c'
source_filename = "/tmp/claude-1000/-home-alim-work-personal-llrm--claude-worktrees-jolly-rubin-cc7730/10f7b61f-7486-46fe-b0bd-31e18feff05b/scratchpad/u8b.c"
target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
target triple = "m68k-unknown-linux-gnu"

; Function Attrs: nounwind
define dso_local i32 @total() #0 {
  %1 = alloca i32, align 2
  %2 = alloca i32, align 2
  %3 = alloca i32, align 2
  %4 = alloca i8, align 1
  %5 = alloca i8, align 1
  call void @llvm.lifetime.start.p0(ptr %1) #2
  store i32 0, ptr %1, align 2, !tbaa !8
  call void @llvm.lifetime.start.p0(ptr %2) #2
  call void @llvm.lifetime.start.p0(ptr %3) #2
  store i32 0, ptr %2, align 2, !tbaa !10
  br label %6

6:                                                ; preds = %37, %0
  %7 = load i32, ptr %2, align 2, !tbaa !10
  %8 = icmp slt i32 %7, 256
  br i1 %8, label %9, label %40

9:                                                ; preds = %6
  call void @llvm.lifetime.start.p0(ptr %4) #2
  %10 = load i32, ptr %2, align 2, !tbaa !10
  %11 = trunc i32 %10 to i8
  store i8 %11, ptr %4, align 1, !tbaa !11
  store i32 1, ptr %3, align 2, !tbaa !10
  br label %12

12:                                               ; preds = %33, %9
  %13 = load i32, ptr %3, align 2, !tbaa !10
  %14 = icmp slt i32 %13, 256
  br i1 %14, label %15, label %36

15:                                               ; preds = %12
  call void @llvm.lifetime.start.p0(ptr %5) #2
  %16 = load i32, ptr %3, align 2, !tbaa !10
  %17 = trunc i32 %16 to i8
  store i8 %17, ptr %5, align 1, !tbaa !11
  %18 = load i32, ptr %1, align 2, !tbaa !8
  %19 = mul i32 %18, 3
  %20 = load i8, ptr %4, align 1, !tbaa !11
  %21 = zext i8 %20 to i32
  %22 = load i8, ptr %5, align 1, !tbaa !11
  %23 = zext i8 %22 to i32
  %24 = sdiv i32 %21, %23
  %25 = add i32 %19, %24
  %26 = load i8, ptr %4, align 1, !tbaa !11
  %27 = zext i8 %26 to i32
  %28 = load i8, ptr %5, align 1, !tbaa !11
  %29 = zext i8 %28 to i32
  %30 = srem i32 %27, %29
  %31 = mul nsw i32 %30, 11
  %32 = add i32 %25, %31
  store i32 %32, ptr %1, align 2, !tbaa !8
  call void @llvm.lifetime.end.p0(ptr %5) #2
  br label %33

33:                                               ; preds = %15
  %34 = load i32, ptr %3, align 2, !tbaa !10
  %35 = add nsw i32 %34, 17
  store i32 %35, ptr %3, align 2, !tbaa !10
  br label %12, !llvm.loop !12

36:                                               ; preds = %12
  call void @llvm.lifetime.end.p0(ptr %4) #2
  br label %37

37:                                               ; preds = %36
  %38 = load i32, ptr %2, align 2, !tbaa !10
  %39 = add nsw i32 %38, 1
  store i32 %39, ptr %2, align 2, !tbaa !10
  br label %6, !llvm.loop !14

40:                                               ; preds = %6
  %41 = load i32, ptr %1, align 2, !tbaa !8
  call void @llvm.lifetime.end.p0(ptr %3) #2
  call void @llvm.lifetime.end.p0(ptr %2) #2
  call void @llvm.lifetime.end.p0(ptr %1) #2
  ret i32 %41
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

attributes #0 = { nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { nounwind }

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
!12 = distinct !{!12, !13}
!13 = !{!"llvm.loop.mustprogress"}
!14 = distinct !{!14, !13}
