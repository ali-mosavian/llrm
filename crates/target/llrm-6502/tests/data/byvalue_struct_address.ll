; ModuleID = '/home/alim/scratch/wt-rtsym/tests/run/c/byvalue_struct_address.c'
source_filename = "/home/alim/scratch/wt-rtsym/tests/run/c/byvalue_struct_address.c"
target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8"

%struct.baz = type { i16, i16, i16, i16, i16 }

@sum = internal global i16 0, align 2
@__const.main.x = private unnamed_addr constant %struct.baz { i16 1, i16 2, i16 3, i16 4, i16 5 }, align 2

; Function Attrs: nounwind optsize
define dso_local void @bar(ptr noundef %0, i16 noundef %1, i16 noundef %2, i16 noundef %3, i16 noundef %4, i16 noundef %5) #0 {
  %7 = alloca ptr, align 2
  %8 = alloca i16, align 2
  %9 = alloca i16, align 2
  %10 = alloca i16, align 2
  %11 = alloca i16, align 2
  %12 = alloca i16, align 2
  store ptr %0, ptr %7, align 2, !tbaa !7
  store i16 %1, ptr %8, align 2, !tbaa !10
  store i16 %2, ptr %9, align 2, !tbaa !10
  store i16 %3, ptr %10, align 2, !tbaa !10
  store i16 %4, ptr %11, align 2, !tbaa !10
  store i16 %5, ptr %12, align 2, !tbaa !10
  %13 = load ptr, ptr %7, align 2, !tbaa !7
  %14 = getelementptr inbounds nuw %struct.baz, ptr %13, i32 0, i32 0
  %15 = load i16, ptr %14, align 2, !tbaa !11
  %16 = mul nsw i16 %15, 1
  %17 = load ptr, ptr %7, align 2, !tbaa !7
  %18 = getelementptr inbounds nuw %struct.baz, ptr %17, i32 0, i32 1
  %19 = load i16, ptr %18, align 2, !tbaa !13
  %20 = mul nsw i16 %19, 10
  %21 = add nsw i16 %16, %20
  %22 = load ptr, ptr %7, align 2, !tbaa !7
  %23 = getelementptr inbounds nuw %struct.baz, ptr %22, i32 0, i32 2
  %24 = load i16, ptr %23, align 2, !tbaa !14
  %25 = mul nsw i16 %24, 100
  %26 = add nsw i16 %21, %25
  %27 = load ptr, ptr %7, align 2, !tbaa !7
  %28 = getelementptr inbounds nuw %struct.baz, ptr %27, i32 0, i32 3
  %29 = load i16, ptr %28, align 2, !tbaa !15
  %30 = mul nsw i16 %29, 1000
  %31 = add nsw i16 %26, %30
  %32 = load ptr, ptr %7, align 2, !tbaa !7
  %33 = getelementptr inbounds nuw %struct.baz, ptr %32, i32 0, i32 4
  %34 = load i16, ptr %33, align 2, !tbaa !16
  %35 = mul nsw i16 %34, 10000
  %36 = add nsw i16 %31, %35
  %37 = load i16, ptr %8, align 2, !tbaa !10
  %38 = add nsw i16 %36, %37
  %39 = load i16, ptr %9, align 2, !tbaa !10
  %40 = add nsw i16 %38, %39
  %41 = load i16, ptr %10, align 2, !tbaa !10
  %42 = add nsw i16 %40, %41
  %43 = load i16, ptr %11, align 2, !tbaa !10
  %44 = add nsw i16 %42, %43
  %45 = load i16, ptr %12, align 2, !tbaa !10
  %46 = add nsw i16 %44, %45
  store i16 %46, ptr @sum, align 2, !tbaa !10
  ret void
}

; Function Attrs: nounwind optsize
define dso_local void @foo(ptr noundef %0, ptr noundef byval(%struct.baz) align 2 %1, ptr noundef %2) #0 {
  %4 = alloca ptr, align 2
  %5 = alloca ptr, align 2
  store ptr %0, ptr %4, align 2, !tbaa !17
  store ptr %2, ptr %5, align 2, !tbaa !17
  call void @bar(ptr noundef %1, i16 noundef 6, i16 noundef 7, i16 noundef 8, i16 noundef 9, i16 noundef 10) #3
  ret void
}

; Function Attrs: nounwind optsize
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  %2 = alloca %struct.baz, align 2
  store i16 0, ptr %1, align 2
  call void @llvm.lifetime.start.p0(ptr %2) #4
  call void @llvm.memcpy.p0.p0.i16(ptr align 2 %2, ptr align 2 @__const.main.x, i16 10, i1 false)
  call void @foo(ptr noundef null, ptr noundef byval(%struct.baz) align 2 %2, ptr noundef null) #3
  %3 = load i16, ptr @sum, align 2, !tbaa !10
  %4 = sext i16 %3 to i32
  call void @report(i32 noundef %4) #3
  call void @llvm.lifetime.end.p0(ptr %2) #4
  ret i16 0
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.memcpy.p0.p0.i16(ptr noalias writeonly captures(none), ptr noalias readonly captures(none), i16, i1 immarg) #1

; Function Attrs: optsize
declare dso_local void @report(i32 noundef) #2

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

attributes #0 = { nounwind optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #3 = { optsize }
attributes #4 = { nounwind }

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
!8 = !{!"p1 _ZTS3baz", !9, i64 0}
!9 = !{!"any pointer", !5, i64 0}
!10 = !{!4, !4, i64 0}
!11 = !{!12, !4, i64 0}
!12 = !{!"baz", !4, i64 0, !4, i64 2, !4, i64 4, !4, i64 6, !4, i64 8}
!13 = !{!12, !4, i64 2}
!14 = !{!12, !4, i64 4}
!15 = !{!12, !4, i64 6}
!16 = !{!12, !4, i64 8}
!17 = !{!18, !18, i64 0}
!18 = !{!"p1 omnipotent char", !9, i64 0}
