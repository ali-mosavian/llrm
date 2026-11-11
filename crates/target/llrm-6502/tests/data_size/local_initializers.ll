target triple = "msp430"

%struct.pair = type { i16, i16 }

@__const.main.one = private unnamed_addr constant [1 x %struct.pair] [%struct.pair { i16 0, i16 1 }], align 1
@__const.main.many = private unnamed_addr constant [3 x %struct.pair] [%struct.pair { i16 3, i16 4 }, %struct.pair { i16 5, i16 6 }, %struct.pair { i16 7, i16 8 }], align 1
@__const.main.numbers = private unnamed_addr constant [5 x i16] [i16 11, i16 22, i16 33, i16 44, i16 55], align 2
@__const.main.partial = private unnamed_addr constant [4 x i16] [i16 9, i16 8, i16 0, i16 0], align 2
@__const.main.wide = private unnamed_addr constant [2 x i32] [i32 100000, i32 -200000], align 2

; Function Attrs: nounwind optsize
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  %2 = alloca [1 x %struct.pair], align 1
  %3 = alloca [3 x %struct.pair], align 1
  %4 = alloca [5 x i16], align 2
  %5 = alloca [4 x i16], align 2
  %6 = alloca [2 x i32], align 2
  store i16 0, ptr %1, align 2
  call void @llvm.lifetime.start.p0(ptr %2) #3
  call void @llvm.memcpy.p0.p0.i16(ptr align 1 %2, ptr align 1 @__const.main.one, i16 4, i1 false)
  call void @llvm.lifetime.start.p0(ptr %3) #3
  call void @llvm.memcpy.p0.p0.i16(ptr align 1 %3, ptr align 1 @__const.main.many, i16 12, i1 false)
  call void @llvm.lifetime.start.p0(ptr %4) #3
  call void @llvm.memcpy.p0.p0.i16(ptr align 2 %4, ptr align 2 @__const.main.numbers, i16 10, i1 false)
  call void @llvm.lifetime.start.p0(ptr %5) #3
  call void @llvm.memcpy.p0.p0.i16(ptr align 2 %5, ptr align 2 @__const.main.partial, i16 8, i1 false)
  call void @llvm.lifetime.start.p0(ptr %6) #3
  call void @llvm.memcpy.p0.p0.i16(ptr align 2 %6, ptr align 2 @__const.main.wide, i16 8, i1 false)
  %7 = getelementptr inbounds [1 x %struct.pair], ptr %2, i16 0, i16 0
  %8 = getelementptr inbounds nuw %struct.pair, ptr %7, i32 0, i32 0
  %9 = load i16, ptr %8, align 1, !tbaa !7
  %10 = mul nsw i16 %9, 10
  %11 = getelementptr inbounds [1 x %struct.pair], ptr %2, i16 0, i16 0
  %12 = getelementptr inbounds nuw %struct.pair, ptr %11, i32 0, i32 1
  %13 = load i16, ptr %12, align 1, !tbaa !9
  %14 = add nsw i16 %10, %13
  %15 = sext i16 %14 to i32
  call void @report(i32 noundef %15) #4
  %16 = getelementptr inbounds [3 x %struct.pair], ptr %3, i16 0, i16 0
  %17 = getelementptr inbounds nuw %struct.pair, ptr %16, i32 0, i32 0
  %18 = load i16, ptr %17, align 1, !tbaa !7
  %19 = sext i16 %18 to i32
  %20 = getelementptr inbounds [3 x %struct.pair], ptr %3, i16 0, i16 1
  %21 = getelementptr inbounds nuw %struct.pair, ptr %20, i32 0, i32 1
  %22 = load i16, ptr %21, align 1, !tbaa !9
  %23 = sext i16 %22 to i32
  %24 = mul nsw i32 %23, 100
  %25 = add nsw i32 %19, %24
  %26 = getelementptr inbounds [3 x %struct.pair], ptr %3, i16 0, i16 2
  %27 = getelementptr inbounds nuw %struct.pair, ptr %26, i32 0, i32 0
  %28 = load i16, ptr %27, align 1, !tbaa !7
  %29 = sext i16 %28 to i32
  %30 = mul nsw i32 %29, 10000
  %31 = add nsw i32 %25, %30
  call void @report(i32 noundef %31) #4
  %32 = getelementptr inbounds [5 x i16], ptr %4, i16 0, i16 0
  %33 = call i32 @sum(ptr noundef %32, i16 noundef 5) #4
  call void @report(i32 noundef %33) #4
  %34 = getelementptr inbounds [4 x i16], ptr %5, i16 0, i16 0
  %35 = call i32 @sum(ptr noundef %34, i16 noundef 4) #4
  call void @report(i32 noundef %35) #4
  %36 = getelementptr inbounds [2 x i32], ptr %6, i16 0, i16 0
  %37 = load i32, ptr %36, align 2, !tbaa !10
  %38 = getelementptr inbounds [2 x i32], ptr %6, i16 0, i16 1
  %39 = load i32, ptr %38, align 2, !tbaa !10
  %40 = add nsw i32 %37, %39
  call void @report(i32 noundef %40) #4
  call void @llvm.lifetime.end.p0(ptr %6) #3
  call void @llvm.lifetime.end.p0(ptr %5) #3
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  ret i16 0
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.memcpy.p0.p0.i16(ptr noalias writeonly captures(none), ptr noalias readonly captures(none), i16, i1 immarg) #1

