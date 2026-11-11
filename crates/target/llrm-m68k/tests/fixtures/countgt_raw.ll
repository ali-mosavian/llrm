; ModuleID = '/home/alim/scratch/mfext/crates/target/llrm-m68k/bench/kernels/countgt.c'
source_filename = "/home/alim/scratch/mfext/crates/target/llrm-m68k/bench/kernels/countgt.c"
target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
target triple = "m68k-unknown-linux-gnu"

; Function Attrs: nounwind
define dso_local i32 @k(ptr noundef %0, i32 noundef %1, i32 noundef %2) #0 {
  %4 = alloca ptr, align 2
  %5 = alloca i32, align 2
  %6 = alloca i32, align 2
  %7 = alloca i32, align 2
  %8 = alloca i32, align 2
  store ptr %0, ptr %4, align 2, !tbaa !8
  store i32 %1, ptr %5, align 2, !tbaa !11
  store i32 %2, ptr %6, align 2, !tbaa !11
  call void @llvm.lifetime.start.p0(ptr %7) #2
  store i32 0, ptr %7, align 2, !tbaa !11
  call void @llvm.lifetime.start.p0(ptr %8) #2
  store i32 0, ptr %8, align 2, !tbaa !11
  br label %9

9:                                                ; preds = %21, %3
  %10 = load ptr, ptr %4, align 2, !tbaa !8
  %11 = load i32, ptr %8, align 2, !tbaa !11
  %12 = getelementptr inbounds i32, ptr %10, i32 %11
  %13 = load i32, ptr %12, align 2, !tbaa !11
  %14 = load i32, ptr %6, align 2, !tbaa !11
  %15 = icmp sgt i32 %13, %14
  %16 = zext i1 %15 to i32
  %17 = load i32, ptr %7, align 2, !tbaa !11
  %18 = add nsw i32 %17, %16
  store i32 %18, ptr %7, align 2, !tbaa !11
  %19 = load i32, ptr %8, align 2, !tbaa !11
  %20 = add nsw i32 %19, 1
  store i32 %20, ptr %8, align 2, !tbaa !11
  br label %21

21:                                               ; preds = %9
  %22 = load i32, ptr %8, align 2, !tbaa !11
  %23 = load i32, ptr %5, align 2, !tbaa !11
  %24 = icmp slt i32 %22, %23
  br i1 %24, label %9, label %25, !llvm.loop !12

25:                                               ; preds = %21
  %26 = load i32, ptr %7, align 2, !tbaa !11
  call void @llvm.lifetime.end.p0(ptr %8) #2
  call void @llvm.lifetime.end.p0(ptr %7) #2
  ret i32 %26
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
!9 = !{!"p1 int", !10, i64 0}
!10 = !{!"any pointer", !6, i64 0}
!11 = !{!5, !5, i64 0}
!12 = distinct !{!12, !13}
!13 = !{!"llvm.loop.mustprogress"}
