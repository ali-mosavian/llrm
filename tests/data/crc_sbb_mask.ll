; ModuleID = '/home/alim/scratch/dyn-work/src/crc.c'
source_filename = "/home/alim/scratch/dyn-work/src/crc.c"
target datalayout = "e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32"

@bench_crc.data = internal constant [10 x i8] c"123456789\00", align 1
@bench_crc_via = dso_local global ptr @bench_crc, align 4

; Function Attrs: nounwind
define dso_local i32 @bench_crc(i32 noundef %0) #0 {
  %2 = alloca i32, align 4
  %3 = alloca i32, align 4
  %4 = alloca i16, align 2
  %5 = alloca i16, align 2
  store i32 %0, ptr %2, align 4, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %3) #3
  %6 = load i32, ptr %2, align 4, !tbaa !7
  %7 = xor i32 -1, %6
  store i32 %7, ptr %3, align 4, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %4) #3
  call void @llvm.lifetime.start.p0(ptr %5) #3
  store i16 0, ptr %4, align 2, !tbaa !9
  br label %8

8:                                                ; preds = %36, %1
  %9 = load i16, ptr %4, align 2, !tbaa !9
  %10 = zext i16 %9 to i32
  %11 = icmp slt i32 %10, 9
  br i1 %11, label %12, label %39

12:                                               ; preds = %8
  %13 = load i16, ptr %4, align 2, !tbaa !9
  %14 = zext i16 %13 to i32
  %15 = getelementptr inbounds nuw [10 x i8], ptr @bench_crc.data, i32 0, i32 %14
  %16 = load i8, ptr %15, align 1, !tbaa !11
  %17 = zext i8 %16 to i32
  %18 = load i32, ptr %3, align 4, !tbaa !7
  %19 = xor i32 %18, %17
  store i32 %19, ptr %3, align 4, !tbaa !7
  store i16 0, ptr %5, align 2, !tbaa !9
  br label %20

20:                                               ; preds = %32, %12
  %21 = load i16, ptr %5, align 2, !tbaa !9
  %22 = zext i16 %21 to i32
  %23 = icmp slt i32 %22, 8
  br i1 %23, label %24, label %35

24:                                               ; preds = %20
  %25 = load i32, ptr %3, align 4, !tbaa !7
  %26 = lshr i32 %25, 1
  %27 = load i32, ptr %3, align 4, !tbaa !7
  %28 = and i32 %27, 1
  %29 = sub i32 0, %28
  %30 = and i32 -306674912, %29
  %31 = xor i32 %26, %30
  store i32 %31, ptr %3, align 4, !tbaa !7
  br label %32

32:                                               ; preds = %24
  %33 = load i16, ptr %5, align 2, !tbaa !9
  %34 = add i16 %33, 1
  store i16 %34, ptr %5, align 2, !tbaa !9
  br label %20

35:                                               ; preds = %20
  br label %36

36:                                               ; preds = %35
  %37 = load i16, ptr %4, align 2, !tbaa !9
  %38 = add i16 %37, 1
  store i16 %38, ptr %4, align 2, !tbaa !9
  br label %8

39:                                               ; preds = %8
  %40 = load i32, ptr %3, align 4, !tbaa !7
  %41 = xor i32 %40, -1
  call void @llvm.lifetime.end.p0(ptr %5) #3
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  ret i32 %41
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

; Function Attrs: nounwind
define dso_local i32 @main() #0 {
  %1 = load volatile ptr, ptr @bench_crc_via, align 4, !tbaa !12
  %2 = call i32 %1(i32 noundef 0) #4
  call void @report(i32 noundef %2) #4
  ret i32 0
}

declare dso_local void @report(i32 noundef) #2

attributes #0 = { nounwind "min-legal-vector-width"="0" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="i486" "target-features"="+x87" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="i486" "target-features"="+x87" }
attributes #3 = { nounwind }
attributes #4 = { nobuiltin "no-builtins" }

!llvm.module.flags = !{!0}
!llvm.ident = !{!1}
!llvm.errno.tbaa = !{!2}

!0 = !{i32 1, !"NumRegisterParameters", i32 0}
!1 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!2 = !{!3, !4, i64 0}
!3 = !{!"__libc_errno", !4, i64 0}
!4 = !{!"int", !5, i64 0}
!5 = !{!"omnipotent char", !6, i64 0}
!6 = !{!"Simple C/C++ TBAA"}
!7 = !{!8, !8, i64 0}
!8 = !{!"long", !5, i64 0}
!9 = !{!10, !10, i64 0}
!10 = !{!"short", !5, i64 0}
!11 = !{!5, !5, i64 0}
!12 = !{!13, !13, i64 0}
!13 = !{!"any pointer", !5, i64 0}