; Function Attrs: optsize
declare dso_local void @report(i32 noundef) #2

; Function Attrs: nounwind optsize
define internal i32 @sum(ptr noundef %0, i16 noundef %1) #0 {
  %3 = alloca ptr, align 2
  %4 = alloca i16, align 2
  %5 = alloca i32, align 2
  %6 = alloca i16, align 2
  store ptr %0, ptr %3, align 2, !tbaa !12
  store i16 %1, ptr %4, align 2, !tbaa !15
  call void @llvm.lifetime.start.p0(ptr %5) #3
  store i32 0, ptr %5, align 2, !tbaa !10
  call void @llvm.lifetime.start.p0(ptr %6) #3
  store i16 0, ptr %6, align 2, !tbaa !15
  br label %7

7:                                                ; preds = %20, %2
  %8 = load i16, ptr %6, align 2, !tbaa !15
  %9 = load i16, ptr %4, align 2, !tbaa !15
  %10 = icmp slt i16 %8, %9
  br i1 %10, label %11, label %23

11:                                               ; preds = %7
  %12 = load i32, ptr %5, align 2, !tbaa !10
  %13 = mul nsw i32 %12, 7
  %14 = load ptr, ptr %3, align 2, !tbaa !12
  %15 = load i16, ptr %6, align 2, !tbaa !15
  %16 = getelementptr inbounds i16, ptr %14, i16 %15
  %17 = load i16, ptr %16, align 2, !tbaa !15
  %18 = sext i16 %17 to i32
  %19 = add nsw i32 %13, %18
  store i32 %19, ptr %5, align 2, !tbaa !10
  br label %20

20:                                               ; preds = %11
  %21 = load i16, ptr %6, align 2, !tbaa !15
  %22 = add nsw i16 %21, 1
  store i16 %22, ptr %6, align 2, !tbaa !15
  br label %7, !llvm.loop !16

23:                                               ; preds = %7
  %24 = load i32, ptr %5, align 2, !tbaa !10
  call void @llvm.lifetime.end.p0(ptr %6) #3
  call void @llvm.lifetime.end.p0(ptr %5) #3
  ret i32 %24
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

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
!7 = !{!8, !4, i64 0}
!8 = !{!"pair", !4, i64 0, !4, i64 2}
!9 = !{!8, !4, i64 2}
!10 = !{!11, !11, i64 0}
!11 = !{!"long", !5, i64 0}
!12 = !{!13, !13, i64 0}
!13 = !{!"p1 int", !14, i64 0}
!14 = !{!"any pointer", !5, i64 0}
!15 = !{!4, !4, i64 0}
!16 = distinct !{!16, !17}
!17 = !{!"llvm.loop.mustprogress"}
