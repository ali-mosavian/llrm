; ModuleID = '/home/alim/scratch/mfext/crates/target/llrm-m68k/bench/library/structcopy.c'
source_filename = "/home/alim/scratch/mfext/crates/target/llrm-m68k/bench/library/structcopy.c"
target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
target triple = "m68k-unknown-linux-gnu"

%struct.S = type { i32, i32, i32, i32, i32, i8 }

; Function Attrs: nounwind
define dso_local i32 @k(ptr noundef %0, ptr noundef %1) #0 {
  %3 = alloca ptr, align 2
  %4 = alloca ptr, align 2
  store ptr %0, ptr %3, align 2, !tbaa !8
  store ptr %1, ptr %4, align 2, !tbaa !8
  %5 = load ptr, ptr %3, align 2, !tbaa !8
  %6 = load ptr, ptr %4, align 2, !tbaa !8
  call void @llvm.memcpy.p0.p0.i32(ptr align 2 %5, ptr align 2 %6, i32 22, i1 false), !tbaa.struct !11
  %7 = load ptr, ptr %3, align 2, !tbaa !8
  %8 = getelementptr inbounds nuw %struct.S, ptr %7, i32 0, i32 2
  %9 = load i32, ptr %8, align 2, !tbaa !14
  %10 = load ptr, ptr %3, align 2, !tbaa !8
  %11 = getelementptr inbounds nuw %struct.S, ptr %10, i32 0, i32 5
  %12 = load i8, ptr %11, align 2, !tbaa !16
  %13 = sext i8 %12 to i32
  %14 = add nsw i32 %9, %13
  ret i32 %14
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.memcpy.p0.p0.i32(ptr noalias writeonly captures(none), ptr noalias readonly captures(none), i32, i1 immarg) #1

attributes #0 = { nounwind "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }

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
!9 = !{!"p1 _ZTS1S", !10, i64 0}
!10 = !{!"any pointer", !6, i64 0}
!11 = !{i64 0, i64 4, !12, i64 4, i64 4, !12, i64 8, i64 4, !12, i64 12, i64 4, !12, i64 16, i64 4, !12, i64 20, i64 1, !13}
!12 = !{!5, !5, i64 0}
!13 = !{!6, !6, i64 0}
!14 = !{!15, !5, i64 8}
!15 = !{!"S", !5, i64 0, !5, i64 4, !5, i64 8, !5, i64 12, !5, i64 16, !6, i64 20}
!16 = !{!15, !6, i64 20}
