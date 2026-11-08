; ModuleID = 'os_inline_negative_cost.c'
source_filename = "os_inline_negative_cost.c"
target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32"

%struct.L = type { ptr, i32, i32, i32, i32, [2 x i32], i32 }

@seen = internal global i32 0, align 4

; Function Attrs: nounwind optsize
define hidden void @terms(ptr noundef %0, i32 noundef %1, i32 noundef %2) #0 {
  %4 = alloca ptr, align 4
  %5 = alloca i32, align 4
  %6 = alloca i32, align 4
  store ptr %0, ptr %4, align 4, !tbaa !6
  store i32 %1, ptr %5, align 4, !tbaa !9
  store i32 %2, ptr %6, align 4, !tbaa !9
  %7 = load i32, ptr %5, align 4, !tbaa !9
  %8 = load i32, ptr %6, align 4, !tbaa !9
  %9 = add i32 %7, %8
  %10 = load ptr, ptr %4, align 4, !tbaa !6
  %11 = getelementptr inbounds nuw %struct.L, ptr %10, i32 0, i32 1
  store i32 %9, ptr %11, align 4, !tbaa !10
  %12 = load i32, ptr %5, align 4, !tbaa !9
  %13 = load i32, ptr %6, align 4, !tbaa !9
  %14 = sub nsw i32 %12, %13
  %15 = load ptr, ptr %4, align 4, !tbaa !6
  %16 = getelementptr inbounds nuw %struct.L, ptr %15, i32 0, i32 2
  store i32 %14, ptr %16, align 4, !tbaa !12
  %17 = load ptr, ptr %4, align 4, !tbaa !6
  %18 = getelementptr inbounds nuw %struct.L, ptr %17, i32 0, i32 3
  store i32 4, ptr %18, align 4, !tbaa !13
  %19 = load i32, ptr %5, align 4, !tbaa !9
  %20 = mul nsw i32 %19, 100
  %21 = load i32, ptr %6, align 4, !tbaa !9
  %22 = add i32 %20, %21
  store i32 %22, ptr @seen, align 4, !tbaa !14
  ret void
}

; Function Attrs: nounwind optsize
define hidden void @kernel(ptr noundef %0) #0 {
  %2 = alloca ptr, align 4
  store ptr %0, ptr %2, align 4, !tbaa !6
  %3 = load ptr, ptr %2, align 4, !tbaa !6
  %4 = getelementptr inbounds nuw %struct.L, ptr %3, i32 0, i32 1
  %5 = load i32, ptr %4, align 4, !tbaa !10
  %6 = mul nsw i32 %5, 1000
  %7 = load ptr, ptr %2, align 4, !tbaa !6
  %8 = getelementptr inbounds nuw %struct.L, ptr %7, i32 0, i32 5
  %9 = getelementptr inbounds [2 x i32], ptr %8, i32 0, i32 0
  %10 = load i32, ptr %9, align 4, !tbaa !9
  %11 = add nsw i32 %6, %10
  %12 = load ptr, ptr %2, align 4, !tbaa !6
  %13 = getelementptr inbounds nuw %struct.L, ptr %12, i32 0, i32 5
  %14 = getelementptr inbounds [2 x i32], ptr %13, i32 0, i32 1
  %15 = load i32, ptr %14, align 4, !tbaa !9
  %16 = add nsw i32 %11, %15
  %17 = load ptr, ptr %2, align 4, !tbaa !6
  %18 = getelementptr inbounds nuw %struct.L, ptr %17, i32 0, i32 6
  %19 = load i32, ptr %18, align 4, !tbaa !16
  %20 = add i32 %16, %19
  %21 = load i32, ptr @seen, align 4, !tbaa !14
  %22 = add i32 %20, %21
  call void @report(i32 noundef %22) #3
  ret void
}

; Function Attrs: optsize
declare void @report(i32 noundef) #1

; Function Attrs: nounwind optsize
define hidden void @line(i32 noundef %0, i32 noundef %1, i32 noundef %2) #0 {
  %4 = alloca i32, align 4
  %5 = alloca i32, align 4
  %6 = alloca i32, align 4
  %7 = alloca %struct.L, align 4
  store i32 %0, ptr %4, align 4, !tbaa !9
  store i32 %1, ptr %5, align 4, !tbaa !9
  store i32 %2, ptr %6, align 4, !tbaa !9
  call void @llvm.lifetime.start.p0(ptr %7) #4
  %8 = load i32, ptr %4, align 4, !tbaa !9
  %9 = load i32, ptr %5, align 4, !tbaa !9
  call void @terms(ptr noundef %7, i32 noundef %8, i32 noundef %9) #3
  %10 = load i32, ptr %6, align 4, !tbaa !9
  %11 = getelementptr inbounds nuw %struct.L, ptr %7, i32 0, i32 5
  %12 = getelementptr inbounds [2 x i32], ptr %11, i32 0, i32 1
  store i32 %10, ptr %12, align 4, !tbaa !9
  %13 = getelementptr inbounds nuw %struct.L, ptr %7, i32 0, i32 5
  %14 = getelementptr inbounds [2 x i32], ptr %13, i32 0, i32 0
  store i32 %10, ptr %14, align 4, !tbaa !9
  %15 = getelementptr inbounds nuw %struct.L, ptr %7, i32 0, i32 6
  store i32 0, ptr %15, align 4, !tbaa !16
  call void @kernel(ptr noundef %7) #3
  call void @llvm.lifetime.end.p0(ptr %7) #4
  ret void
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #2

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #2

attributes #0 = { nounwind optsize "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+bulk-memory,+bulk-memory-opt,+call-indirect-overlong,+multivalue,+mutable-globals,+nontrapping-fptoint,+reference-types,+sign-ext" }
attributes #1 = { optsize "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+bulk-memory,+bulk-memory-opt,+call-indirect-overlong,+multivalue,+mutable-globals,+nontrapping-fptoint,+reference-types,+sign-ext" }
attributes #2 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #3 = { nobuiltin optsize "no-builtins" }
attributes #4 = { nounwind }

!llvm.ident = !{!0}
!llvm.errno.tbaa = !{!1}

!0 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!1 = !{!2, !3, i64 0}
!2 = !{!"__libc_errno", !3, i64 0}
!3 = !{!"int", !4, i64 0}
!4 = !{!"omnipotent char", !5, i64 0}
!5 = !{!"Simple C/C++ TBAA"}
!6 = !{!7, !7, i64 0}
!7 = !{!"p1 _ZTS1L", !8, i64 0}
!8 = !{!"any pointer", !4, i64 0}
!9 = !{!3, !3, i64 0}
!10 = !{!11, !3, i64 4}
!11 = !{!"L", !8, i64 0, !3, i64 4, !3, i64 8, !3, i64 12, !3, i64 16, !4, i64 20, !3, i64 28}
!12 = !{!11, !3, i64 8}
!13 = !{!11, !3, i64 12}
!14 = !{!15, !15, i64 0}
!15 = !{!"long", !4, i64 0}
!16 = !{!11, !3, i64 28}
