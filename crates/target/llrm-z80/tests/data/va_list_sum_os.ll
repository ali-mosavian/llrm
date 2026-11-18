; ModuleID = 'va.c'
source_filename = "va.c"
target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"
target triple = "msp430"

; Function Attrs: nounwind optsize
define dso_local i16 @sum(i16 noundef %0, ...) #0 {
  %2 = alloca i16, align 2
  %3 = alloca ptr, align 2
  %4 = alloca i16, align 2
  %5 = alloca i16, align 2
  store i16 %0, ptr %2, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %3) #3
  call void @llvm.lifetime.start.p0(ptr %4) #3
  store i16 0, ptr %4, align 2, !tbaa !7
  call void @llvm.va_start.p0(ptr %3)
  br label %6

6:                                                ; preds = %10, %1
  %7 = load i16, ptr %2, align 2, !tbaa !7
  %8 = add nsw i16 %7, -1
  store i16 %8, ptr %2, align 2, !tbaa !7
  %9 = icmp sgt i16 %7, 0
  br i1 %9, label %10, label %15

10:                                               ; preds = %6
  %11 = va_arg ptr %3, i16
  store i16 %11, ptr %5, align 2
  %12 = load i16, ptr %5, align 2, !tbaa !7
  %13 = load i16, ptr %4, align 2, !tbaa !7
  %14 = add nsw i16 %13, %12
  store i16 %14, ptr %4, align 2, !tbaa !7
  br label %6, !llvm.loop !8

15:                                               ; preds = %6
  call void @llvm.va_end.p0(ptr %3)
  %16 = load i16, ptr %4, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  ret i16 %16
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nocallback nofree nosync nounwind willreturn
declare void @llvm.va_start.p0(ptr) #2

; Function Attrs: nocallback nofree nosync nounwind willreturn
declare void @llvm.va_end.p0(ptr) #2

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

attributes #0 = { nounwind optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { nocallback nofree nosync nounwind willreturn }
attributes #3 = { nounwind }

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
!7 = !{!4, !4, i64 0}
!8 = distinct !{!8, !9}
!9 = !{!"llvm.loop.mustprogress"}
