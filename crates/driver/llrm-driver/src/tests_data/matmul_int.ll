target datalayout = "@m32-datalayout@"

@bench_matmul_via = dso_local global ptr @bench_matmul, align 4

; Function Attrs: nounwind
define dso_local i32 @bench_matmul(i16 noundef zeroext %0) #0 {
  %2 = alloca i16, align 2
  %3 = alloca [8 x [8 x i16]], align 2
  %4 = alloca [8 x [8 x i16]], align 2
  %5 = alloca [8 x [8 x i32]], align 4
  %6 = alloca i16, align 2
  %7 = alloca i16, align 2
  %8 = alloca i16, align 2
  %9 = alloca i32, align 4
  %10 = alloca i32, align 4
  store i16 %0, ptr %2, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %3) #3
  call void @llvm.lifetime.start.p0(ptr %4) #3
  call void @llvm.lifetime.start.p0(ptr %5) #3
  call void @llvm.lifetime.start.p0(ptr %6) #3
  call void @llvm.lifetime.start.p0(ptr %7) #3
  call void @llvm.lifetime.start.p0(ptr %8) #3
  call void @llvm.lifetime.start.p0(ptr %9) #3
  store i32 0, ptr %9, align 4, !tbaa !9
  store i16 0, ptr %6, align 2, !tbaa !7
  br label %11

11:                                               ; preds = %64, %1
  %12 = load i16, ptr %6, align 2, !tbaa !7
  %13 = zext i16 %12 to i32
  %14 = icmp slt i32 %13, 8
  br i1 %14, label %15, label %67

15:                                               ; preds = %11
  store i16 0, ptr %7, align 2, !tbaa !7
  br label %16

16:                                               ; preds = %60, %15
  %17 = load i16, ptr %7, align 2, !tbaa !7
  %18 = zext i16 %17 to i32
  %19 = icmp slt i32 %18, 8
  br i1 %19, label %20, label %63

20:                                               ; preds = %16
  %21 = load i16, ptr %6, align 2, !tbaa !7
  %22 = zext i16 %21 to i32
  %23 = mul nsw i32 %22, 3
  %24 = load i16, ptr %7, align 2, !tbaa !7
  %25 = zext i16 %24 to i32
  %26 = add nsw i32 %23, %25
  %27 = add nsw i32 %26, 1
  %28 = load i16, ptr %2, align 2, !tbaa !7
  %29 = zext i16 %28 to i32
  %30 = add nsw i32 %27, %29
  %31 = trunc i32 %30 to i16
  %32 = load i16, ptr %6, align 2, !tbaa !7
  %33 = zext i16 %32 to i32
  %34 = getelementptr inbounds nuw [8 x [8 x i16]], ptr %3, i32 0, i32 %33
  %35 = load i16, ptr %7, align 2, !tbaa !7
  %36 = zext i16 %35 to i32
  %37 = getelementptr inbounds nuw [8 x i16], ptr %34, i32 0, i32 %36
  store i16 %31, ptr %37, align 2, !tbaa !7
  %38 = load i16, ptr %6, align 2, !tbaa !7
  %39 = zext i16 %38 to i32
  %40 = load i16, ptr %7, align 2, !tbaa !7
  %41 = zext i16 %40 to i32
  %42 = icmp eq i32 %39, %41
  br i1 %42, label %43, label %44

43:                                               ; preds = %20
  br label %51

44:                                               ; preds = %20
  %45 = load i16, ptr %6, align 2, !tbaa !7
  %46 = zext i16 %45 to i32
  %47 = load i16, ptr %7, align 2, !tbaa !7
  %48 = zext i16 %47 to i32
  %49 = add nsw i32 %46, %48
  %50 = srem i32 %49, 3
  br label %51

51:                                               ; preds = %44, %43
  %52 = phi i32 [ 2, %43 ], [ %50, %44 ]
  %53 = trunc i32 %52 to i16
  %54 = load i16, ptr %6, align 2, !tbaa !7
  %55 = zext i16 %54 to i32
  %56 = getelementptr inbounds nuw [8 x [8 x i16]], ptr %4, i32 0, i32 %55
  %57 = load i16, ptr %7, align 2, !tbaa !7
  %58 = zext i16 %57 to i32
  %59 = getelementptr inbounds nuw [8 x i16], ptr %56, i32 0, i32 %58
  store i16 %53, ptr %59, align 2, !tbaa !7
  br label %60

