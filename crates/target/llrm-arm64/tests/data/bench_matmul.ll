; ModuleID = 'w15-bar/out/src/matmul.c'
source_filename = "w15-bar/out/src/matmul.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

@opq0 = internal global i32 0, align 4

; Function Attrs: noinline nounwind
define dso_local i64 @bench_matmul(i16 noundef %0) #0 {
  %2 = alloca i16, align 2
  %3 = alloca [8 x [8 x i16]], align 2
  %4 = alloca [8 x [8 x i16]], align 2
  %5 = alloca [8 x [8 x i64]], align 8
  %6 = alloca i16, align 2
  %7 = alloca i16, align 2
  %8 = alloca i16, align 2
  %9 = alloca i64, align 8
  %10 = alloca i64, align 8
  store i16 %0, ptr %2, align 2, !tbaa !5
  call void @llvm.lifetime.start.p0(i64 128, ptr %3) #4
  call void @llvm.lifetime.start.p0(i64 128, ptr %4) #4
  call void @llvm.lifetime.start.p0(i64 512, ptr %5) #4
  call void @llvm.lifetime.start.p0(i64 2, ptr %6) #4
  call void @llvm.lifetime.start.p0(i64 2, ptr %7) #4
  call void @llvm.lifetime.start.p0(i64 2, ptr %8) #4
  call void @llvm.lifetime.start.p0(i64 8, ptr %9) #4
  store i64 0, ptr %9, align 8, !tbaa !9
  store i16 0, ptr %6, align 2, !tbaa !5
  br label %11

11:                                               ; preds = %64, %1
  %12 = load i16, ptr %6, align 2, !tbaa !5
  %13 = zext i16 %12 to i32
  %14 = icmp slt i32 %13, 8
  br i1 %14, label %15, label %67

15:                                               ; preds = %11
  store i16 0, ptr %7, align 2, !tbaa !5
  br label %16

16:                                               ; preds = %60, %15
  %17 = load i16, ptr %7, align 2, !tbaa !5
  %18 = zext i16 %17 to i32
  %19 = icmp slt i32 %18, 8
  br i1 %19, label %20, label %63

20:                                               ; preds = %16
  %21 = load i16, ptr %6, align 2, !tbaa !5
  %22 = zext i16 %21 to i32
  %23 = mul nsw i32 %22, 3
  %24 = load i16, ptr %7, align 2, !tbaa !5
  %25 = zext i16 %24 to i32
  %26 = add nsw i32 %23, %25
  %27 = add nsw i32 %26, 1
  %28 = load i16, ptr %2, align 2, !tbaa !5
  %29 = zext i16 %28 to i32
  %30 = add nsw i32 %27, %29
  %31 = trunc i32 %30 to i16
  %32 = load i16, ptr %6, align 2, !tbaa !5
  %33 = zext i16 %32 to i64
  %34 = getelementptr inbounds nuw [8 x [8 x i16]], ptr %3, i64 0, i64 %33
  %35 = load i16, ptr %7, align 2, !tbaa !5
  %36 = zext i16 %35 to i64
  %37 = getelementptr inbounds nuw [8 x i16], ptr %34, i64 0, i64 %36
  store i16 %31, ptr %37, align 2, !tbaa !5
  %38 = load i16, ptr %6, align 2, !tbaa !5
  %39 = zext i16 %38 to i32
  %40 = load i16, ptr %7, align 2, !tbaa !5
  %41 = zext i16 %40 to i32
  %42 = icmp eq i32 %39, %41
  br i1 %42, label %43, label %44

43:                                               ; preds = %20
  br label %51

44:                                               ; preds = %20
  %45 = load i16, ptr %6, align 2, !tbaa !5
  %46 = zext i16 %45 to i32
  %47 = load i16, ptr %7, align 2, !tbaa !5
  %48 = zext i16 %47 to i32
  %49 = add nsw i32 %46, %48
  %50 = srem i32 %49, 3
  br label %51

51:                                               ; preds = %44, %43
  %52 = phi i32 [ 2, %43 ], [ %50, %44 ]
  %53 = trunc i32 %52 to i16
  %54 = load i16, ptr %6, align 2, !tbaa !5
  %55 = zext i16 %54 to i64
  %56 = getelementptr inbounds nuw [8 x [8 x i16]], ptr %4, i64 0, i64 %55
  %57 = load i16, ptr %7, align 2, !tbaa !5
  %58 = zext i16 %57 to i64
  %59 = getelementptr inbounds nuw [8 x i16], ptr %56, i64 0, i64 %58
  store i16 %53, ptr %59, align 2, !tbaa !5
  br label %60

