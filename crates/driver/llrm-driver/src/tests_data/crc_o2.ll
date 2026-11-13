; ModuleID = '/home/alim/scratch/wt-m64-tip/bench/crc/crc.c'
source_filename = "/home/alim/scratch/wt-m64-tip/bench/crc/crc.c"
target datalayout = "@m64-datalayout@"

@bench_crc.data = internal constant [10 x i8] c"123456789\00", align 1

; Function Attrs: nounwind uwtable
define dso_local i64 @bench_crc(i64 noundef %0) #0 {
  %2 = alloca i64, align 8
  %3 = alloca i64, align 8
  %4 = alloca i16, align 2
  %5 = alloca i16, align 2
  store i64 %0, ptr %2, align 8, !tbaa !9
  call void @llvm.lifetime.start.p0(ptr %3) #3
  %6 = load i64, ptr %2, align 8, !tbaa !9
  %7 = xor i64 4294967295, %6
  store i64 %7, ptr %3, align 8, !tbaa !9
  call void @llvm.lifetime.start.p0(ptr %4) #3
  call void @llvm.lifetime.start.p0(ptr %5) #3
  store i16 0, ptr %4, align 2, !tbaa !11
  br label %8

8:                                                ; preds = %36, %1
  %9 = load i16, ptr %4, align 2, !tbaa !11
  %10 = zext i16 %9 to i32
  %11 = icmp slt i32 %10, 9
  br i1 %11, label %12, label %39

12:                                               ; preds = %8
  %13 = load i16, ptr %4, align 2, !tbaa !11
  %14 = zext i16 %13 to i64
  %15 = getelementptr inbounds nuw [10 x i8], ptr @bench_crc.data, i64 0, i64 %14
  %16 = load i8, ptr %15, align 1, !tbaa !13
  %17 = zext i8 %16 to i64
  %18 = load i64, ptr %3, align 8, !tbaa !9
  %19 = xor i64 %18, %17
  store i64 %19, ptr %3, align 8, !tbaa !9
  store i16 0, ptr %5, align 2, !tbaa !11
  br label %20

20:                                               ; preds = %32, %12
  %21 = load i16, ptr %5, align 2, !tbaa !11
  %22 = zext i16 %21 to i32
  %23 = icmp slt i32 %22, 8
  br i1 %23, label %24, label %35

24:                                               ; preds = %20
  %25 = load i64, ptr %3, align 8, !tbaa !9
  %26 = lshr i64 %25, 1
  %27 = load i64, ptr %3, align 8, !tbaa !9
  %28 = and i64 %27, 1
  %29 = sub i64 0, %28
  %30 = and i64 3988292384, %29
  %31 = xor i64 %26, %30
  store i64 %31, ptr %3, align 8, !tbaa !9
  br label %32

32:                                               ; preds = %24
  %33 = load i16, ptr %5, align 2, !tbaa !11
  %34 = add i16 %33, 1
  store i16 %34, ptr %5, align 2, !tbaa !11
  br label %20, !llvm.loop !14

35:                                               ; preds = %20
  br label %36

36:                                               ; preds = %35
  %37 = load i16, ptr %4, align 2, !tbaa !11
  %38 = add i16 %37, 1
  store i16 %38, ptr %4, align 2, !tbaa !11
  br label %8, !llvm.loop !16

39:                                               ; preds = %8
  %40 = load i64, ptr %3, align 8, !tbaa !9
  %41 = xor i64 %40, 4294967295
  call void @llvm.lifetime.end.p0(ptr %5) #3
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  ret i64 %41
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

; Function Attrs: nounwind uwtable
define dso_local i32 @bench_main_() #0 {
  %1 = call i64 @bench_crc(i64 noundef 0)
  call void @report(i64 noundef %1)
  ret i32 0
}

declare void @report(i64 noundef) #2

attributes #0 = { nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" }
attributes #3 = { nounwind }

!llvm.module.flags = !{!0, !1, !2}
!llvm.ident = !{!3}
!llvm.errno.tbaa = !{!4}

!0 = !{i32 8, !"PIC Level", i32 2}
!1 = !{i32 7, !"PIE Level", i32 2}
!2 = !{i32 7, !"uwtable", i32 2}
!3 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!4 = !{!5, !6, i64 0}
!5 = !{!"__libc_errno", !6, i64 0}
!6 = !{!"int", !7, i64 0}
!7 = !{!"omnipotent char", !8, i64 0}
!8 = !{!"Simple C/C++ TBAA"}
!9 = !{!10, !10, i64 0}
!10 = !{!"long", !7, i64 0}
!11 = !{!12, !12, i64 0}
!12 = !{!"short", !7, i64 0}
!13 = !{!7, !7, i64 0}
!14 = distinct !{!14, !15}
!15 = !{!"llvm.loop.mustprogress"}
!16 = distinct !{!16, !15}
