target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"
; ModuleID = 'raw.ll'

@sum = external hidden global i32, align 2
@state = external hidden global i32, align 2

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #0

; Function Attrs: noinline nounwind optsize
define hidden i32 @c_short_7() #1 {
  %1 = alloca i16, align 2
  %2 = alloca i16, align 2
  %3 = alloca i16, align 2
  %4 = alloca i16, align 2
  %5 = alloca i32, align 2
  %6 = alloca i32, align 2
  %7 = alloca i32, align 2
  %8 = alloca i32, align 2
  %9 = alloca i16, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #2
  store volatile i16 7, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %2) #2
  call void @llvm.lifetime.start.p0(ptr %3) #2
  call void @llvm.lifetime.start.p0(ptr %4) #2
  call void @llvm.lifetime.start.p0(ptr %5) #2
  store i32 0, ptr %5, align 2, !tbaa !9
  call void @llvm.lifetime.start.p0(ptr %6) #2
  store i32 0, ptr %6, align 2, !tbaa !9
  br label %10

10:                                               ; preds = %84, %0
  %11 = load i32, ptr %6, align 2, !tbaa !9
  %12 = icmp slt i32 %11, 1500
  br i1 %12, label %13, label %87

13:                                               ; preds = %10
  call void @llvm.lifetime.start.p0(ptr %7) #2
  %14 = call i32 @next() #3
  store i32 %14, ptr %7, align 2, !tbaa !9
  call void @llvm.lifetime.start.p0(ptr %8) #2
  %15 = call i32 @next() #3
  %16 = and i32 %15, 7
  store i32 %16, ptr %8, align 2, !tbaa !9
  %17 = load i32, ptr %8, align 2, !tbaa !9
  %18 = icmp eq i32 %17, 0
  br i1 %18, label %19, label %21

19:                                               ; preds = %13
  %20 = load i32, ptr %7, align 2, !tbaa !9
  br label %47

21:                                               ; preds = %13
  %22 = load i32, ptr %8, align 2, !tbaa !9
  %23 = icmp eq i32 %22, 1
  br i1 %23, label %24, label %27

24:                                               ; preds = %21
  %25 = load i32, ptr %7, align 2, !tbaa !9
  %26 = lshr i32 %25, 16
  br label %45

27:                                               ; preds = %21
  %28 = load i32, ptr %8, align 2, !tbaa !9
  %29 = icmp eq i32 %28, 2
  br i1 %29, label %30, label %33

30:                                               ; preds = %27
  %31 = load i32, ptr %7, align 2, !tbaa !9
  %32 = lshr i32 %31, 24
  br label %43

33:                                               ; preds = %27
  %34 = load i32, ptr %8, align 2, !tbaa !9
  %35 = icmp eq i32 %34, 3
  br i1 %35, label %36, label %39

36:                                               ; preds = %33
  %37 = load i32, ptr %7, align 2, !tbaa !9
  %38 = lshr i32 %37, 29
  br label %41

39:                                               ; preds = %33
  %40 = load i32, ptr %7, align 2, !tbaa !9
  br label %41

41:                                               ; preds = %39, %36
  %42 = phi i32 [ %38, %36 ], [ %40, %39 ]
  br label %43

43:                                               ; preds = %41, %30
  %44 = phi i32 [ %32, %30 ], [ %42, %41 ]
  br label %45

45:                                               ; preds = %43, %24
  %46 = phi i32 [ %26, %24 ], [ %44, %43 ]
  br label %47

47:                                               ; preds = %45, %19
  %48 = phi i32 [ %20, %19 ], [ %46, %45 ]
  %49 = trunc i32 %48 to i16
  store i16 %49, ptr %2, align 2, !tbaa !7
  %50 = call i32 @next() #3
  %51 = and i32 %50, 1
  %52 = icmp ne i32 %51, 0
  br i1 %52, label %53, label %56

53:                                               ; preds = %47
  %54 = load i16, ptr %2, align 2, !tbaa !7
  %55 = sub nsw i16 0, %54
  store i16 %55, ptr %2, align 2, !tbaa !7
  br label %56

56:                                               ; preds = %53, %47
  %57 = load i16, ptr %2, align 2, !tbaa !7
  %58 = sdiv i16 %57, 7
  store i16 %58, ptr %3, align 2, !tbaa !7
  %59 = load i16, ptr %2, align 2, !tbaa !7
  %60 = srem i16 %59, 7
  store i16 %60, ptr %4, align 2, !tbaa !7
  %61 = load i16, ptr %3, align 2, !tbaa !7
  %62 = load i16, ptr %2, align 2, !tbaa !7
  %63 = load volatile i16, ptr %1, align 2, !tbaa !7
  %64 = sdiv i16 %62, %63
  %65 = icmp ne i16 %61, %64
  br i1 %65, label %72, label %66

