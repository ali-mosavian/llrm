; known_loads_walks reduced (clang -O0, optnone removed): an inner loop whose select is invariant after one trip,
; peeled, inside a loop the pin leaves alone. The pin: 175 B; unrolled by two with an epilogue: 292 B.
target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32"

; Function Attrs: noinline nounwind
define hidden zeroext i8 @f(i8 noundef zeroext %0, i8 noundef zeroext %1, i8 noundef zeroext %2) #0 {
  %4 = alloca i8, align 1
  %5 = alloca i8, align 1
  %6 = alloca i8, align 1
  %7 = alloca i8, align 1
  %8 = alloca i8, align 1
  %9 = alloca i8, align 1
  %10 = alloca i8, align 1
  %11 = alloca i8, align 1
  %12 = alloca i8, align 1
  %13 = alloca i8, align 1
  %14 = alloca i8, align 1
  %15 = alloca i8, align 1
  %16 = alloca i8, align 1
  %17 = alloca i8, align 1
  %18 = alloca i8, align 1
  %19 = alloca i8, align 1
  store i8 %0, ptr %4, align 1
  store i8 %1, ptr %5, align 1
  store i8 %2, ptr %6, align 1
  store i8 0, ptr %7, align 1
  store i8 0, ptr %8, align 1
  store i8 0, ptr %9, align 1
  %20 = load i8, ptr %5, align 1
  %21 = zext i8 %20 to i32
  %22 = icmp slt i32 %21, 100
  br i1 %22, label %23, label %189

23:                                               ; preds = %3
  store i8 0, ptr %10, align 1
  br label %24

24:                                               ; preds = %55, %23
  %25 = load i8, ptr %10, align 1
  %26 = zext i8 %25 to i32
  %27 = load i8, ptr %6, align 1
  %28 = zext i8 %27 to i32
  %29 = add nsw i32 %28, 1
  %30 = trunc i32 %29 to i8
  %31 = zext i8 %30 to i32
  %32 = icmp slt i32 %26, %31
  br i1 %32, label %33, label %58

33:                                               ; preds = %24
  %34 = load i8, ptr %4, align 1
  %35 = zext i8 %34 to i32
  %36 = load i8, ptr %7, align 1
  %37 = zext i8 %36 to i32
  %38 = icmp ne i32 %35, %37
  br i1 %38, label %39, label %54

39:                                               ; preds = %33
  store i8 0, ptr %11, align 1
  br label %40

40:                                               ; preds = %50, %39
  %41 = load i8, ptr %11, align 1
  %42 = zext i8 %41 to i32
  %43 = load i8, ptr %6, align 1
  %44 = zext i8 %43 to i32
  %45 = add nsw i32 %44, 0
  %46 = trunc i32 %45 to i8
  %47 = zext i8 %46 to i32
  %48 = icmp slt i32 %42, %47
  br i1 %48, label %49, label %53

49:                                               ; preds = %40
  br label %50

50:                                               ; preds = %49
  %51 = load i8, ptr %11, align 1
  %52 = add i8 %51, 1
  store i8 %52, ptr %11, align 1
  br label %40, !llvm.loop !1

53:                                               ; preds = %40
  br label %54

54:                                               ; preds = %53, %33
  br label %55

55:                                               ; preds = %54
  %56 = load i8, ptr %10, align 1
  %57 = add i8 %56, 1
  store i8 %57, ptr %10, align 1
  br label %24, !llvm.loop !3

58:                                               ; preds = %24
  store i8 0, ptr %12, align 1
  br label %59

59:                                               ; preds = %109, %58
  %60 = load i8, ptr %12, align 1
  %61 = zext i8 %60 to i32
  %62 = load i8, ptr %6, align 1
  %63 = zext i8 %62 to i32
  %64 = add nsw i32 %63, 0
  %65 = trunc i32 %64 to i8
  %66 = zext i8 %65 to i32
  %67 = icmp slt i32 %61, %66
  br i1 %67, label %68, label %112

68:                                               ; preds = %59
  store i8 0, ptr %13, align 1
  br label %69

