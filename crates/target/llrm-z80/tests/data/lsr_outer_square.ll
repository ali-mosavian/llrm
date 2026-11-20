target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"
; ModuleID = 'sieve.c'
source_filename = "sieve.c"
target triple = "msp430"

; Function Attrs: nounwind optsize
define dso_local i32 @bench_sieve(i16 noundef zeroext %0) #0 {
  %2 = alloca i16, align 2
  %3 = alloca [1024 x i8], align 1
  %4 = alloca i16, align 2
  %5 = alloca i16, align 2
  %6 = alloca i16, align 2
  %7 = alloca i32, align 2
  store i16 %0, ptr %2, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %3) #3
  call void @llvm.lifetime.start.p0(ptr %4) #3
  call void @llvm.lifetime.start.p0(ptr %5) #3
  call void @llvm.lifetime.start.p0(ptr %6) #3
  store i16 0, ptr %6, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %7) #3
  store i32 0, ptr %7, align 2, !tbaa !9
  store i16 0, ptr %4, align 2, !tbaa !7
  br label %8

8:                                                ; preds = %15, %1
  %9 = load i16, ptr %4, align 2, !tbaa !7
  %10 = load i16, ptr %2, align 2, !tbaa !7
  %11 = icmp ult i16 %9, %10
  br i1 %11, label %12, label %18

12:                                               ; preds = %8
  %13 = load i16, ptr %4, align 2, !tbaa !7
  %14 = getelementptr inbounds nuw [1024 x i8], ptr %3, i16 0, i16 %13
  store i8 0, ptr %14, align 1, !tbaa !11
  br label %15

15:                                               ; preds = %12
  %16 = load i16, ptr %4, align 2, !tbaa !7
  %17 = add i16 %16, 1
  store i16 %17, ptr %4, align 2, !tbaa !7
  br label %8, !llvm.loop !12

18:                                               ; preds = %8
  store i16 2, ptr %4, align 2, !tbaa !7
  br label %19

19:                                               ; preds = %55, %18
  %20 = load i16, ptr %4, align 2, !tbaa !7
  %21 = load i16, ptr %2, align 2, !tbaa !7
  %22 = icmp ult i16 %20, %21
  br i1 %22, label %23, label %58

23:                                               ; preds = %19
  %24 = load i16, ptr %4, align 2, !tbaa !7
  %25 = getelementptr inbounds nuw [1024 x i8], ptr %3, i16 0, i16 %24
  %26 = load i8, ptr %25, align 1, !tbaa !11
  %27 = icmp ne i8 %26, 0
  br i1 %27, label %28, label %29

28:                                               ; preds = %23
  br label %55

29:                                               ; preds = %23
  %30 = load i16, ptr %6, align 2, !tbaa !7
  %31 = add i16 %30, 1
  store i16 %31, ptr %6, align 2, !tbaa !7
  %32 = load i16, ptr %4, align 2, !tbaa !7
  %33 = zext i16 %32 to i32
  %34 = load i32, ptr %7, align 2, !tbaa !9
  %35 = add i32 %34, %33
  store i32 %35, ptr %7, align 2, !tbaa !9
  %36 = load i16, ptr %4, align 2, !tbaa !7
  %37 = icmp ule i16 %36, 31
  br i1 %37, label %38, label %54

38:                                               ; preds = %29
  %39 = load i16, ptr %4, align 2, !tbaa !7
  %40 = load i16, ptr %4, align 2, !tbaa !7
  %41 = mul i16 %39, %40
  store i16 %41, ptr %5, align 2, !tbaa !7
  br label %42

42:                                               ; preds = %49, %38
  %43 = load i16, ptr %5, align 2, !tbaa !7
  %44 = load i16, ptr %2, align 2, !tbaa !7
  %45 = icmp ult i16 %43, %44
  br i1 %45, label %46, label %53

46:                                               ; preds = %42
  %47 = load i16, ptr %5, align 2, !tbaa !7
  %48 = getelementptr inbounds nuw [1024 x i8], ptr %3, i16 0, i16 %47
  store i8 1, ptr %48, align 1, !tbaa !11
  br label %49

49:                                               ; preds = %46
  %50 = load i16, ptr %5, align 2, !tbaa !7
  %51 = load i16, ptr %4, align 2, !tbaa !7
  %52 = add i16 %50, %51
  store i16 %52, ptr %5, align 2, !tbaa !7
  br label %42, !llvm.loop !14

53:                                               ; preds = %42
  br label %54

54:                                               ; preds = %53, %29
  br label %55

55:                                               ; preds = %54, %28
  %56 = load i16, ptr %4, align 2, !tbaa !7
  %57 = add i16 %56, 1
  store i16 %57, ptr %4, align 2, !tbaa !7
  br label %19, !llvm.loop !15

58:                                               ; preds = %19
  %59 = load i16, ptr %6, align 2, !tbaa !7
  %60 = zext i16 %59 to i32
  %61 = shl i32 %60, 16
  %62 = load i32, ptr %7, align 2, !tbaa !9
  %63 = xor i32 %61, %62
  call void @llvm.lifetime.end.p0(ptr %7) #3
  call void @llvm.lifetime.end.p0(ptr %6) #3
  call void @llvm.lifetime.end.p0(ptr %5) #3
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  ret i32 %63
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

; Function Attrs: nounwind optsize
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  store i16 0, ptr %1, align 2
  %2 = call i32 @bench_sieve(i16 noundef zeroext 1024) #4
  call void @report(i32 noundef %2) #4
  ret i16 0
}

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
!7 = !{!8, !8, i64 0}
!8 = !{!"short", !5, i64 0}
!9 = !{!10, !10, i64 0}
!10 = !{!"long", !5, i64 0}
!11 = !{!5, !5, i64 0}
!12 = distinct !{!12, !13}
!13 = !{!"llvm.loop.mustprogress"}
!14 = distinct !{!14, !13}
!15 = distinct !{!15, !13}