60:                                               ; preds = %51
  %61 = load i16, ptr %7, align 2, !tbaa !7
  %62 = add i16 %61, 1
  store i16 %62, ptr %7, align 2, !tbaa !7
  br label %16, !llvm.loop !11

63:                                               ; preds = %16
  br label %64

64:                                               ; preds = %63
  %65 = load i16, ptr %6, align 2, !tbaa !7
  %66 = add i16 %65, 1
  store i16 %66, ptr %6, align 2, !tbaa !7
  br label %11, !llvm.loop !13

67:                                               ; preds = %11
  store i16 0, ptr %6, align 2, !tbaa !7
  br label %68

68:                                               ; preds = %117, %67
  %69 = load i16, ptr %6, align 2, !tbaa !7
  %70 = zext i16 %69 to i32
  %71 = icmp slt i32 %70, 8
  br i1 %71, label %72, label %120

72:                                               ; preds = %68
  store i16 0, ptr %7, align 2, !tbaa !7
  br label %73

73:                                               ; preds = %113, %72
  %74 = load i16, ptr %7, align 2, !tbaa !7
  %75 = zext i16 %74 to i32
  %76 = icmp slt i32 %75, 8
  br i1 %76, label %77, label %116

77:                                               ; preds = %73
  call void @llvm.lifetime.start.p0(ptr %10) #3
  store i32 0, ptr %10, align 4, !tbaa !9
  store i16 0, ptr %8, align 2, !tbaa !7
  br label %78

78:                                               ; preds = %102, %77
  %79 = load i16, ptr %8, align 2, !tbaa !7
  %80 = zext i16 %79 to i32
  %81 = icmp slt i32 %80, 8
  br i1 %81, label %82, label %105

82:                                               ; preds = %78
  %83 = load i16, ptr %6, align 2, !tbaa !7
  %84 = zext i16 %83 to i32
  %85 = getelementptr inbounds nuw [8 x [8 x i16]], ptr %3, i32 0, i32 %84
  %86 = load i16, ptr %8, align 2, !tbaa !7
  %87 = zext i16 %86 to i32
  %88 = getelementptr inbounds nuw [8 x i16], ptr %85, i32 0, i32 %87
  %89 = load i16, ptr %88, align 2, !tbaa !7
  %90 = sext i16 %89 to i32
  %91 = load i16, ptr %8, align 2, !tbaa !7
  %92 = zext i16 %91 to i32
  %93 = getelementptr inbounds nuw [8 x [8 x i16]], ptr %4, i32 0, i32 %92
  %94 = load i16, ptr %7, align 2, !tbaa !7
  %95 = zext i16 %94 to i32
  %96 = getelementptr inbounds nuw [8 x i16], ptr %93, i32 0, i32 %95
  %97 = load i16, ptr %96, align 2, !tbaa !7
  %98 = sext i16 %97 to i32
  %99 = mul nsw i32 %90, %98
  %100 = load i32, ptr %10, align 4, !tbaa !9
  %101 = add nsw i32 %100, %99
  store i32 %101, ptr %10, align 4, !tbaa !9
  br label %102

102:                                              ; preds = %82
  %103 = load i16, ptr %8, align 2, !tbaa !7
  %104 = add i16 %103, 1
  store i16 %104, ptr %8, align 2, !tbaa !7
  br label %78, !llvm.loop !14

105:                                              ; preds = %78
  %106 = load i32, ptr %10, align 4, !tbaa !9
  %107 = load i16, ptr %6, align 2, !tbaa !7
  %108 = zext i16 %107 to i32
  %109 = getelementptr inbounds nuw [8 x [8 x i32]], ptr %5, i32 0, i32 %108
  %110 = load i16, ptr %7, align 2, !tbaa !7
  %111 = zext i16 %110 to i32
  %112 = getelementptr inbounds nuw [8 x i32], ptr %109, i32 0, i32 %111
  store i32 %106, ptr %112, align 4, !tbaa !9
  call void @llvm.lifetime.end.p0(ptr %10) #3
  br label %113

113:                                              ; preds = %105
  %114 = load i16, ptr %7, align 2, !tbaa !7
  %115 = add i16 %114, 1
  store i16 %115, ptr %7, align 2, !tbaa !7
  br label %73, !llvm.loop !15

