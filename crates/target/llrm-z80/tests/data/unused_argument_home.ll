target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"
; ModuleID = '/home/alim/work/personal/llrm/.claude/worktrees/elated-mclaren-9dfa1f/tests/run/c/i64_param_home.c'
source_filename = "/home/alim/work/personal/llrm/.claude/worktrees/elated-mclaren-9dfa1f/tests/run/c/i64_param_home.c"
target triple = "msp430"

%union.U = type { i64 }

; Function Attrs: nounwind
define dso_local i16 @through_pointer(i16 noundef %0, i64 noundef %1, i16 noundef %2) #0 {
  %4 = alloca i16, align 2
  %5 = alloca i64, align 2
  %6 = alloca i16, align 2
  store i16 %0, ptr %4, align 2, !tbaa !7
  store i64 %1, ptr %5, align 2, !tbaa !8
  store i16 %2, ptr %6, align 2, !tbaa !7
  %7 = load i16, ptr %4, align 2, !tbaa !7
  %8 = mul nsw i16 %7, 3
  %9 = getelementptr inbounds i16, ptr %5, i16 1
  %10 = load i16, ptr %9, align 2, !tbaa !7
  %11 = mul nsw i16 %10, 5
  %12 = add nsw i16 %8, %11
  %13 = load i64, ptr %5, align 2, !tbaa !8
  %14 = trunc i64 %13 to i16
  %15 = mul nsw i16 %14, 7
  %16 = add nsw i16 %12, %15
  %17 = load i16, ptr %6, align 2, !tbaa !7
  %18 = mul nsw i16 %17, 11
  %19 = add nsw i16 %16, %18
  ret i16 %19
}

; Function Attrs: nounwind
define dso_local i16 @through_union(i16 noundef %0, i64 noundef %1, i16 noundef %2) #0 {
  %4 = alloca i16, align 2
  %5 = alloca i64, align 2
  %6 = alloca i16, align 2
  %7 = alloca %union.U, align 1
  store i16 %0, ptr %4, align 2, !tbaa !7
  store i64 %1, ptr %5, align 2, !tbaa !8
  store i16 %2, ptr %6, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %7) #3
  %8 = load i64, ptr %5, align 2, !tbaa !8
  store i64 %8, ptr %7, align 1, !tbaa !10
  %9 = load i16, ptr %4, align 2, !tbaa !7
  %10 = mul nsw i16 %9, 3
  %11 = getelementptr inbounds [2 x i16], ptr %7, i16 0, i16 1
  %12 = load i16, ptr %11, align 1, !tbaa !10
  %13 = mul nsw i16 %12, 5
  %14 = add nsw i16 %10, %13
  %15 = getelementptr inbounds [2 x i16], ptr %7, i16 0, i16 0
  %16 = load i16, ptr %15, align 1, !tbaa !10
  %17 = mul nsw i16 %16, 7
  %18 = add nsw i16 %14, %17
  %19 = load i16, ptr %6, align 2, !tbaa !7
  %20 = mul nsw i16 %19, 11
  %21 = add nsw i16 %18, %20
  call void @llvm.lifetime.end.p0(ptr %7) #3
  ret i16 %21
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

; Function Attrs: nounwind
define dso_local i16 @on_the_stack(i16 noundef %0, i16 noundef %1, i16 noundef %2, i16 noundef %3, i64 noundef %4) #0 {
  %6 = alloca i16, align 2
  %7 = alloca i16, align 2
  %8 = alloca i16, align 2
  %9 = alloca i16, align 2
  %10 = alloca i64, align 2
  store i16 %0, ptr %6, align 2, !tbaa !7
  store i16 %1, ptr %7, align 2, !tbaa !7
  store i16 %2, ptr %8, align 2, !tbaa !7
  store i16 %3, ptr %9, align 2, !tbaa !7
  store i64 %4, ptr %10, align 2, !tbaa !8
  %11 = load i16, ptr %6, align 2, !tbaa !7
  %12 = load i16, ptr %7, align 2, !tbaa !7
  %13 = add nsw i16 %11, %12
  %14 = load i16, ptr %8, align 2, !tbaa !7
  %15 = add nsw i16 %13, %14
  %16 = load i16, ptr %9, align 2, !tbaa !7
  %17 = add nsw i16 %15, %16
  %18 = getelementptr inbounds i16, ptr %10, i16 1
  %19 = load i16, ptr %18, align 2, !tbaa !7
  %20 = mul nsw i16 %19, 3
  %21 = add nsw i16 %17, %20
  %22 = load i64, ptr %10, align 2, !tbaa !8
  %23 = trunc i64 %22 to i16
  %24 = mul nsw i16 %23, 5
  %25 = add nsw i16 %21, %24
  ret i16 %25
}

