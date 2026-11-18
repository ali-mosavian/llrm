; ModuleID = 'bench/recgcd/recgcd.c'
source_filename = "bench/recgcd/recgcd.c"
target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
target triple = "m68k-unknown-linux-gnu"

; Function Attrs: nounwind
define dso_local i32 @bench_recgcd(i32 noundef %0) #0 {
  %2 = alloca i32, align 2
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  store i32 %0, ptr %2, align 2, !tbaa !8
  call void @llvm.lifetime.start.p0(ptr %3) #3
  store i32 0, ptr %3, align 2, !tbaa !9
  call void @llvm.lifetime.start.p0(ptr %4) #3
  store i32 1, ptr %4, align 2, !tbaa !8
  br label %5

5:                                                ; preds = %23, %1
  %6 = load i32, ptr %4, align 2, !tbaa !8
  %7 = load i32, ptr %2, align 2, !tbaa !8
  %8 = icmp sle i32 %6, %7
  br i1 %8, label %9, label %26

9:                                                ; preds = %5
  %10 = load i32, ptr %4, align 2, !tbaa !8
  %11 = mul nsw i32 %10, 7
  %12 = add nsw i32 %11, 3
  %13 = load i32, ptr %4, align 2, !tbaa !8
  %14 = mul nsw i32 %13, 5
  %15 = add nsw i32 %14, 2
  %16 = call i32 @gcd(i32 noundef %12, i32 noundef %15) #4
  %17 = load i32, ptr %4, align 2, !tbaa !8
  %18 = add nsw i32 %17, 90
  %19 = call i32 @gcd(i32 noundef %18, i32 noundef 360) #4
  %20 = add nsw i32 %16, %19
  %21 = load i32, ptr %3, align 2, !tbaa !9
  %22 = add nsw i32 %21, %20
  store i32 %22, ptr %3, align 2, !tbaa !9
  br label %23

23:                                               ; preds = %9
  %24 = load i32, ptr %4, align 2, !tbaa !8
  %25 = add nsw i32 %24, 1
  store i32 %25, ptr %4, align 2, !tbaa !8
  br label %5, !llvm.loop !11

26:                                               ; preds = %5
  %27 = load i32, ptr %3, align 2, !tbaa !9
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  ret i32 %27
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nounwind
define internal i32 @gcd(i32 noundef %0, i32 noundef %1) #0 {
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  %5 = alloca i32, align 2
  store i32 %0, ptr %4, align 2, !tbaa !8
  store i32 %1, ptr %5, align 2, !tbaa !8
  %6 = load i32, ptr %5, align 2, !tbaa !8
  %7 = icmp eq i32 %6, 0
  br i1 %7, label %8, label %10

8:                                                ; preds = %2
  %9 = load i32, ptr %4, align 2, !tbaa !8
  store i32 %9, ptr %3, align 2
  br label %16

10:                                               ; preds = %2
  %11 = load i32, ptr %5, align 2, !tbaa !8
  %12 = load i32, ptr %4, align 2, !tbaa !8
  %13 = load i32, ptr %5, align 2, !tbaa !8
  %14 = srem i32 %12, %13
  %15 = call i32 @gcd(i32 noundef %11, i32 noundef %14) #4
  store i32 %15, ptr %3, align 2
  br label %16

16:                                               ; preds = %10, %8
  %17 = load i32, ptr %3, align 2
  ret i32 %17
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

; Function Attrs: nounwind
define dso_local i32 @main() #0 {
  %1 = call i32 @bench_recgcd(i32 noundef 300) #4
  call void @report(i32 noundef %1) #4
  ret i32 0
}

declare void @report(i32 noundef) #2

attributes #0 = { nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68020" "target-features"="+isa-68881" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68020" "target-features"="+isa-68881" }
attributes #3 = { nounwind }
attributes #4 = { nobuiltin "no-builtins" }

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
!9 = !{!10, !10, i64 0}
!10 = !{!"long", !6, i64 0}
!11 = distinct !{!11, !12, !13}
!12 = !{!"llvm.loop.mustprogress"}
!13 = !{!"llvm.loop.unroll.disable"}