66:                                               ; preds = %56
  %67 = load i16, ptr %4, align 2, !tbaa !7
  %68 = load i16, ptr %2, align 2, !tbaa !7
  %69 = load volatile i16, ptr %1, align 2, !tbaa !7
  %70 = srem i16 %68, %69
  %71 = icmp ne i16 %67, %70
  br i1 %71, label %72, label %75

72:                                               ; preds = %66, %56
  %73 = load i32, ptr %5, align 2, !tbaa !9
  %74 = add nsw i32 %73, 1
  store i32 %74, ptr %5, align 2, !tbaa !9
  br label %75

75:                                               ; preds = %72, %66
  %76 = load i16, ptr %3, align 2, !tbaa !7
  %77 = sext i16 %76 to i32
  %78 = mul i32 %77, 31
  %79 = load i16, ptr %4, align 2, !tbaa !7
  %80 = sext i16 %79 to i32
  %81 = add i32 %78, %80
  %82 = load i32, ptr @sum, align 2, !tbaa !9
  %83 = add i32 %82, %81
  store i32 %83, ptr @sum, align 2, !tbaa !9
  call void @llvm.lifetime.end.p0(ptr %8) #2
  call void @llvm.lifetime.end.p0(ptr %7) #2
  br label %84

84:                                               ; preds = %75
  %85 = load i32, ptr %6, align 2, !tbaa !9
  %86 = add nsw i32 %85, 1
  store i32 %86, ptr %6, align 2, !tbaa !9
  br label %10, !llvm.loop !11

87:                                               ; preds = %10
  store i32 0, ptr %6, align 2, !tbaa !9
  br label %88

88:                                               ; preds = %178, %87
  %89 = load i32, ptr %6, align 2, !tbaa !9
  %90 = icmp slt i32 %89, 12
  br i1 %90, label %91, label %181

91:                                               ; preds = %88
  call void @llvm.lifetime.start.p0(ptr %9) #2
  store i16 16383, ptr %9, align 2, !tbaa !7
  %92 = load i32, ptr %6, align 2, !tbaa !9
  %93 = icmp slt i32 %92, 2
  br i1 %93, label %94, label %96

94:                                               ; preds = %91
  %95 = load i32, ptr %6, align 2, !tbaa !9
  br label %148

96:                                               ; preds = %91
  %97 = load i32, ptr %6, align 2, !tbaa !9
  %98 = icmp slt i32 %97, 4
  br i1 %98, label %99, label %105

99:                                               ; preds = %96
  %100 = load i16, ptr %9, align 2, !tbaa !7
  %101 = sext i16 %100 to i32
  %102 = load i32, ptr %6, align 2, !tbaa !9
  %103 = sub nsw i32 %102, 2
  %104 = sub nsw i32 %101, %103
  br label %146

105:                                              ; preds = %96
  %106 = load i32, ptr %6, align 2, !tbaa !9
  %107 = icmp slt i32 %106, 6
  br i1 %107, label %108, label %112

108:                                              ; preds = %105
  %109 = load i32, ptr %6, align 2, !tbaa !9
  %110 = sub nsw i32 %109, 5
  %111 = add nsw i32 7, %110
  br label %144

112:                                              ; preds = %105
  %113 = load i32, ptr %6, align 2, !tbaa !9
  %114 = icmp slt i32 %113, 8
  br i1 %114, label %115, label %123

115:                                              ; preds = %112
  %116 = load i16, ptr %9, align 2, !tbaa !7
  %117 = sdiv i16 %116, 7
  %118 = mul nsw i16 %117, 7
  %119 = sext i16 %118 to i32
  %120 = load i32, ptr %6, align 2, !tbaa !9
  %121 = sub nsw i32 %120, 6
  %122 = sub nsw i32 %119, %121
  br label %142

123:                                              ; preds = %112
  %124 = load i32, ptr %6, align 2, !tbaa !9
  %125 = icmp slt i32 %124, 10
  br i1 %125, label %126, label %134

126:                                              ; preds = %123
  %127 = load i16, ptr %9, align 2, !tbaa !7
  %128 = sub nsw i16 %127, 7
  %129 = sext i16 %128 to i32
  %130 = load i32, ptr %6, align 2, !tbaa !9
  %131 = sub nsw i32 %130, 8
  %132 = add nsw i32 %129, %131
  %133 = trunc i32 %132 to i16
  br label %139

134:                                              ; preds = %123
  %135 = load i32, ptr %6, align 2, !tbaa !9
  %136 = sub nsw i32 %135, 10
  %137 = sub nsw i32 14, %136
  %138 = trunc i32 %137 to i16
  br label %139