69:                                               ; preds = %93, %68
  %70 = load i8, ptr %13, align 1
  %71 = zext i8 %70 to i32
  %72 = load i8, ptr %6, align 1
  %73 = zext i8 %72 to i32
  %74 = add nsw i32 %73, 0
  %75 = trunc i32 %74 to i8
  %76 = zext i8 %75 to i32
  %77 = icmp slt i32 %71, %76
  br i1 %77, label %78, label %96

78:                                               ; preds = %69
  store i8 0, ptr %14, align 1
  br label %79

79:                                               ; preds = %89, %78
  %80 = load i8, ptr %14, align 1
  %81 = zext i8 %80 to i32
  %82 = load i8, ptr %6, align 1
  %83 = zext i8 %82 to i32
  %84 = add nsw i32 %83, 1
  %85 = trunc i32 %84 to i8
  %86 = zext i8 %85 to i32
  %87 = icmp slt i32 %81, %86
  br i1 %87, label %88, label %92

88:                                               ; preds = %79
  br label %89

89:                                               ; preds = %88
  %90 = load i8, ptr %14, align 1
  %91 = add i8 %90, 1
  store i8 %91, ptr %14, align 1
  br label %79, !llvm.loop !4

92:                                               ; preds = %79
  br label %93

93:                                               ; preds = %92
  %94 = load i8, ptr %13, align 1
  %95 = add i8 %94, 1
  store i8 %95, ptr %13, align 1
  br label %69, !llvm.loop !5

96:                                               ; preds = %69
  %97 = load i8, ptr %8, align 1
  %98 = zext i8 %97 to i32
  %99 = icmp ne i32 %98, 3
  br i1 %99, label %100, label %108

100:                                              ; preds = %96
  %101 = load i8, ptr %9, align 1
  %102 = zext i8 %101 to i32
  %103 = load i8, ptr %8, align 1
  %104 = zext i8 %103 to i32
  %105 = icmp ne i32 %102, %104
  br i1 %105, label %106, label %107

106:                                              ; preds = %100
  br label %107

107:                                              ; preds = %106, %100
  br label %108

108:                                              ; preds = %107, %96
  br label %109

109:                                              ; preds = %108
  %110 = load i8, ptr %12, align 1
  %111 = add i8 %110, 1
  store i8 %111, ptr %12, align 1
  br label %59, !llvm.loop !6

112:                                              ; preds = %59
  store i8 0, ptr %15, align 1
  br label %113

113:                                              ; preds = %185, %112
  %114 = load i8, ptr %15, align 1
  %115 = zext i8 %114 to i32
  %116 = load i8, ptr %6, align 1
  %117 = zext i8 %116 to i32
  %118 = add nsw i32 %117, 1
  %119 = trunc i32 %118 to i8
  %120 = zext i8 %119 to i32
  %121 = icmp slt i32 %115, %120
  br i1 %121, label %122, label %188

122:                                              ; preds = %113
  store i8 0, ptr %16, align 1
  br label %123

123:                                              ; preds = %138, %122
  %124 = load i8, ptr %16, align 1
  %125 = zext i8 %124 to i32
  %126 = load i8, ptr %6, align 1
  %127 = zext i8 %126 to i32
  %128 = add nsw i32 %127, 0
  %129 = trunc i32 %128 to i8
  %130 = zext i8 %129 to i32
  %131 = icmp slt i32 %125, %130
  br i1 %131, label %132, label %141

132:                                              ; preds = %123
  %133 = load i8, ptr %8, align 1
  %134 = zext i8 %133 to i32
  %135 = icmp slt i32 %134, 100
  br i1 %135, label %136, label %137

136:                                              ; preds = %132
  br label %137

137:                                              ; preds = %136, %132
  br label %138

138:                                              ; preds = %137
  %139 = load i8, ptr %16, align 1
  %140 = add i8 %139, 1
  store i8 %140, ptr %16, align 1
  br label %123, !llvm.loop !7

141:                                              ; preds = %123
  store i8 0, ptr %17, align 1
  br label %142

142:                                              ; preds = %181, %141
  %143 = load i8, ptr %17, align 1
  %144 = zext i8 %143 to i32
  %145 = load i8, ptr %6, align 1
  %146 = zext i8 %145 to i32
  %147 = add nsw i32 %146, 1
  %148 = trunc i32 %147 to i8
  %149 = zext i8 %148 to i32
  %150 = icmp slt i32 %144, %149
  br i1 %150, label %151, label %184

