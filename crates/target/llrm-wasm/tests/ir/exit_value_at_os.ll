; ModuleID = 'pl.c'
source_filename = "pl.c"
target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32"

; Function Attrs: nounwind optsize
define hidden i32 @parity_loop(ptr noundef %0, ptr noundef %1) #0 {
  %3 = alloca ptr, align 4
  %4 = alloca ptr, align 4
  %5 = alloca i32, align 4
  %6 = alloca i16, align 2
  store ptr %0, ptr %3, align 4, !tbaa !6
  store ptr %1, ptr %4, align 4, !tbaa !6
  call void @llvm.lifetime.start.p0(ptr %5) #2
  call void @llvm.lifetime.start.p0(ptr %6) #2
  %7 = load ptr, ptr %4, align 4, !tbaa !6
  %8 = load i16, ptr %7, align 2, !tbaa !9
  %9 = sext i16 %8 to i32
  store i32 %9, ptr %5, align 4, !tbaa !11
  store i16 0, ptr %6, align 2, !tbaa !9
  br label %10

10:                                               ; preds = %17, %2
  %11 = load i16, ptr %6, align 2, !tbaa !9
  %12 = sext i16 %11 to i32
  %13 = load ptr, ptr %3, align 4, !tbaa !6
  %14 = load i16, ptr %13, align 2, !tbaa !9
  %15 = sext i16 %14 to i32
  %16 = icmp slt i32 %12, %15
  br i1 %16, label %17, label %29

17:                                               ; preds = %10
  %18 = load i16, ptr %6, align 2, !tbaa !9
  %19 = sext i16 %18 to i32
  %20 = load ptr, ptr %4, align 4, !tbaa !6
  %21 = load i16, ptr %20, align 2, !tbaa !9
  %22 = sext i16 %21 to i32
  %23 = mul nsw i32 %19, %22
  %24 = add nsw i32 %23, 3
  %25 = load i32, ptr %5, align 4, !tbaa !11
  %26 = add nsw i32 %25, %24
  store i32 %26, ptr %5, align 4, !tbaa !11
  %27 = load i16, ptr %6, align 2, !tbaa !9
  %28 = add i16 %27, 1
  store i16 %28, ptr %6, align 2, !tbaa !9
  br label %10, !llvm.loop !13

29:                                               ; preds = %10
  %30 = load i32, ptr %5, align 4, !tbaa !11
  call void @llvm.lifetime.end.p0(ptr %6) #2
  call void @llvm.lifetime.end.p0(ptr %5) #2
  ret i32 %30
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

attributes #0 = { nounwind optsize "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+bulk-memory,+bulk-memory-opt,+call-indirect-overlong,+multivalue,+mutable-globals,+nontrapping-fptoint,+reference-types,+sign-ext" }
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
!7 = !{!"p1 short", !8, i64 0}
!8 = !{!"any pointer", !4, i64 0}
!9 = !{!10, !10, i64 0}
!10 = !{!"short", !4, i64 0}
!11 = !{!12, !12, i64 0}
!12 = !{!"long", !4, i64 0}
!13 = distinct !{!13, !14}
!14 = !{!"llvm.loop.mustprogress"}
