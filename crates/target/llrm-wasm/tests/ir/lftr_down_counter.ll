; ModuleID = '/tmp/sd.c'
source_filename = "/tmp/sd.c"
target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32"

@a = hidden global [160 x i16] zeroinitializer, align 16

; Function Attrs: nounwind
define hidden void @scroll_down(i16 noundef signext %0) #0 {
  %2 = alloca i16, align 2
  %3 = alloca i16, align 2
  store i16 %0, ptr %2, align 2, !tbaa !6
  call void @llvm.lifetime.start.p0(ptr %3) #2
  %4 = load i16, ptr %2, align 2, !tbaa !6
  %5 = sext i16 %4 to i32
  %6 = sub nsw i32 %5, 1
  %7 = trunc i32 %6 to i16
  store i16 %7, ptr %3, align 2, !tbaa !6
  br label %8

8:                                                ; preds = %21, %1
  %9 = load i16, ptr %3, align 2, !tbaa !6
  %10 = sext i16 %9 to i32
  %11 = icmp sge i32 %10, 0
  br i1 %11, label %12, label %24

12:                                               ; preds = %8
  %13 = load i16, ptr %3, align 2, !tbaa !6
  %14 = sext i16 %13 to i32
  %15 = getelementptr inbounds [160 x i16], ptr @a, i32 0, i32 %14
  %16 = load i16, ptr %15, align 2, !tbaa !6
  %17 = load i16, ptr %3, align 2, !tbaa !6
  %18 = sext i16 %17 to i32
  %19 = add nsw i32 %18, 16
  %20 = getelementptr inbounds [160 x i16], ptr @a, i32 0, i32 %19
  store i16 %16, ptr %20, align 2, !tbaa !6
  br label %21

21:                                               ; preds = %12
  %22 = load i16, ptr %3, align 2, !tbaa !6
  %23 = add i16 %22, -1
  store i16 %23, ptr %3, align 2, !tbaa !6
  br label %8, !llvm.loop !8

24:                                               ; preds = %8
  call void @llvm.lifetime.end.p0(ptr %3) #2
  ret void
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

attributes #0 = { nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+bulk-memory,+bulk-memory-opt,+call-indirect-overlong,+multivalue,+mutable-globals,+nontrapping-fptoint,+reference-types,+sign-ext" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { nounwind }

!llvm.ident = !{!0}
!llvm.errno.tbaa = !{!1}

!0 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!1 = !{!2, !3, i64 0}
!2 = !{!"__libc_errno", !3, i64 0}
!3 = !{!"int", !4, i64 0}
!4 = !{!"omnipotent char", !5, i64 0}
!5 = !{!"Simple C/C++ TBAA"}
!6 = !{!7, !7, i64 0}
!7 = !{!"short", !4, i64 0}
!8 = distinct !{!8, !9}
!9 = !{!"llvm.loop.mustprogress"}