151:                                              ; preds = %142
  store i8 0, ptr %18, align 1
  br label %152

152:                                              ; preds = %163, %151
  %153 = load i8, ptr %18, align 1
  %154 = zext i8 %153 to i32
  %155 = load i8, ptr %6, align 1
  %156 = zext i8 %155 to i32
  %157 = add nsw i32 %156, 1
  %158 = trunc i32 %157 to i8
  %159 = zext i8 %158 to i32
  %160 = icmp slt i32 %154, %159
  br i1 %160, label %161, label %166

161:                                              ; preds = %152
  %162 = load i8, ptr %8, align 1
  store i8 %162, ptr %9, align 1
  br label %163

163:                                              ; preds = %161
  %164 = load i8, ptr %18, align 1
  %165 = add i8 %164, 1
  store i8 %165, ptr %18, align 1
  br label %152, !llvm.loop !8

166:                                              ; preds = %152
  store i8 -1, ptr %8, align 1
  store i8 0, ptr %19, align 1
  br label %167

167:                                              ; preds = %177, %166
  %168 = load i8, ptr %19, align 1
  %169 = zext i8 %168 to i32
  %170 = load i8, ptr %6, align 1
  %171 = zext i8 %170 to i32
  %172 = add nsw i32 %171, 0
  %173 = trunc i32 %172 to i8
  %174 = zext i8 %173 to i32
  %175 = icmp slt i32 %169, %174
  br i1 %175, label %176, label %180

176:                                              ; preds = %167
  store i8 -1, ptr %5, align 1
  br label %177

177:                                              ; preds = %176
  %178 = load i8, ptr %19, align 1
  %179 = add i8 %178, 1
  store i8 %179, ptr %19, align 1
  br label %167, !llvm.loop !9

180:                                              ; preds = %167
  br label %181

181:                                              ; preds = %180
  %182 = load i8, ptr %17, align 1
  %183 = add i8 %182, 1
  store i8 %183, ptr %17, align 1
  br label %142, !llvm.loop !10

184:                                              ; preds = %142
  br label %185

185:                                              ; preds = %184
  %186 = load i8, ptr %15, align 1
  %187 = add i8 %186, 1
  store i8 %187, ptr %15, align 1
  br label %113, !llvm.loop !11

188:                                              ; preds = %113
  br label %189

189:                                              ; preds = %188, %3
  %190 = load i8, ptr %4, align 1
  %191 = zext i8 %190 to i32
  %192 = load i8, ptr %5, align 1
  %193 = zext i8 %192 to i32
  %194 = mul nsw i32 3, %193
  %195 = add nsw i32 %191, %194
  %196 = load i8, ptr %7, align 1
  %197 = zext i8 %196 to i32
  %198 = mul nsw i32 5, %197
  %199 = add nsw i32 %195, %198
  %200 = load i8, ptr %8, align 1
  %201 = zext i8 %200 to i32
  %202 = mul nsw i32 7, %201
  %203 = add nsw i32 %199, %202
  %204 = load i8, ptr %9, align 1
  %205 = zext i8 %204 to i32
  %206 = mul nsw i32 11, %205
  %207 = add nsw i32 %203, %206
  %208 = trunc i32 %207 to i8
  ret i8 %208
}

attributes #0 = { noinline nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+bulk-memory,+bulk-memory-opt,+call-indirect-overlong,+multivalue,+mutable-globals,+nontrapping-fptoint,+reference-types,+sign-ext" }

!llvm.ident = !{!0}

!0 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!1 = distinct !{!1, !2}
!2 = !{!"llvm.loop.mustprogress"}
!3 = distinct !{!3, !2}
!4 = distinct !{!4, !2}
!5 = distinct !{!5, !2}
!6 = distinct !{!6, !2}
!7 = distinct !{!7, !2}
!8 = distinct !{!8, !2}
!9 = distinct !{!9, !2}
!10 = distinct !{!10, !2}
!11 = distinct !{!11, !2}
