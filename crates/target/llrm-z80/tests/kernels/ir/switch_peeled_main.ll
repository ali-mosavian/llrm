target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"
; ModuleID = '/home/alim/work/personal/llrm/.claude/worktrees/quizzical-goodall-66c340/tests/run/c/switch_peeled_table.c'
source_filename = "/home/alim/work/personal/llrm/.claude/worktrees/quizzical-goodall-66c340/tests/run/c/switch_peeled_table.c"
target triple = "msp430"

@start = dso_local global i32 0, align 2

; Function Attrs: nounwind
define dso_local i32 @pick(i16 noundef %0, i16 noundef %1) #0 {
  %3 = alloca i32, align 2
  %4 = alloca i16, align 2
  %5 = alloca i16, align 2
  store i16 %0, ptr %4, align 2, !tbaa !7
  store i16 %1, ptr %5, align 2, !tbaa !7
  %6 = load i16, ptr %4, align 2, !tbaa !7
  switch i16 %6, label %14 [
    i16 192, label %7
    i16 194, label %8
    i16 196, label %9
    i16 219, label %10
    i16 193, label %11
    i16 195, label %11
    i16 197, label %11
    i16 198, label %11
    i16 199, label %11
    i16 200, label %11
    i16 201, label %11
    i16 202, label %11
    i16 203, label %11
    i16 204, label %11
    i16 205, label %11
    i16 206, label %11
    i16 207, label %11
    i16 208, label %11
    i16 209, label %11
    i16 210, label %11
    i16 211, label %11
    i16 212, label %11
    i16 213, label %11
    i16 214, label %11
    i16 215, label %11
    i16 216, label %11
    i16 217, label %11
    i16 218, label %11
  ]

7:                                                ; preds = %2
  store i32 3, ptr %3, align 2
  br label %15

8:                                                ; preds = %2
  store i32 4, ptr %3, align 2
  br label %15

9:                                                ; preds = %2
  store i32 2, ptr %3, align 2
  br label %15

10:                                               ; preds = %2
  store i32 1, ptr %3, align 2
  br label %15

11:                                               ; preds = %2, %2, %2, %2, %2, %2, %2, %2, %2, %2, %2, %2, %2, %2, %2, %2, %2, %2, %2, %2, %2, %2, %2, %2
  %12 = load i16, ptr %5, align 2, !tbaa !7
  %13 = sext i16 %12 to i32
  store i32 %13, ptr %3, align 2
  br label %15

14:                                               ; preds = %2
  store i32 0, ptr %3, align 2
  br label %15

15:                                               ; preds = %14, %11, %10, %9, %8, %7
  %16 = load i32, ptr %3, align 2
  ret i32 %16
}

; Function Attrs: nounwind
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  %2 = alloca i32, align 2
  %3 = alloca i16, align 2
  %4 = alloca i32, align 2
  store i16 0, ptr %1, align 2
  call void @llvm.lifetime.start.p0(ptr %2) #3
  store i32 0, ptr %2, align 2, !tbaa !8
  call void @llvm.lifetime.start.p0(ptr %3) #3
  %5 = load volatile i32, ptr @start, align 2, !tbaa !8
  %6 = trunc i32 %5 to i16
  store i16 %6, ptr %3, align 2, !tbaa !7
  br label %7

7:                                                ; preds = %22, %0
  %8 = load i16, ptr %3, align 2, !tbaa !7
  %9 = icmp slt i16 %8, 256
  br i1 %9, label %10, label %25

10:                                               ; preds = %7
  call void @llvm.lifetime.start.p0(ptr %4) #3
  %11 = load i16, ptr %3, align 2, !tbaa !7
  %12 = call i32 @pick(i16 noundef %11, i16 noundef 9)
  store i32 %12, ptr %4, align 2, !tbaa !8
  %13 = load i32, ptr %4, align 2, !tbaa !8
  %14 = icmp ne i32 %13, 0
  br i1 %14, label %15, label %21

15:                                               ; preds = %10
  %16 = load i16, ptr %3, align 2, !tbaa !7
  %17 = mul nsw i16 %16, 16
  %18 = sext i16 %17 to i32
  %19 = load i32, ptr %4, align 2, !tbaa !8
  %20 = add nsw i32 %18, %19
  call void @report(i32 noundef %20)
  br label %21

21:                                               ; preds = %15, %10
  call void @llvm.lifetime.end.p0(ptr %4) #3
  br label %22

22:                                               ; preds = %21
  %23 = load i16, ptr %3, align 2, !tbaa !7
  %24 = add nsw i16 %23, 1
  store i16 %24, ptr %3, align 2, !tbaa !7
  br label %7, !llvm.loop !10

25:                                               ; preds = %7
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  ret i16 0
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

declare dso_local void @report(i32 noundef) #2

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

attributes #0 = { nounwind "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
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
!8 = !{!9, !9, i64 0}
!9 = !{!"long", !5, i64 0}
!10 = distinct !{!10, !11, !12}
!11 = !{!"llvm.loop.mustprogress"}
!12 = !{!"llvm.loop.unroll.disable"}
