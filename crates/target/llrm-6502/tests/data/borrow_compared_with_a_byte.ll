target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8"

@facexp = global i8 0, align 1
@facho = global i8 0, align 1
@facmoh = global i8 0, align 1
@facmo = global i8 0, align 1
@faclo = global i8 0, align 1
@facsgn = global i8 0, align 1
@tempf2 = global [5 x i8] zeroinitializer, align 1
@tp = internal unnamed_addr global i16 0, align 2
@degree = global i8 0, align 1
@tend = internal unnamed_addr global i16 -1, align 2

declare void @fp_round() nounwind "no-trapping-math"="true" "stack-protector-buffer-size"="8" 















declare void @fp_bfaddt() nounwind "no-trapping-math"="true" "stack-protector-buffer-size"="8" 









declare void @fp_mul10() nounwind "no-trapping-math"="true" "stack-protector-buffer-size"="8" 

declare void @fp_bfdivt() nounwind "no-trapping-math"="true" "stack-protector-buffer-size"="8" 








































define i16 @fp_fin(i16 noundef %0) nounwind "no-trapping-math"="true" "stack-protector-buffer-size"="8" {
  %2 = load i16, ptr @tend, align 2
  %3 = icmp eq i16 %0, %2
  br label %12

4:
  %5 = add i16 %8, 1
  %6 = icmp eq i16 %5, %2
  br label %20

7:
  %8 = phi i16 [ %17, %19 ]
  %9 = inttoptr i16 %8 to ptr
  %10 = load i8, ptr %9, align 1
  %11 = icmp eq i8 %10, 32
  br label %4

12:
  %13 = inttoptr i16 %0 to ptr
  %14 = load i8, ptr %13, align 1
  %15 = icmp eq i8 %14, 32
  br label %16

16:
  %17 = add i16 %0, 1
  %18 = icmp eq i16 %17, %2
  br label %19

19:
  br label %7

20:
  %promoted13 = phi i16 [ %2, %4 ]
  %21 = phi i8 [ 0, %4 ]
  %22 = add i8 %21, -58
  %23 = icmp ult i8 %22, -10
  tail call void @llvm.memset.p0.i16(ptr align 1 getelementptr inbounds nuw (i8, ptr @tempf2, i16 1), i8 0, i16 4, i1 false)
  br label %24

24:
  %25 = icmp eq i8 %21, 43
  br label %26

26:
  %27 = icmp eq i8 %21, 45
  br label %28

28:
  br label %37

29:
  %30 = add i16 %33, 1
  %31 = icmp eq i16 %30, %38
  br label %129

32:
  %33 = phi i16 [ %46, %48 ]
  %34 = inttoptr i16 %33 to ptr
  %35 = load i8, ptr %34, align 1
  %36 = icmp eq i8 %35, 32
  br label %29

37:
  %promoted21 = phi i16 [ %promoted13, %28 ], [ %promoted2.pre, %190 ], [ %promoted11, %124 ]
  %38 = phi i16 [ %2, %28 ], [ %191, %190 ], [ %54, %124 ]
  %39 = add i16 %promoted21, 1
  %40 = icmp eq i16 %39, %38
  br i1 %40, label %129, label %41

41:
  %42 = inttoptr i16 %39 to ptr
  %43 = load i8, ptr %42, align 1
  %44 = icmp eq i8 %43, 32
  br i1 %44, label %45, label %49

45:
  %46 = add i16 %promoted21, 2
  %47 = icmp eq i16 %46, %38
  br label %48

48:
  br label %32

49:
  %promoted12 = phi i16 [ %39, %41 ]
  %50 = phi i8 [ %43, %41 ]
  %51 = add i8 %50, -58
  %52 = icmp ult i8 %51, -10
  br i1 %52, label %53, label %163

53:
  %54 = phi i16 [ %38, %49 ]
  %promoted11 = phi i16 [ %promoted12, %49 ]
  %55 = phi i8 [ %50, %49 ]
  %56 = icmp eq i8 %55, 46
  br i1 %56, label %124, label %57

57:
  %58 = icmp eq i8 %55, 69
  br label %67

59:
  %60 = add i16 %63, 1
  %61 = icmp eq i16 %60, %54
  br label %118

62:
  %63 = phi i16 [ %75, %77 ]
  %64 = inttoptr i16 %63 to ptr
  %65 = load i8, ptr %64, align 1
  %66 = icmp eq i8 %65, 32
  br label %59

67:
  %68 = add i16 %promoted11, 1
  %69 = icmp eq i16 %68, %54
  br label %70

70:
  %71 = inttoptr i16 %68 to ptr
  %72 = load i8, ptr %71, align 1
  %73 = icmp eq i8 %72, 32
  br i1 %73, label %74, label %78

74:
  %75 = add i16 %promoted11, 2
  %76 = icmp eq i16 %75, %54
  br label %77

77:
  br label %62

78:
  %promoted4 = phi i16 [ %68, %70 ]
  %79 = phi i8 [ %72, %70 ]
  %80 = add i8 %79, -58
  %81 = icmp ult i8 %80, -10
  br label %82

82:
  %83 = icmp slt i8 %79, 43
  br i1 %83, label %84, label %88

84:
  %85 = icmp eq i8 %79, -86
  br label %86

86:
  %87 = icmp eq i8 %79, -85
  br label %92

88:
  %89 = icmp eq i8 %79, 43
  br label %90

90:
  %91 = icmp eq i8 %79, 45
  br label %92

92:
  br label %101

93:
  %94 = add i16 %97, 1
  %95 = icmp eq i16 %94, %102
  br label %118

96:
  %97 = phi i16 [ %110, %112 ]
  %98 = inttoptr i16 %97 to ptr
  %99 = load i8, ptr %98, align 1
  %100 = icmp eq i8 %99, 32
  br label %93

101:
  %102 = phi i16 [ %54, %92 ]
  %promoted5 = phi i16 [ %promoted4, %92 ]
  %103 = add i16 %promoted5, 1
  %104 = icmp eq i16 %103, %102
  br label %105

105:
  %106 = inttoptr i16 %103 to ptr
  %107 = load i8, ptr %106, align 1
  %108 = icmp eq i8 %107, 32
  br i1 %108, label %109, label %113

109:
  %110 = add i16 %promoted5, 2
  %111 = icmp eq i16 %110, %102
  br label %112

112:
  br label %96

113:
  %114 = phi i16 [ %103, %105 ]
  %115 = phi i8 [ %107, %105 ]
  %116 = add i8 %115, -58
  %117 = icmp ult i8 %116, -10
  br label %118

118:
  %119 = load i8, ptr getelementptr inbounds nuw (i8, ptr @tempf2, i16 4), align 1
  %120 = icmp sgt i8 %119, -1
  br label %121

121:
  %122 = load i8, ptr getelementptr inbounds nuw (i8, ptr @tempf2, i16 2), align 1
  %123 = sub i8 0, %122
  br label %131

124:
  %125 = load i8, ptr getelementptr inbounds nuw (i8, ptr @tempf2, i16 3), align 1
  %126 = lshr i8 %125, 1
  %127 = or disjoint i8 %126, -128
  %128 = icmp sgt i8 %125, -1
  br label %37

129:
  %130 = load i8, ptr getelementptr inbounds nuw (i8, ptr @tempf2, i16 2), align 1
  br label %131

131:
  %storemerge = phi i8 [ %123, %121 ], [ %130, %129 ]
  %132 = load i8, ptr getelementptr inbounds nuw (i8, ptr @tempf2, i16 1), align 1
  %133 = sub i8 %storemerge, %132
  %134 = icmp eq i8 %storemerge, %132
  br label %135

135:
  %136 = icmp sgt i8 %133, -1
  br i1 %136, label %138, label %137

137:
  br label %143

138:
  br label %139

139:
  tail call void @fp_mul10()
  %140 = load i8, ptr getelementptr inbounds nuw (i8, ptr @tempf2, i16 2), align 1
  %141 = add i8 %140, -1
  %142 = icmp eq i8 %141, 0
  br label %153

143:
  tail call void @fp_round()
  %144 = load i8, ptr @facexp, align 1
  %145 = load i8, ptr @facho, align 1
  %146 = load i8, ptr @facmoh, align 1
  %147 = load i8, ptr @facmo, align 1
  %148 = load i8, ptr @faclo, align 1
  %149 = load i8, ptr @facsgn, align 1
  tail call void @fp_bfdivt()
  %150 = load i8, ptr getelementptr inbounds nuw (i8, ptr @tempf2, i16 2), align 1
  %151 = add i8 %150, 1
  %152 = icmp eq i8 %151, 0
  br label %153

153:
  %154 = load i8, ptr @degree, align 1
  %155 = icmp sgt i8 %154, -1
  %156 = load i8, ptr @facexp, align 1
  %157 = icmp eq i8 %156, 0
  br label %158

158:
  %159 = load i8, ptr @facsgn, align 1
  %160 = xor i8 %159, -1
  br label %161

161:
  %162 = load i16, ptr @tp, align 2
  ret i16 %162

163:
  %164 = load i8, ptr getelementptr inbounds nuw (i8, ptr @tempf2, i16 3), align 1
  %165 = icmp sgt i8 %164, -1
  br label %166

166:
  %167 = load i8, ptr getelementptr inbounds nuw (i8, ptr @tempf2, i16 1), align 1
  %168 = add i8 %167, 1
  br label %169

169:
  %170 = phi i8 [ %50, %166 ]
  tail call void @fp_mul10()
  %171 = add nsw i8 %170, -48
  tail call void @fp_round()
  %172 = load i8, ptr @facexp, align 1
  %173 = load i8, ptr @facho, align 1
  %174 = load i8, ptr @facmoh, align 1
  %175 = load i8, ptr @facmo, align 1
  %176 = load i8, ptr @faclo, align 1
  %177 = load i8, ptr @facsgn, align 1
  %178 = icmp eq i8 %171, 0
  br i1 %178, label %180, label %179

179:
  br label %181

180:
  ret i16 0
181:
  %182 = phi i8 [ %184, %181 ], [ 0, %179 ]
  %183 = phi i8 [ %185, %181 ], [ %171, %179 ]
  %184 = add i8 %182, 1
  %185 = tail call i8 @llvm.fshl.i8(i8 %183, i8 0, i8 1)
  %186 = icmp sgt i8 %185, -1
  br i1 %186, label %181, label %187

187:
  %188 = call i8 @llvm.umax.i8(i8 %184, i8 -120)
  %189 = sub i8 %188, %184
  br label %190

190:
  %sink = phi i8 [ %189, %187 ]
  store i8 %sink, ptr @facexp, align 1
  tail call void @fp_bfaddt()
  %191 = load i16, ptr @tend, align 2
  %promoted2.pre = load i16, ptr @tp, align 2
  br label %37

}


declare i8 @llvm.fshl.i8(i8, i8, i8) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: write)


declare i8 @llvm.umax.i8(i8, i8) nocallback nofree nosync nounwind speculatable willreturn memory(none)


