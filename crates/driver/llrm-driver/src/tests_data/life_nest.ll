target datalayout = "@m32-datalayout@"

@g = internal global [2 x [24 x [24 x i8]]] zeroinitializer, align 1
@bench_x_life_via = dso_local global ptr @bench_x_life, align 4

; Function Attrs: nounwind
define dso_local i32 @bench_x_life(i32 noundef %0) #0 {
  %2 = alloca i32, align 4
  %3 = alloca i32, align 4
  %4 = alloca i32, align 4
  %5 = alloca i32, align 4
  %6 = alloca i32, align 4
  %7 = alloca i32, align 4
  %8 = alloca i32, align 4
  %9 = alloca i32, align 4
  %10 = alloca i32, align 4
  store i32 %0, ptr %2, align 4, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %3) #3
  call void @llvm.lifetime.start.p0(ptr %4) #3
  call void @llvm.lifetime.start.p0(ptr %5) #3
  call void @llvm.lifetime.start.p0(ptr %6) #3
  store i32 0, ptr %6, align 4, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %7) #3
  call void @llvm.lifetime.start.p0(ptr %8) #3
  call void @llvm.lifetime.start.p0(ptr %9) #3
  store i32 0, ptr %9, align 4, !tbaa !8
  store i32 0, ptr %4, align 4, !tbaa !7
  br label %11

11:                                               ; preds = %38, %1
  %12 = load i32, ptr %4, align 4, !tbaa !7
  %13 = load i32, ptr %2, align 4, !tbaa !7
  %14 = icmp slt i32 %12, %13
  br i1 %14, label %15, label %41

15:                                               ; preds = %11
  store i32 0, ptr %3, align 4, !tbaa !7
  br label %16

16:                                               ; preds = %34, %15
  %17 = load i32, ptr %3, align 4, !tbaa !7
  %18 = load i32, ptr %2, align 4, !tbaa !7
  %19 = icmp slt i32 %17, %18
  br i1 %19, label %20, label %37

20:                                               ; preds = %16
  %21 = load i32, ptr %3, align 4, !tbaa !7
  %22 = mul nsw i32 %21, 7
  %23 = load i32, ptr %4, align 4, !tbaa !7
  %24 = mul nsw i32 %23, 13
  %25 = add nsw i32 %22, %24
  %26 = srem i32 %25, 5
  %27 = icmp eq i32 %26, 0
  %28 = zext i1 %27 to i32
  %29 = trunc i32 %28 to i8
  %30 = load i32, ptr %4, align 4, !tbaa !7
  %31 = getelementptr inbounds [24 x [24 x i8]], ptr @g, i32 0, i32 %30
  %32 = load i32, ptr %3, align 4, !tbaa !7
  %33 = getelementptr inbounds [24 x i8], ptr %31, i32 0, i32 %32
  store i8 %29, ptr %33, align 1, !tbaa !10
  br label %34

34:                                               ; preds = %20
  %35 = load i32, ptr %3, align 4, !tbaa !7
  %36 = add nsw i32 %35, 1
  store i32 %36, ptr %3, align 4, !tbaa !7
  br label %16, !llvm.loop !11

37:                                               ; preds = %16
  br label %38

38:                                               ; preds = %37
  %39 = load i32, ptr %4, align 4, !tbaa !7
  %40 = add nsw i32 %39, 1
  store i32 %40, ptr %4, align 4, !tbaa !7
  br label %11, !llvm.loop !13

41:                                               ; preds = %11
  store i32 0, ptr %5, align 4, !tbaa !7
  br label %42

42:                                               ; preds = %136, %41
  %43 = load i32, ptr %5, align 4, !tbaa !7
  %44 = icmp slt i32 %43, 10
  br i1 %44, label %45, label %139

45:                                               ; preds = %42
  store i32 1, ptr %4, align 4, !tbaa !7
  br label %46

46:                                               ; preds = %130, %45
  %47 = load i32, ptr %4, align 4, !tbaa !7
  %48 = load i32, ptr %2, align 4, !tbaa !7
  %49 = sub nsw i32 %48, 1
  %50 = icmp slt i32 %47, %49
  br i1 %50, label %51, label %133

51:                                               ; preds = %46
  store i32 1, ptr %3, align 4, !tbaa !7
  br label %52

52:                                               ; preds = %126, %51
  %53 = load i32, ptr %3, align 4, !tbaa !7
  %54 = load i32, ptr %2, align 4, !tbaa !7
  %55 = sub nsw i32 %54, 1
  %56 = icmp slt i32 %53, %55
  br i1 %56, label %57, label %129

57:                                               ; preds = %52
  call void @llvm.lifetime.start.p0(ptr %10) #3
  store i32 0, ptr %10, align 4, !tbaa !7
  store i32 -1, ptr %8, align 4, !tbaa !7
  br label %58

