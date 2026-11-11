target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8"
target triple = "msp430"

%struct.s = type <{ i8, i32 }>
%struct.t = type { i64 }

@main.i = internal global %struct.s zeroinitializer, align 1

; Function Attrs: nounwind optsize
define dso_local void @f(ptr dead_on_unwind noalias writable sret(%struct.s) align 1 %0, ptr noundef byval(%struct.s) align 1 %1) #0 {
  %3 = getelementptr inbounds nuw %struct.s, ptr %1, i32 0, i32 1
  store i32 -839970252, ptr %3, align 1
  call void @llvm.memcpy.p0.p0.i16(ptr align 1 %0, ptr align 1 %1, i16 5, i1 false), !tbaa.struct !7
  ret void
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.memcpy.p0.p0.i16(ptr noalias writeonly captures(none), ptr noalias readonly captures(none), i16, i1 immarg) #1

; Function Attrs: nounwind optsize
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  %2 = alloca %struct.s, align 1
  %3 = alloca %struct.t, align 1
  %4 = alloca %struct.s, align 1
  store i16 0, ptr %1, align 2
  call void @llvm.lifetime.start.p0(ptr %2) #3
  call void @llvm.lifetime.start.p0(ptr %3) #3
  store i8 12, ptr @main.i, align 1
  call void @llvm.lifetime.start.p0(ptr %4) #3
  call void @f(ptr dead_on_unwind writable sret(%struct.s) align 1 %4, ptr noundef byval(%struct.s) align 1 @main.i) #4
  call void @llvm.memcpy.p0.p0.i16(ptr align 1 @main.i, ptr align 1 %4, i16 5, i1 false), !tbaa.struct !7
  call void @llvm.lifetime.end.p0(ptr %4) #3
  %5 = load i8, ptr @main.i, align 1
  %6 = zext i8 %5 to i64
  %7 = trunc i64 %6 to i32
  call void @report(i32 noundef %7) #4
  %8 = load i32, ptr getelementptr inbounds nuw (i8, ptr @main.i, i16 1), align 1
  %9 = zext i32 %8 to i64
  %10 = icmp eq i64 %9, 3454997044
  %11 = zext i1 %10 to i16
  %12 = sext i16 %11 to i32
  call void @report(i32 noundef %12) #4
  store i8 -56, ptr %2, align 1
  %13 = getelementptr inbounds nuw %struct.s, ptr %2, i32 0, i32 1
  store i32 7, ptr %13, align 1
  %14 = getelementptr inbounds nuw %struct.s, ptr %2, i32 0, i32 1
  %15 = load i32, ptr %14, align 1
  %16 = zext i32 %15 to i64
  %17 = add i64 %16, 5
  %18 = trunc i64 %17 to i32
  store i32 %18, ptr %14, align 1
  %19 = zext i32 %18 to i64
  %20 = load i8, ptr %2, align 1
  %21 = zext i8 %20 to i64
  %22 = trunc i64 %21 to i32
  %23 = mul nsw i32 %22, 1000
  %24 = getelementptr inbounds nuw %struct.s, ptr %2, i32 0, i32 1
  %25 = load i32, ptr %24, align 1
  %26 = zext i32 %25 to i64
  %27 = trunc i64 %26 to i32
  %28 = add nsw i32 %23, %27
  call void @report(i32 noundef %28) #4
  %29 = load i64, ptr %3, align 1
  %30 = and i64 %29, -8
  %31 = or i64 %30, 5
  store i64 %31, ptr %3, align 1
  %32 = load i64, ptr %3, align 1
  %33 = and i64 %32, -9007199254740985
  %34 = or i64 %33, 39093746760
  store i64 %34, ptr %3, align 1
  %35 = load i64, ptr %3, align 1
  %36 = and i64 %35, 9007199254740991
  %37 = or i64 %36, 9007199254740992000
  store i64 %37, ptr %3, align 1
  %38 = load i64, ptr %3, align 1
  %39 = and i64 %38, 7
  %40 = trunc i64 %39 to i32
  call void @report(i32 noundef %40) #4
  %41 = load i64, ptr %3, align 1
  %42 = lshr i64 %41, 3
  %43 = and i64 %42, 1125899906842623
  %44 = and i64 %43, 16777215
  %45 = trunc i64 %44 to i32
  call void @report(i32 noundef %45) #4
  %46 = load i64, ptr %3, align 1
  %47 = lshr i64 %46, 53
  %48 = trunc i64 %47 to i32
  call void @report(i32 noundef %48) #4
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  ret i16 0
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

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
!7 = !{i64 0, i64 1, !8, i64 1, i64 4, !8}
!8 = !{!5, !5, i64 0}
