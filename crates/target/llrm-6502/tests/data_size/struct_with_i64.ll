target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8"
target triple = "msp430"

%struct.S = type <{ i8, i64 }>

; Function Attrs: nounwind optsize
define dso_local void @divided(ptr dead_on_unwind noalias writable sret(%struct.S) align 1 %0, ptr noundef byval(%struct.S) align 1 %1, ptr noundef byval(%struct.S) align 1 %2) #0 {
  %4 = getelementptr inbounds nuw %struct.S, ptr %1, i32 0, i32 0
  %5 = load i8, ptr %4, align 1, !tbaa !7
  %6 = zext i8 %5 to i16
  %7 = getelementptr inbounds nuw %struct.S, ptr %2, i32 0, i32 0
  %8 = load i8, ptr %7, align 1, !tbaa !7
  %9 = zext i8 %8 to i16
  %10 = add nsw i16 %6, %9
  %11 = trunc i16 %10 to i8
  %12 = getelementptr inbounds nuw %struct.S, ptr %0, i32 0, i32 0
  store i8 %11, ptr %12, align 1, !tbaa !7
  %13 = getelementptr inbounds nuw %struct.S, ptr %1, i32 0, i32 1
  %14 = load i64, ptr %13, align 1, !tbaa !10
  %15 = mul nsw i64 %14, 2
  %16 = getelementptr inbounds nuw %struct.S, ptr %2, i32 0, i32 1
  %17 = load i64, ptr %16, align 1, !tbaa !10
  %18 = add nsw i64 %15, %17
  %19 = getelementptr inbounds nuw %struct.S, ptr %0, i32 0, i32 1
  store i64 %18, ptr %19, align 1, !tbaa !10
  ret void
}

; Function Attrs: nounwind optsize
define dso_local i64 @sum(ptr noundef byval(%struct.S) align 1 %0, ptr noundef byval(%struct.S) align 1 %1) #0 {
  %3 = getelementptr inbounds nuw %struct.S, ptr %0, i32 0, i32 1
  %4 = load i64, ptr %3, align 1, !tbaa !10
  %5 = getelementptr inbounds nuw %struct.S, ptr %1, i32 0, i32 1
  %6 = load i64, ptr %5, align 1, !tbaa !10
  %7 = add nsw i64 %4, %6
  %8 = getelementptr inbounds nuw %struct.S, ptr %0, i32 0, i32 0
  %9 = load i8, ptr %8, align 1, !tbaa !7
  %10 = zext i8 %9 to i16
  %11 = mul nsw i16 %10, 1000
  %12 = sext i16 %11 to i64
  %13 = add nsw i64 %7, %12
  %14 = getelementptr inbounds nuw %struct.S, ptr %1, i32 0, i32 0
  %15 = load i8, ptr %14, align 1, !tbaa !7
  %16 = zext i8 %15 to i32
  %17 = mul nsw i32 %16, 100000
  %18 = sext i32 %17 to i64
  %19 = add nsw i64 %13, %18
  ret i64 %19
}

; Function Attrs: nounwind optsize
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  %2 = alloca %struct.S, align 1
  %3 = alloca %struct.S, align 1
  %4 = alloca %struct.S, align 1
  %5 = alloca %struct.S, align 1
  %6 = alloca %struct.S, align 1
  store i16 0, ptr %1, align 2
  call void @llvm.lifetime.start.p0(ptr %2) #3
  call void @llvm.lifetime.start.p0(ptr %3) #3
  call void @llvm.lifetime.start.p0(ptr %4) #3
  %7 = getelementptr inbounds nuw %struct.S, ptr %2, i32 0, i32 0
  store i8 1, ptr %7, align 1, !tbaa !7
  %8 = getelementptr inbounds nuw %struct.S, ptr %2, i32 0, i32 1
  store i64 32, ptr %8, align 1, !tbaa !10
  %9 = getelementptr inbounds nuw %struct.S, ptr %3, i32 0, i32 0
  store i8 2, ptr %9, align 1, !tbaa !7
  %10 = getelementptr inbounds nuw %struct.S, ptr %3, i32 0, i32 1
  store i64 4, ptr %10, align 1, !tbaa !10
  call void @llvm.lifetime.start.p0(ptr %5) #3
  call void @divided(ptr dead_on_unwind writable sret(%struct.S) align 1 %5, ptr noundef byval(%struct.S) align 1 %2, ptr noundef byval(%struct.S) align 1 %3) #4
  call void @llvm.memcpy.p0.p0.i16(ptr align 1 %4, ptr align 1 %5, i16 9, i1 false), !tbaa.struct !11
  call void @llvm.lifetime.end.p0(ptr %5) #3
  %11 = getelementptr inbounds nuw %struct.S, ptr %4, i32 0, i32 1
  %12 = load i64, ptr %11, align 1, !tbaa !10
  %13 = trunc i64 %12 to i32
  call void @report(i32 noundef %13) #4
  %14 = getelementptr inbounds nuw %struct.S, ptr %4, i32 0, i32 0
  %15 = load i8, ptr %14, align 1, !tbaa !7
  %16 = zext i8 %15 to i32
  call void @report(i32 noundef %16) #4
  %17 = call i64 @sum(ptr noundef byval(%struct.S) align 1 %2, ptr noundef byval(%struct.S) align 1 %3) #4
  %18 = trunc i64 %17 to i32
  call void @report(i32 noundef %18) #4
  %19 = getelementptr inbounds nuw %struct.S, ptr %2, i32 0, i32 1
  store i64 -8, ptr %19, align 1, !tbaa !10
  %20 = getelementptr inbounds nuw %struct.S, ptr %3, i32 0, i32 1
  store i64 -2, ptr %20, align 1, !tbaa !10
  call void @llvm.lifetime.start.p0(ptr %6) #3
  call void @divided(ptr dead_on_unwind writable sret(%struct.S) align 1 %6, ptr noundef byval(%struct.S) align 1 %2, ptr noundef byval(%struct.S) align 1 %3) #4
  call void @llvm.memcpy.p0.p0.i16(ptr align 1 %4, ptr align 1 %6, i16 9, i1 false), !tbaa.struct !11
  call void @llvm.lifetime.end.p0(ptr %6) #3
  %21 = getelementptr inbounds nuw %struct.S, ptr %4, i32 0, i32 1
  %22 = load i64, ptr %21, align 1, !tbaa !10
  %23 = trunc i64 %22 to i32
  call void @report(i32 noundef %23) #4
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  ret i16 0
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.memcpy.p0.p0.i16(ptr noalias writeonly captures(none), ptr noalias readonly captures(none), i16, i1 immarg) #1

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

; Function Attrs: optsize
declare dso_local void @report(i32 noundef) #2

attributes #0 = { nounwind optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #3 = { nounwind }
attributes #4 = { optsize }

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
!7 = !{!8, !5, i64 0}
!8 = !{!"S", !5, i64 0, !9, i64 1}
!9 = !{!"long long", !5, i64 0}
!10 = !{!8, !9, i64 1}
!11 = !{i64 0, i64 1, !12, i64 1, i64 8, !13}
!12 = !{!5, !5, i64 0}
!13 = !{!9, !9, i64 0}