58:                                               ; preds = %84, %57
  %59 = load i32, ptr %8, align 4, !tbaa !7
  %60 = icmp sle i32 %59, 1
  br i1 %60, label %61, label %87

61:                                               ; preds = %58
  store i32 -1, ptr %7, align 4, !tbaa !7
  br label %62

62:                                               ; preds = %80, %61
  %63 = load i32, ptr %7, align 4, !tbaa !7
  %64 = icmp sle i32 %63, 1
  br i1 %64, label %65, label %83

65:                                               ; preds = %62
  %66 = load i32, ptr %6, align 4, !tbaa !7
  %67 = getelementptr inbounds [2 x [24 x [24 x i8]]], ptr @g, i32 0, i32 %66
  %68 = load i32, ptr %4, align 4, !tbaa !7
  %69 = load i32, ptr %8, align 4, !tbaa !7
  %70 = add nsw i32 %68, %69
  %71 = getelementptr inbounds [24 x [24 x i8]], ptr %67, i32 0, i32 %70
  %72 = load i32, ptr %3, align 4, !tbaa !7
  %73 = load i32, ptr %7, align 4, !tbaa !7
  %74 = add nsw i32 %72, %73
  %75 = getelementptr inbounds [24 x i8], ptr %71, i32 0, i32 %74
  %76 = load i8, ptr %75, align 1, !tbaa !10
  %77 = zext i8 %76 to i32
  %78 = load i32, ptr %10, align 4, !tbaa !7
  %79 = add nsw i32 %78, %77
  store i32 %79, ptr %10, align 4, !tbaa !7
  br label %80

80:                                               ; preds = %65
  %81 = load i32, ptr %7, align 4, !tbaa !7
  %82 = add nsw i32 %81, 1
  store i32 %82, ptr %7, align 4, !tbaa !7
  br label %62, !llvm.loop !14

83:                                               ; preds = %62
  br label %84

84:                                               ; preds = %83
  %85 = load i32, ptr %8, align 4, !tbaa !7
  %86 = add nsw i32 %85, 1
  store i32 %86, ptr %8, align 4, !tbaa !7
  br label %58, !llvm.loop !15

87:                                               ; preds = %58
  %88 = load i32, ptr %6, align 4, !tbaa !7
  %89 = getelementptr inbounds [2 x [24 x [24 x i8]]], ptr @g, i32 0, i32 %88
  %90 = load i32, ptr %4, align 4, !tbaa !7
  %91 = getelementptr inbounds [24 x [24 x i8]], ptr %89, i32 0, i32 %90
  %92 = load i32, ptr %3, align 4, !tbaa !7
  %93 = getelementptr inbounds [24 x i8], ptr %91, i32 0, i32 %92
  %94 = load i8, ptr %93, align 1, !tbaa !10
  %95 = zext i8 %94 to i32
  %96 = load i32, ptr %10, align 4, !tbaa !7
  %97 = sub nsw i32 %96, %95
  store i32 %97, ptr %10, align 4, !tbaa !7
  %98 = load i32, ptr %10, align 4, !tbaa !7
  %99 = icmp eq i32 %98, 3
  br i1 %99, label %115, label %100

100:                                              ; preds = %87
  %101 = load i32, ptr %10, align 4, !tbaa !7
  %102 = icmp eq i32 %101, 2
  br i1 %102, label %103, label %113

103:                                              ; preds = %100
  %104 = load i32, ptr %6, align 4, !tbaa !7
  %105 = getelementptr inbounds [2 x [24 x [24 x i8]]], ptr @g, i32 0, i32 %104
  %106 = load i32, ptr %4, align 4, !tbaa !7
  %107 = getelementptr inbounds [24 x [24 x i8]], ptr %105, i32 0, i32 %106
  %108 = load i32, ptr %3, align 4, !tbaa !7
  %109 = getelementptr inbounds [24 x i8], ptr %107, i32 0, i32 %108
  %110 = load i8, ptr %109, align 1, !tbaa !10
  %111 = zext i8 %110 to i32
  %112 = icmp ne i32 %111, 0
  br label %113

113:                                              ; preds = %103, %100
  %114 = phi i1 [ false, %100 ], [ %112, %103 ]
  br label %115

115:                                              ; preds = %113, %87
  %116 = phi i1 [ true, %87 ], [ %114, %113 ]
  %117 = zext i1 %116 to i32
  %118 = trunc i32 %117 to i8
  %119 = load i32, ptr %6, align 4, !tbaa !7
  %120 = sub nsw i32 1, %119
  %121 = getelementptr inbounds [2 x [24 x [24 x i8]]], ptr @g, i32 0, i32 %120
  %122 = load i32, ptr %4, align 4, !tbaa !7
  %123 = getelementptr inbounds [24 x [24 x i8]], ptr %121, i32 0, i32 %122
  %124 = load i32, ptr %3, align 4, !tbaa !7
  %125 = getelementptr inbounds [24 x i8], ptr %123, i32 0, i32 %124
  store i8 %118, ptr %125, align 1, !tbaa !10
  call void @llvm.lifetime.end.p0(ptr %10) #3
  br label %126

