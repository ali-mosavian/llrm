target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"
; ModuleID = '/home/alim/scratch/z80w/red/u_clear.c'
source_filename = "/home/alim/scratch/z80w/red/u_clear.c"
target triple = "msp430"

@a = dso_local global [64 x i16] zeroinitializer, align 2
@b = dso_local global [64 x i16] zeroinitializer, align 2

; Function Attrs: nounwind
define dso_local void @clear() #0 {
  %1 = alloca i16, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #2
  store i16 0, ptr %1, align 2, !tbaa !7
  br label %2

2:                                                ; preds = %14, %0
  %3 = load i16, ptr %1, align 2, !tbaa !7
  %4 = icmp slt i16 %3, 64
  br i1 %4, label %5, label %17

5:                                                ; preds = %2
  %6 = load i16, ptr %1, align 2, !tbaa !7
  %7 = add nsw i16 100, %6
  %8 = load i16, ptr %1, align 2, !tbaa !7
  %9 = getelementptr inbounds [64 x i16], ptr @a, i16 0, i16 %8
  store i16 %7, ptr %9, align 2, !tbaa !7
  %10 = load i16, ptr %1, align 2, !tbaa !7
  %11 = add nsw i16 200, %10
  %12 = load i16, ptr %1, align 2, !tbaa !7
  %13 = getelementptr inbounds [64 x i16], ptr @b, i16 0, i16 %12
  store i16 %11, ptr %13, align 2, !tbaa !7
  br label %14

14:                                               ; preds = %5
  %15 = load i16, ptr %1, align 2, !tbaa !7
  %16 = add nsw i16 %15, 1
  store i16 %16, ptr %1, align 2, !tbaa !7
  br label %2, !llvm.loop !9

17:                                               ; preds = %2
  call void @llvm.lifetime.end.p0(ptr %1) #2
  ret void
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

attributes #0 = { nounwind "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { nounwind }

!llvm.module.flags = !{!0}
!llvm.ident = !{!1}
!llvm.errno.tbaa = !{!2}

!0 = !{i32 1, !"wchar_size", i32 2}
!1 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!2 = !{!3, !4, i64 0}
!3 = !{!"__libc_errno", !4, i64 0}
!4 = !{!"int", !5, i64 0}
!5 = !{!"omnipotent char", !6, i64 0}
!6 = !{!"Simple C/C++ TBAA"}
!7 = !{!8, !8, i64 0}
!8 = !{!"short", !5, i64 0}
!9 = distinct !{!9, !10, !11}
!10 = !{!"llvm.loop.mustprogress"}
!11 = !{!"llvm.loop.unroll.disable"}