60:                                               ; preds = %51
  %61 = load i16, ptr %7, align 2, !tbaa !5
  %62 = add i16 %61, 1
  store i16 %62, ptr %7, align 2, !tbaa !5
  br label %16, !llvm.loop !11

63:                                               ; preds = %16
  br label %64

64:                                               ; preds = %63
  %65 = load i16, ptr %6, align 2, !tbaa !5
  %66 = add i16 %65, 1
  store i16 %66, ptr %6, align 2, !tbaa !5
  br label %11, !llvm.loop !13

67:                                               ; preds = %11
  store i16 0, ptr %6, align 2, !tbaa !5
  br label %68

68:                                               ; preds = %117, %67
  %69 = load i16, ptr %6, align 2, !tbaa !5
  %70 = zext i16 %69 to i32
  %71 = icmp slt i32 %70, 8
  br i1 %71, label %72, label %120

72:                                               ; preds = %68
  store i16 0, ptr %7, align 2, !tbaa !5
  br label %73

73:                                               ; preds = %113, %72
  %74 = load i16, ptr %7, align 2, !tbaa !5
  %75 = zext i16 %74 to i32
  %76 = icmp slt i32 %75, 8
  br i1 %76, label %77, label %116

77:                                               ; preds = %73
  call void @llvm.lifetime.start.p0(i64 8, ptr %10) #4
  store i64 0, ptr %10, align 8, !tbaa !9
  store i16 0, ptr %8, align 2, !tbaa !5
  br label %78

78:                                               ; preds = %102, %77
  %79 = load i16, ptr %8, align 2, !tbaa !5
  %80 = zext i16 %79 to i32
  %81 = icmp slt i32 %80, 8
  br i1 %81, label %82, label %105

82:                                               ; preds = %78
  %83 = load i16, ptr %6, align 2, !tbaa !5
  %84 = zext i16 %83 to i64
  %85 = getelementptr inbounds nuw [8 x [8 x i16]], ptr %3, i64 0, i64 %84
  %86 = load i16, ptr %8, align 2, !tbaa !5
  %87 = zext i16 %86 to i64
  %88 = getelementptr inbounds nuw [8 x i16], ptr %85, i64 0, i64 %87
  %89 = load i16, ptr %88, align 2, !tbaa !5
  %90 = sext i16 %89 to i64
  %91 = load i16, ptr %8, align 2, !tbaa !5
  %92 = zext i16 %91 to i64
  %93 = getelementptr inbounds nuw [8 x [8 x i16]], ptr %4, i64 0, i64 %92
  %94 = load i16, ptr %7, align 2, !tbaa !5
  %95 = zext i16 %94 to i64
  %96 = getelementptr inbounds nuw [8 x i16], ptr %93, i64 0, i64 %95
  %97 = load i16, ptr %96, align 2, !tbaa !5
  %98 = sext i16 %97 to i64
  %99 = mul nsw i64 %90, %98
  %100 = load i64, ptr %10, align 8, !tbaa !9
  %101 = add nsw i64 %100, %99
  store i64 %101, ptr %10, align 8, !tbaa !9
  br label %102

102:                                              ; preds = %82
  %103 = load i16, ptr %8, align 2, !tbaa !5
  %104 = add i16 %103, 1
  store i16 %104, ptr %8, align 2, !tbaa !5
  br label %78, !llvm.loop !14

105:                                              ; preds = %78
  %106 = load i64, ptr %10, align 8, !tbaa !9
  %107 = load i16, ptr %6, align 2, !tbaa !5
  %108 = zext i16 %107 to i64
  %109 = getelementptr inbounds nuw [8 x [8 x i64]], ptr %5, i64 0, i64 %108
  %110 = load i16, ptr %7, align 2, !tbaa !5
  %111 = zext i16 %110 to i64
  %112 = getelementptr inbounds nuw [8 x i64], ptr %109, i64 0, i64 %111
  store i64 %106, ptr %112, align 8, !tbaa !9
  call void @llvm.lifetime.end.p0(i64 8, ptr %10) #4
  br label %113

113:                                              ; preds = %105
  %114 = load i16, ptr %7, align 2, !tbaa !5
  %115 = add i16 %114, 1
  store i16 %115, ptr %7, align 2, !tbaa !5
  br label %73, !llvm.loop !15

116:                                              ; preds = %73
  br label %117

117:                                              ; preds = %116
  %118 = load i16, ptr %6, align 2, !tbaa !5
  %119 = add i16 %118, 1
  store i16 %119, ptr %6, align 2, !tbaa !5
  br label %68, !llvm.loop !16

120:                                              ; preds = %68
  store i16 0, ptr %6, align 2, !tbaa !5
  br label %121

121:                                              ; preds = %153, %120
  %122 = load i16, ptr %6, align 2, !tbaa !5
  %123 = zext i16 %122 to i32
  %124 = icmp slt i32 %123, 8
  br i1 %124, label %125, label %156