126:                                              ; preds = %115
  %127 = load i32, ptr %3, align 4, !tbaa !7
  %128 = add nsw i32 %127, 1
  store i32 %128, ptr %3, align 4, !tbaa !7
  br label %52, !llvm.loop !16

129:                                              ; preds = %52
  br label %130

130:                                              ; preds = %129
  %131 = load i32, ptr %4, align 4, !tbaa !7
  %132 = add nsw i32 %131, 1
  store i32 %132, ptr %4, align 4, !tbaa !7
  br label %46, !llvm.loop !17

133:                                              ; preds = %46
  %134 = load i32, ptr %6, align 4, !tbaa !7
  %135 = sub nsw i32 1, %134
  store i32 %135, ptr %6, align 4, !tbaa !7
  br label %136

136:                                              ; preds = %133
  %137 = load i32, ptr %5, align 4, !tbaa !7
  %138 = add nsw i32 %137, 1
  store i32 %138, ptr %5, align 4, !tbaa !7
  br label %42, !llvm.loop !18

139:                                              ; preds = %42
  store i32 0, ptr %4, align 4, !tbaa !7
  br label %140

140:                                              ; preds = %164, %139
  %141 = load i32, ptr %4, align 4, !tbaa !7
  %142 = load i32, ptr %2, align 4, !tbaa !7
  %143 = icmp slt i32 %141, %142
  br i1 %143, label %144, label %167

144:                                              ; preds = %140
  store i32 0, ptr %3, align 4, !tbaa !7
  br label %145

145:                                              ; preds = %160, %144
  %146 = load i32, ptr %3, align 4, !tbaa !7
  %147 = load i32, ptr %2, align 4, !tbaa !7
  %148 = icmp slt i32 %146, %147
  br i1 %148, label %149, label %163

149:                                              ; preds = %145
  %150 = load i32, ptr %6, align 4, !tbaa !7
  %151 = getelementptr inbounds [2 x [24 x [24 x i8]]], ptr @g, i32 0, i32 %150
  %152 = load i32, ptr %4, align 4, !tbaa !7
  %153 = getelementptr inbounds [24 x [24 x i8]], ptr %151, i32 0, i32 %152
  %154 = load i32, ptr %3, align 4, !tbaa !7
  %155 = getelementptr inbounds [24 x i8], ptr %153, i32 0, i32 %154
  %156 = load i8, ptr %155, align 1, !tbaa !10
  %157 = zext i8 %156 to i32
  %158 = load i32, ptr %9, align 4, !tbaa !8
  %159 = add nsw i32 %158, %157
  store i32 %159, ptr %9, align 4, !tbaa !8
  br label %160

160:                                              ; preds = %149
  %161 = load i32, ptr %3, align 4, !tbaa !7
  %162 = add nsw i32 %161, 1
  store i32 %162, ptr %3, align 4, !tbaa !7
  br label %145, !llvm.loop !19

163:                                              ; preds = %145
  br label %164

164:                                              ; preds = %163
  %165 = load i32, ptr %4, align 4, !tbaa !7
  %166 = add nsw i32 %165, 1
  store i32 %166, ptr %4, align 4, !tbaa !7
  br label %140, !llvm.loop !20

167:                                              ; preds = %140
  %168 = load i32, ptr %9, align 4, !tbaa !8
  call void @llvm.lifetime.end.p0(ptr %9) #3
  call void @llvm.lifetime.end.p0(ptr %8) #3
  call void @llvm.lifetime.end.p0(ptr %7) #3
  call void @llvm.lifetime.end.p0(ptr %6) #3
  call void @llvm.lifetime.end.p0(ptr %5) #3
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  ret i32 %168
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

; Function Attrs: nounwind
define dso_local i32 @main() #0 {
  %1 = alloca i32, align 4
  store i32 0, ptr %1, align 4
  %2 = load volatile ptr, ptr @bench_x_life_via, align 4, !tbaa !21
  %3 = call i32 %2(i32 noundef 24)
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
!7 = !{!4, !4, i64 0}
!8 = !{!9, !9, i64 0}
!9 = !{!"long", !5, i64 0}
!10 = !{!5, !5, i64 0}
!11 = distinct !{!11, !12}
!12 = !{!"llvm.loop.mustprogress"}
!13 = distinct !{!13, !12}
!14 = distinct !{!14, !12}
!15 = distinct !{!15, !12}
!16 = distinct !{!16, !12}
!17 = distinct !{!17, !12}
!18 = distinct !{!18, !12}
!19 = distinct !{!19, !12}
!20 = distinct !{!20, !12}
!21 = !{!22, !22, i64 0}
!22 = !{!"any pointer", !5, i64 0}