116:                                              ; preds = %73
  br label %117

117:                                              ; preds = %116
  %118 = load i16, ptr %6, align 2, !tbaa !7
  %119 = add i16 %118, 1
  store i16 %119, ptr %6, align 2, !tbaa !7
  br label %68, !llvm.loop !16

120:                                              ; preds = %68
  store i16 0, ptr %6, align 2, !tbaa !7
  br label %121

121:                                              ; preds = %152, %120
  %122 = load i16, ptr %6, align 2, !tbaa !7
  %123 = zext i16 %122 to i32
  %124 = icmp slt i32 %123, 8
  br i1 %124, label %125, label %155

125:                                              ; preds = %121
  store i16 0, ptr %7, align 2, !tbaa !7
  br label %126

126:                                              ; preds = %148, %125
  %127 = load i16, ptr %7, align 2, !tbaa !7
  %128 = zext i16 %127 to i32
  %129 = icmp slt i32 %128, 8
  br i1 %129, label %130, label %151

130:                                              ; preds = %126
  %131 = load i16, ptr %6, align 2, !tbaa !7
  %132 = zext i16 %131 to i32
  %133 = getelementptr inbounds nuw [8 x [8 x i32]], ptr %5, i32 0, i32 %132
  %134 = load i16, ptr %7, align 2, !tbaa !7
  %135 = zext i16 %134 to i32
  %136 = getelementptr inbounds nuw [8 x i32], ptr %133, i32 0, i32 %135
  %137 = load i32, ptr %136, align 4, !tbaa !9
  %138 = load i16, ptr %6, align 2, !tbaa !7
  %139 = zext i16 %138 to i32
  %140 = mul nsw i32 %139, 8
  %141 = load i16, ptr %7, align 2, !tbaa !7
  %142 = zext i16 %141 to i32
  %143 = add nsw i32 %140, %142
  %144 = add nsw i32 %143, 1
  %145 = mul i32 %137, %144
  %146 = load i32, ptr %9, align 4, !tbaa !9
  %147 = add i32 %146, %145
  store i32 %147, ptr %9, align 4, !tbaa !9
  br label %148

148:                                              ; preds = %130
  %149 = load i16, ptr %7, align 2, !tbaa !7
  %150 = add i16 %149, 1
  store i16 %150, ptr %7, align 2, !tbaa !7
  br label %126, !llvm.loop !17

151:                                              ; preds = %126
  br label %152

152:                                              ; preds = %151
  %153 = load i16, ptr %6, align 2, !tbaa !7
  %154 = add i16 %153, 1
  store i16 %154, ptr %6, align 2, !tbaa !7
  br label %121, !llvm.loop !18

155:                                              ; preds = %121
  %156 = load i32, ptr %9, align 4, !tbaa !9
  call void @llvm.lifetime.end.p0(ptr %9) #3
  call void @llvm.lifetime.end.p0(ptr %8) #3
  call void @llvm.lifetime.end.p0(ptr %7) #3
  call void @llvm.lifetime.end.p0(ptr %6) #3
  call void @llvm.lifetime.end.p0(ptr %5) #3
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  ret i32 %156
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

; Function Attrs: nounwind
define dso_local i32 @main() #0 {
  %1 = alloca i32, align 4
  store i32 0, ptr %1, align 4
  %2 = load volatile ptr, ptr @bench_matmul_via, align 4, !tbaa !19
  %3 = call i32 %2(i16 noundef zeroext 0)
  call void @report(i32 noundef %3)
  ret i32 0
}

declare dso_local void @report(i32 noundef) #2

attributes #0 = { nounwind "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="i486" "target-features"="+x87" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="i486" "target-features"="+x87" }
attributes #3 = { nounwind }

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
!8 = !{!"short", !5, i64 0}
!9 = !{!10, !10, i64 0}
!10 = !{!"long", !5, i64 0}
!11 = distinct !{!11, !12}
!12 = !{!"llvm.loop.mustprogress"}
!13 = distinct !{!13, !12}
!14 = distinct !{!14, !12}
!15 = distinct !{!15, !12}
!16 = distinct !{!16, !12}
!17 = distinct !{!17, !12}
!18 = distinct !{!18, !12}
!19 = !{!20, !20, i64 0}
!20 = !{!"any pointer", !5, i64 0}
