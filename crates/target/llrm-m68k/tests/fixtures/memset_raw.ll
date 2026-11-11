; ModuleID = '/home/alim/scratch/mfext/crates/target/llrm-m68k/bench/library/memset.c'
source_filename = "/home/alim/scratch/mfext/crates/target/llrm-m68k/bench/library/memset.c"
target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
target triple = "m68k-unknown-linux-gnu"

; Function Attrs: nounwind
define dso_local i32 @k(ptr noundef %0, i32 noundef %1, i32 noundef %2) #0 {
  %4 = alloca ptr, align 2
  %5 = alloca i32, align 2
  %6 = alloca i32, align 2
  %7 = alloca i32, align 2
  store ptr %0, ptr %4, align 2, !tbaa !8
  store i32 %1, ptr %5, align 2, !tbaa !11
  store i32 %2, ptr %6, align 2, !tbaa !11
  call void @llvm.lifetime.start.p0(ptr %7) #2
  store i32 0, ptr %7, align 2, !tbaa !11
  br label %8

8:                                                ; preds = %19, %3
  %9 = load i32, ptr %7, align 2, !tbaa !11
  %10 = load i32, ptr %6, align 2, !tbaa !11
  %11 = icmp slt i32 %9, %10
  br i1 %11, label %13, label %12

12:                                               ; preds = %8
  call void @llvm.lifetime.end.p0(ptr %7) #2
  br label %22

13:                                               ; preds = %8
  %14 = load i32, ptr %5, align 2, !tbaa !11
  %15 = trunc i32 %14 to i8
  %16 = load ptr, ptr %4, align 2, !tbaa !8
  %17 = load i32, ptr %7, align 2, !tbaa !11
  %18 = getelementptr inbounds i8, ptr %16, i32 %17
  store i8 %15, ptr %18, align 1, !tbaa !12
  br label %19

19:                                               ; preds = %13
  %20 = load i32, ptr %7, align 2, !tbaa !11
  %21 = add nsw i32 %20, 1
  store i32 %21, ptr %7, align 2, !tbaa !11
  br label %8, !llvm.loop !13

22:                                               ; preds = %12
  %23 = load ptr, ptr %4, align 2, !tbaa !8
  %24 = load i32, ptr %6, align 2, !tbaa !11
  %25 = sub nsw i32 %24, 1
  %26 = getelementptr inbounds i8, ptr %23, i32 %25
  %27 = load i8, ptr %26, align 1, !tbaa !12
  %28 = sext i8 %27 to i32
  %29 = load ptr, ptr %4, align 2, !tbaa !8
  %30 = getelementptr inbounds i8, ptr %29, i32 0
  %31 = load i8, ptr %30, align 1, !tbaa !12
  %32 = sext i8 %31 to i32
  %33 = add nsw i32 %28, %32
  ret i32 %33
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

attributes #0 = { nounwind "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
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
!9 = !{!"p1 omnipotent char", !10, i64 0}
!10 = !{!"any pointer", !6, i64 0}
!11 = !{!5, !5, i64 0}
!12 = !{!6, !6, i64 0}
!13 = distinct !{!13, !14}
!14 = !{!"llvm.loop.mustprogress"}
