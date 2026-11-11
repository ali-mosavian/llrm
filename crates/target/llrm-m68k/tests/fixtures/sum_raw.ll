; ModuleID = '/home/alim/scratch/mfext/crates/target/llrm-m68k/bench/kernels/sum.c'
source_filename = "/home/alim/scratch/mfext/crates/target/llrm-m68k/bench/kernels/sum.c"
target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
target triple = "m68k-unknown-linux-gnu"

; Function Attrs: nounwind
define dso_local i32 @k(i32 noundef %0) #0 {
  %2 = alloca i32, align 2
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  store i32 %0, ptr %2, align 2, !tbaa !8
  call void @llvm.lifetime.start.p0(ptr %3) #2
  store i32 0, ptr %3, align 2, !tbaa !8
  call void @llvm.lifetime.start.p0(ptr %4) #2
  store i32 0, ptr %4, align 2, !tbaa !8
  br label %5

5:                                                ; preds = %11, %1
  %6 = load i32, ptr %4, align 2, !tbaa !8
  %7 = load i32, ptr %3, align 2, !tbaa !8
  %8 = add nsw i32 %7, %6
  store i32 %8, ptr %3, align 2, !tbaa !8
  %9 = load i32, ptr %4, align 2, !tbaa !8
  %10 = add nsw i32 %9, 1
  store i32 %10, ptr %4, align 2, !tbaa !8
  br label %11

11:                                               ; preds = %5
  %12 = load i32, ptr %4, align 2, !tbaa !8
  %13 = load i32, ptr %2, align 2, !tbaa !8
  %14 = icmp slt i32 %12, %13
  br i1 %14, label %5, label %15, !llvm.loop !9

15:                                               ; preds = %11
  %16 = load i32, ptr %3, align 2, !tbaa !8
  call void @llvm.lifetime.end.p0(ptr %4) #2
  call void @llvm.lifetime.end.p0(ptr %3) #2
  ret i32 %16
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
!8 = !{!5, !5, i64 0}
!9 = distinct !{!9, !10}
!10 = !{!"llvm.loop.mustprogress"}