125:                                              ; preds = %121
  store i16 0, ptr %7, align 2, !tbaa !5
  br label %126

126:                                              ; preds = %149, %125
  %127 = load i16, ptr %7, align 2, !tbaa !5
  %128 = zext i16 %127 to i32
  %129 = icmp slt i32 %128, 8
  br i1 %129, label %130, label %152

130:                                              ; preds = %126
  %131 = load i16, ptr %6, align 2, !tbaa !5
  %132 = zext i16 %131 to i64
  %133 = getelementptr inbounds nuw [8 x [8 x i64]], ptr %5, i64 0, i64 %132
  %134 = load i16, ptr %7, align 2, !tbaa !5
  %135 = zext i16 %134 to i64
  %136 = getelementptr inbounds nuw [8 x i64], ptr %133, i64 0, i64 %135
  %137 = load i64, ptr %136, align 8, !tbaa !9
  %138 = load i16, ptr %6, align 2, !tbaa !5
  %139 = zext i16 %138 to i32
  %140 = mul nsw i32 %139, 8
  %141 = load i16, ptr %7, align 2, !tbaa !5
  %142 = zext i16 %141 to i32
  %143 = add nsw i32 %140, %142
  %144 = add nsw i32 %143, 1
  %145 = sext i32 %144 to i64
  %146 = mul i64 %137, %145
  %147 = load i64, ptr %9, align 8, !tbaa !9
  %148 = add i64 %147, %146
  store i64 %148, ptr %9, align 8, !tbaa !9
  br label %149

149:                                              ; preds = %130
  %150 = load i16, ptr %7, align 2, !tbaa !5
  %151 = add i16 %150, 1
  store i16 %151, ptr %7, align 2, !tbaa !5
  br label %126, !llvm.loop !17

152:                                              ; preds = %126
  br label %153

153:                                              ; preds = %152
  %154 = load i16, ptr %6, align 2, !tbaa !5
  %155 = add i16 %154, 1
  store i16 %155, ptr %6, align 2, !tbaa !5
  br label %121, !llvm.loop !18

156:                                              ; preds = %121
  %157 = load i64, ptr %9, align 8, !tbaa !9
  call void @llvm.lifetime.end.p0(i64 8, ptr %9) #4
  call void @llvm.lifetime.end.p0(i64 2, ptr %8) #4
  call void @llvm.lifetime.end.p0(i64 2, ptr %7) #4
  call void @llvm.lifetime.end.p0(i64 2, ptr %6) #4
  call void @llvm.lifetime.end.p0(i64 512, ptr %5) #4
  call void @llvm.lifetime.end.p0(i64 128, ptr %4) #4
  call void @llvm.lifetime.end.p0(i64 128, ptr %3) #4
  ret i64 %157
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) #1

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) #1

; Function Attrs: nounwind
define dso_local i32 @main() #2 {
  %1 = load volatile i32, ptr @opq0, align 4, !tbaa !19
  %2 = add nsw i32 0, %1
  %3 = trunc i32 %2 to i16
  %4 = call i64 @bench_matmul(i16 noundef %3) #5
  call void @report(i64 noundef %4) #5
  ret i32 0
}

declare void @report(i64 noundef) #3

attributes #0 = { noinline nounwind "frame-pointer"="non-leaf" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { nounwind "frame-pointer"="non-leaf" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #3 = { "frame-pointer"="non-leaf" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #4 = { nounwind }
attributes #5 = { nobuiltin "no-builtins" }

!llvm.module.flags = !{!0, !1, !2, !3}
!llvm.ident = !{!4}

!0 = !{i32 1, !"wchar_size", i32 4}
!1 = !{i32 8, !"PIC Level", i32 2}
!2 = !{i32 7, !"PIE Level", i32 2}
!3 = !{i32 7, !"frame-pointer", i32 1}
!4 = !{!"Ubuntu clang version 20.1.8 (0ubuntu4)"}
!5 = !{!6, !6, i64 0}
!6 = !{!"short", !7, i64 0}
!7 = !{!"omnipotent char", !8, i64 0}
!8 = !{!"Simple C/C++ TBAA"}
!9 = !{!10, !10, i64 0}
!10 = !{!"long", !7, i64 0}
!11 = distinct !{!11, !12}
!12 = !{!"llvm.loop.mustprogress"}
!13 = distinct !{!13, !12}
!14 = distinct !{!14, !12}
!15 = distinct !{!15, !12}
!16 = distinct !{!16, !12}
!17 = distinct !{!17, !12}
!18 = distinct !{!18, !12}
!19 = !{!20, !20, i64 0}
!20 = !{!"int", !7, i64 0}