; Function Attrs: nounwind
define dso_local i16 @explicit_cdecl(i16 noundef %0, i64 noundef %1) #0 {
  %3 = alloca i16, align 2
  %4 = alloca i64, align 2
  store i16 %0, ptr %3, align 2, !tbaa !7
  store i64 %1, ptr %4, align 2, !tbaa !8
  %5 = getelementptr inbounds i16, ptr %4, i16 1
  %6 = load i16, ptr %5, align 2, !tbaa !7
  %7 = mul nsw i16 %6, 2
  %8 = load i64, ptr %4, align 2, !tbaa !8
  %9 = trunc i64 %8 to i16
  %10 = add nsw i16 %7, %9
  %11 = load i16, ptr %3, align 2, !tbaa !7
  %12 = add nsw i16 %10, %11
  ret i16 %12
}

; Function Attrs: nounwind
define dso_local i16 @by_bytes(i64 noundef %0) #0 {
  %2 = alloca i64, align 2
  %3 = alloca ptr, align 2
  store i64 %0, ptr %2, align 2, !tbaa !8
  call void @llvm.lifetime.start.p0(ptr %3) #3
  store ptr %2, ptr %3, align 2, !tbaa !11
  %4 = load ptr, ptr %3, align 2, !tbaa !11
  %5 = getelementptr inbounds i8, ptr %4, i16 4
  %6 = load i8, ptr %5, align 1, !tbaa !10
  %7 = zext i8 %6 to i16
  %8 = load ptr, ptr %3, align 2, !tbaa !11
  %9 = getelementptr inbounds i8, ptr %8, i16 5
  %10 = load i8, ptr %9, align 1, !tbaa !10
  %11 = zext i8 %10 to i16
  %12 = shl i16 %11, 8
  %13 = add nsw i16 %7, %12
  %14 = mul nsw i16 %13, 5
  %15 = load ptr, ptr %3, align 2, !tbaa !11
  %16 = getelementptr inbounds i8, ptr %15, i16 0
  %17 = load i8, ptr %16, align 1, !tbaa !10
  %18 = zext i8 %17 to i16
  %19 = add nsw i16 %14, %18
  %20 = load ptr, ptr %3, align 2, !tbaa !11
  %21 = getelementptr inbounds i8, ptr %20, i16 1
  %22 = load i8, ptr %21, align 1, !tbaa !10
  %23 = zext i8 %22 to i16
  %24 = shl i16 %23, 8
  %25 = add nsw i16 %19, %24
  call void @llvm.lifetime.end.p0(ptr %3) #3
  ret i16 %25
}

; Function Attrs: nounwind
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  %2 = alloca i64, align 2
  store i16 0, ptr %1, align 2
  call void @llvm.lifetime.start.p0(ptr %2) #3
  store i64 8589934595, ptr %2, align 2, !tbaa !8
  %3 = load i64, ptr %2, align 2, !tbaa !8
  %4 = call i16 @through_pointer(i16 noundef 1, i64 noundef %3, i16 noundef 4)
  %5 = sext i16 %4 to i32
  call void @report(i32 noundef %5)
  %6 = load i64, ptr %2, align 2, !tbaa !8
  %7 = call i16 @through_union(i16 noundef 1, i64 noundef %6, i16 noundef 4)
  %8 = sext i16 %7 to i32
  call void @report(i32 noundef %8)
  %9 = load i64, ptr %2, align 2, !tbaa !8
  %10 = call i16 @on_the_stack(i16 noundef 1, i16 noundef 2, i16 noundef 3, i16 noundef 4, i64 noundef %9)
  %11 = sext i16 %10 to i32
  call void @report(i32 noundef %11)
  %12 = load i64, ptr %2, align 2, !tbaa !8
  %13 = call i16 @explicit_cdecl(i16 noundef 5, i64 noundef %12)
  %14 = sext i16 %13 to i32
  call void @report(i32 noundef %14)
  %15 = load i64, ptr %2, align 2, !tbaa !8
  %16 = call i16 @by_bytes(i64 noundef %15)
  %17 = sext i16 %16 to i32
  call void @report(i32 noundef %17)
  call void @llvm.lifetime.end.p0(ptr %2) #3
  ret i16 0
}

declare dso_local void @report(i32 noundef) #2

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
!9 = !{!"long long", !5, i64 0}
!10 = !{!5, !5, i64 0}
!11 = !{!12, !12, i64 0}
!12 = !{!"p1 omnipotent char", !13, i64 0}
!13 = !{!"any pointer", !5, i64 0}