139:                                              ; preds = %134, %126
  %140 = phi i16 [ %133, %126 ], [ %138, %134 ]
  %141 = sext i16 %140 to i32
  br label %142

142:                                              ; preds = %139, %115
  %143 = phi i32 [ %122, %115 ], [ %141, %139 ]
  br label %144

144:                                              ; preds = %142, %108
  %145 = phi i32 [ %111, %108 ], [ %143, %142 ]
  br label %146

146:                                              ; preds = %144, %99
  %147 = phi i32 [ %104, %99 ], [ %145, %144 ]
  br label %148

148:                                              ; preds = %146, %94
  %149 = phi i32 [ %95, %94 ], [ %147, %146 ]
  %150 = trunc i32 %149 to i16
  store i16 %150, ptr %2, align 2, !tbaa !7
  %151 = load i16, ptr %2, align 2, !tbaa !7
  %152 = sdiv i16 %151, 7
  store i16 %152, ptr %3, align 2, !tbaa !7
  %153 = load i16, ptr %2, align 2, !tbaa !7
  %154 = srem i16 %153, 7
  store i16 %154, ptr %4, align 2, !tbaa !7
  %155 = load i16, ptr %3, align 2, !tbaa !7
  %156 = load i16, ptr %2, align 2, !tbaa !7
  %157 = load volatile i16, ptr %1, align 2, !tbaa !7
  %158 = sdiv i16 %156, %157
  %159 = icmp ne i16 %155, %158
  br i1 %159, label %166, label %160

160:                                              ; preds = %148
  %161 = load i16, ptr %4, align 2, !tbaa !7
  %162 = load i16, ptr %2, align 2, !tbaa !7
  %163 = load volatile i16, ptr %1, align 2, !tbaa !7
  %164 = srem i16 %162, %163
  %165 = icmp ne i16 %161, %164
  br i1 %165, label %166, label %169

166:                                              ; preds = %160, %148
  %167 = load i32, ptr %5, align 2, !tbaa !9
  %168 = add nsw i32 %167, 1
  store i32 %168, ptr %5, align 2, !tbaa !9
  br label %169

169:                                              ; preds = %166, %160
  %170 = load i16, ptr %3, align 2, !tbaa !7
  %171 = sext i16 %170 to i32
  %172 = mul i32 %171, 31
  %173 = load i16, ptr %4, align 2, !tbaa !7
  %174 = sext i16 %173 to i32
  %175 = add i32 %172, %174
  %176 = load i32, ptr @sum, align 2, !tbaa !9
  %177 = add i32 %176, %175
  store i32 %177, ptr @sum, align 2, !tbaa !9
  call void @llvm.lifetime.end.p0(ptr %9) #2
  br label %178

178:                                              ; preds = %169
  %179 = load i32, ptr %6, align 2, !tbaa !9
  %180 = add nsw i32 %179, 1
  store i32 %180, ptr %6, align 2, !tbaa !9
  br label %88, !llvm.loop !13

181:                                              ; preds = %88
  %182 = load i32, ptr %5, align 2, !tbaa !9
  call void @llvm.lifetime.end.p0(ptr %6) #2
  call void @llvm.lifetime.end.p0(ptr %5) #2
  call void @llvm.lifetime.end.p0(ptr %4) #2
  call void @llvm.lifetime.end.p0(ptr %3) #2
  call void @llvm.lifetime.end.p0(ptr %2) #2
  call void @llvm.lifetime.end.p0(ptr %1) #2
  ret i32 %182
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #0

; Function Attrs: noinline nounwind optsize
define hidden i32 @next() #1 {
  %1 = load i32, ptr @state, align 2, !tbaa !9
  %2 = shl i32 %1, 13
  %3 = load i32, ptr @state, align 2, !tbaa !9
  %4 = xor i32 %3, %2
  store i32 %4, ptr @state, align 2, !tbaa !9
  %5 = load i32, ptr @state, align 2, !tbaa !9
  %6 = lshr i32 %5, 17
  %7 = load i32, ptr @state, align 2, !tbaa !9
  %8 = xor i32 %7, %6
  store i32 %8, ptr @state, align 2, !tbaa !9
  %9 = load i32, ptr @state, align 2, !tbaa !9
  %10 = shl i32 %9, 5
  %11 = load i32, ptr @state, align 2, !tbaa !9
  %12 = xor i32 %11, %10
  store i32 %12, ptr @state, align 2, !tbaa !9
  %13 = load i32, ptr @state, align 2, !tbaa !9
  ret i32 %13
}

attributes #0 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #1 = { noinline nounwind optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #2 = { nounwind }
attributes #3 = { optsize }

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
!11 = distinct !{!11, !12}
!12 = !{!"llvm.loop.mustprogress"}
!13 = distinct !{!13, !12}
