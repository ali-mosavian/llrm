target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"

define zeroext i8 @f(i8 noundef zeroext %0, i8 noundef zeroext %1, i8 noundef zeroext %2) noinline nounwind optsize "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" {
  %4 = alloca i8, align 1
  %5 = alloca i8, align 1
  %6 = alloca i8, align 1
  %7 = alloca i8, align 1
  %8 = alloca i8, align 1
  %9 = alloca i8, align 1
  %10 = alloca i8, align 1
  %11 = alloca i32, align 4
  %12 = alloca i8, align 1
  %13 = alloca i8, align 1
  %14 = alloca i8, align 1
  %15 = alloca i8, align 1
  %16 = alloca i8, align 1
  %17 = alloca i8, align 1
  %18 = alloca i8, align 1
  %19 = alloca i8, align 1
  %20 = alloca i8, align 1
  store i8 %0, ptr %4, align 1, !tbaa !8
  store i8 %1, ptr %5, align 1, !tbaa !8
  store i8 %2, ptr %6, align 1, !tbaa !8
  call void @llvm.lifetime.start.p0(i64 -1, ptr %7) nounwind
  store i8 0, ptr %7, align 1, !tbaa !8
  call void @llvm.lifetime.start.p0(i64 -1, ptr %8) nounwind
  store i8 0, ptr %8, align 1, !tbaa !8
  call void @llvm.lifetime.start.p0(i64 -1, ptr %9) nounwind
  store i8 0, ptr %9, align 1, !tbaa !8
  call void @llvm.lifetime.start.p0(i64 -1, ptr %10) nounwind
  store i8 0, ptr %10, align 1, !tbaa !8
  br label %21

21:
  %22 = load i8, ptr %10, align 1, !tbaa !8
  %23 = zext i8 %22 to i32
  %24 = load i8, ptr %6, align 1, !tbaa !8
  %25 = zext i8 %24 to i32
  %26 = add nsw i32 %25, 1
  %27 = trunc i32 %26 to i8
  %28 = zext i8 %27 to i32
  %29 = icmp slt i32 %23, %28
  br i1 %29, label %31, label %30

30:
  store i32 2, ptr %11, align 4
  call void @llvm.lifetime.end.p0(i64 -1, ptr %10) nounwind
  br label %255

31:
  %32 = load i8, ptr %4, align 1, !tbaa !8
  %33 = zext i8 %32 to i32
  %34 = load i8, ptr %8, align 1, !tbaa !8
  %35 = zext i8 %34 to i32
  %36 = icmp eq i32 %33, %35
  br i1 %36, label %37, label %96

37:
  call void @llvm.lifetime.start.p0(i64 -1, ptr %12) nounwind
  store i8 0, ptr %12, align 1, !tbaa !8
  br label %38

38:
  %39 = load i8, ptr %12, align 1, !tbaa !8
  %40 = zext i8 %39 to i32
  %41 = load i8, ptr %6, align 1, !tbaa !8
  %42 = zext i8 %41 to i32
  %43 = add nsw i32 %42, 1
  %44 = trunc i32 %43 to i8
  %45 = zext i8 %44 to i32
  %46 = icmp slt i32 %40, %45
  br i1 %46, label %48, label %47

47:
  store i32 5, ptr %11, align 4
  call void @llvm.lifetime.end.p0(i64 -1, ptr %12) nounwind
  br label %89

48:
  call void @llvm.lifetime.start.p0(i64 -1, ptr %13) nounwind
  store i8 0, ptr %13, align 1, !tbaa !8
  br label %49

49:
  %50 = load i8, ptr %13, align 1, !tbaa !8
  %51 = zext i8 %50 to i32
  %52 = load i8, ptr %6, align 1, !tbaa !8
  %53 = zext i8 %52 to i32
  %54 = add nsw i32 %53, 0
  %55 = trunc i32 %54 to i8
  %56 = zext i8 %55 to i32
  %57 = icmp slt i32 %51, %56
  br i1 %57, label %59, label %58

58:
  store i32 8, ptr %11, align 4
  call void @llvm.lifetime.end.p0(i64 -1, ptr %13) nounwind
  br label %64

59:
  %60 = load i8, ptr %4, align 1, !tbaa !8
  store i8 %60, ptr %4, align 1, !tbaa !8
  store i8 1, ptr %8, align 1, !tbaa !8
  br label %61

61:
  %62 = load i8, ptr %13, align 1, !tbaa !8
  %63 = add i8 %62, 1
  store i8 %63, ptr %13, align 1, !tbaa !8
  br label %49, !llvm.loop !9

64:
  call void @llvm.lifetime.start.p0(i64 -1, ptr %14) nounwind
  store i8 0, ptr %14, align 1, !tbaa !8
  br label %65

65:
  %66 = load i8, ptr %14, align 1, !tbaa !8
  %67 = zext i8 %66 to i32
  %68 = load i8, ptr %6, align 1, !tbaa !8
  %69 = zext i8 %68 to i32
  %70 = add nsw i32 %69, 1
  %71 = trunc i32 %70 to i8
  %72 = zext i8 %71 to i32
  %73 = icmp slt i32 %67, %72
  br i1 %73, label %75, label %74

74:
  store i32 11, ptr %11, align 4
  call void @llvm.lifetime.end.p0(i64 -1, ptr %14) nounwind
  br label %85

75:
  %76 = load i8, ptr %7, align 1, !tbaa !8
  %77 = zext i8 %76 to i32
  %78 = load i8, ptr %5, align 1, !tbaa !8
  %79 = zext i8 %78 to i32
  %80 = or i32 %77, %79
  %81 = trunc i32 %80 to i8
  store i8 %81, ptr %4, align 1, !tbaa !8
  store i8 5, ptr %7, align 1, !tbaa !8
  br label %82

82:
  %83 = load i8, ptr %14, align 1, !tbaa !8
  %84 = add i8 %83, 1
  store i8 %84, ptr %14, align 1, !tbaa !8
  br label %65, !llvm.loop !11

85:
  br label %86

86:
  %87 = load i8, ptr %12, align 1, !tbaa !8
  %88 = add i8 %87, 1
  store i8 %88, ptr %12, align 1, !tbaa !8
  br label %38, !llvm.loop !12

89:
  %90 = load i8, ptr %9, align 1, !tbaa !8
  %91 = zext i8 %90 to i32
  %92 = load i8, ptr %9, align 1, !tbaa !8
  %93 = zext i8 %92 to i32
  %94 = add nsw i32 %91, %93
  %95 = trunc i32 %94 to i8
  store i8 %95, ptr %7, align 1, !tbaa !8
  br label %145

96:
  call void @llvm.lifetime.start.p0(i64 -1, ptr %15) nounwind
  store i8 0, ptr %15, align 1, !tbaa !8
  br label %97

97:
  %98 = load i8, ptr %15, align 1, !tbaa !8
  %99 = zext i8 %98 to i32
  %100 = load i8, ptr %6, align 1, !tbaa !8
  %101 = zext i8 %100 to i32
  %102 = add nsw i32 %101, 0
  %103 = trunc i32 %102 to i8
  %104 = zext i8 %103 to i32
  %105 = icmp slt i32 %99, %104
  br i1 %105, label %107, label %106

106:
  store i32 14, ptr %11, align 4
  call void @llvm.lifetime.end.p0(i64 -1, ptr %15) nounwind
  br label %144

107:
  %108 = load i8, ptr %4, align 1, !tbaa !8
  %109 = zext i8 %108 to i32
  %110 = icmp slt i32 %109, 3
  br i1 %110, label %111, label %113

111:
  %112 = load i8, ptr %5, align 1, !tbaa !8
  store i8 %112, ptr %9, align 1, !tbaa !8
  br label %121

113:
  %114 = load i8, ptr %9, align 1, !tbaa !8
  store i8 %114, ptr %4, align 1, !tbaa !8
  %115 = load i8, ptr %5, align 1, !tbaa !8
  %116 = zext i8 %115 to i32
  %117 = load i8, ptr %5, align 1, !tbaa !8
  %118 = zext i8 %117 to i32
  %119 = or i32 %116, %118
  %120 = trunc i32 %119 to i8
  store i8 %120, ptr %7, align 1, !tbaa !8
  store i8 0, ptr %4, align 1, !tbaa !8
  br label %121

121:
  %122 = load i8, ptr %7, align 1, !tbaa !8
  %123 = zext i8 %122 to i32
  %124 = load i8, ptr %9, align 1, !tbaa !8
  %125 = zext i8 %124 to i32
  %126 = icmp sgt i32 %123, %125
  br i1 %126, label %127, label %129

127:
  %128 = load i8, ptr %5, align 1, !tbaa !8
  store i8 %128, ptr %4, align 1, !tbaa !8
  br label %139

129:
  %130 = load i8, ptr %9, align 1, !tbaa !8
  store i8 %130, ptr %8, align 1, !tbaa !8
  %131 = load i8, ptr %4, align 1, !tbaa !8
  %132 = zext i8 %131 to i32
  %133 = or i32 %132, 7
  %134 = trunc i32 %133 to i8
  store i8 %134, ptr %9, align 1, !tbaa !8
  %135 = load i8, ptr %5, align 1, !tbaa !8
  %136 = zext i8 %135 to i32
  %137 = xor i32 %136, 3
  %138 = trunc i32 %137 to i8
  store i8 %138, ptr %9, align 1, !tbaa !8
  br label %139

139:
  %140 = load i8, ptr %8, align 1, !tbaa !8
  store i8 %140, ptr %7, align 1, !tbaa !8
  store i8 0, ptr %8, align 1, !tbaa !8
  br label %141

141:
  %142 = load i8, ptr %15, align 1, !tbaa !8
  %143 = add i8 %142, 1
  store i8 %143, ptr %15, align 1, !tbaa !8
  br label %97, !llvm.loop !13

144:
  br label %145

145:
  call void @llvm.lifetime.start.p0(i64 -1, ptr %16) nounwind
  store i8 0, ptr %16, align 1, !tbaa !8
  br label %146

146:
  %147 = load i8, ptr %16, align 1, !tbaa !8
  %148 = zext i8 %147 to i32
  %149 = load i8, ptr %6, align 1, !tbaa !8
  %150 = zext i8 %149 to i32
  %151 = add nsw i32 %150, 1
  %152 = trunc i32 %151 to i8
  %153 = zext i8 %152 to i32
  %154 = icmp slt i32 %148, %153
  br i1 %154, label %156, label %155

155:
  store i32 17, ptr %11, align 4
  call void @llvm.lifetime.end.p0(i64 -1, ptr %16) nounwind
  br label %203

156:
  %157 = load i8, ptr %7, align 1, !tbaa !8
  store i8 %157, ptr %7, align 1, !tbaa !8
  %158 = load i8, ptr %7, align 1, !tbaa !8
  %159 = zext i8 %158 to i32
  %160 = icmp ne i32 %159, 100
  br i1 %160, label %161, label %183

161:
  %162 = load i8, ptr %9, align 1, !tbaa !8
  %163 = zext i8 %162 to i32
  %164 = and i32 %163, 3
  %165 = trunc i32 %164 to i8
  store i8 %165, ptr %9, align 1, !tbaa !8
  call void @llvm.lifetime.start.p0(i64 -1, ptr %17) nounwind
  store i8 0, ptr %17, align 1, !tbaa !8
  br label %166

166:
  %167 = load i8, ptr %17, align 1, !tbaa !8
  %168 = zext i8 %167 to i32
  %169 = load i8, ptr %6, align 1, !tbaa !8
  %170 = zext i8 %169 to i32
  %171 = add nsw i32 %170, 1
  %172 = trunc i32 %171 to i8
  %173 = zext i8 %172 to i32
  %174 = icmp slt i32 %168, %173
  br i1 %174, label %176, label %175

175:
  store i32 20, ptr %11, align 4
  call void @llvm.lifetime.end.p0(i64 -1, ptr %17) nounwind
  br label %182

176:
  store i8 0, ptr %9, align 1, !tbaa !8
  %177 = load i8, ptr %4, align 1, !tbaa !8
  store i8 %177, ptr %7, align 1, !tbaa !8
  store i8 -1, ptr %9, align 1, !tbaa !8
  %178 = load i8, ptr %4, align 1, !tbaa !8
  store i8 %178, ptr %4, align 1, !tbaa !8
  br label %179

179:
  %180 = load i8, ptr %17, align 1, !tbaa !8
  %181 = add i8 %180, 1
  store i8 %181, ptr %17, align 1, !tbaa !8
  br label %166, !llvm.loop !14

182:
  br label %199

183:
  %184 = load i8, ptr %7, align 1, !tbaa !8
  %185 = zext i8 %184 to i32
  %186 = load i8, ptr %9, align 1, !tbaa !8
  %187 = zext i8 %186 to i32
  %188 = icmp slt i32 %185, %187
  br i1 %188, label %189, label %191

189:
  store i8 0, ptr %5, align 1, !tbaa !8
  %190 = load i8, ptr %4, align 1, !tbaa !8
  store i8 %190, ptr %9, align 1, !tbaa !8
  br label %198

191:
  store i8 2, ptr %4, align 1, !tbaa !8
  store i8 0, ptr %5, align 1, !tbaa !8
  %192 = load i8, ptr %9, align 1, !tbaa !8
  %193 = zext i8 %192 to i32
  %194 = load i8, ptr %8, align 1, !tbaa !8
  %195 = zext i8 %194 to i32
  %196 = or i32 %193, %195
  %197 = trunc i32 %196 to i8
  store i8 %197, ptr %7, align 1, !tbaa !8
  br label %198

198:
  store i8 5, ptr %4, align 1, !tbaa !8
  br label %199

199:
  store i8 0, ptr %9, align 1, !tbaa !8
  br label %200

200:
  %201 = load i8, ptr %16, align 1, !tbaa !8
  %202 = add i8 %201, 1
  store i8 %202, ptr %16, align 1, !tbaa !8
  br label %146, !llvm.loop !15

203:
  call void @llvm.lifetime.start.p0(i64 -1, ptr %18) nounwind
  store i8 0, ptr %18, align 1, !tbaa !8
  br label %204

204:
  %205 = load i8, ptr %18, align 1, !tbaa !8
  %206 = zext i8 %205 to i32
  %207 = load i8, ptr %6, align 1, !tbaa !8
  %208 = zext i8 %207 to i32
  %209 = add nsw i32 %208, 1
  %210 = trunc i32 %209 to i8
  %211 = zext i8 %210 to i32
  %212 = icmp slt i32 %206, %211
  br i1 %212, label %214, label %213

213:
  store i32 23, ptr %11, align 4
  call void @llvm.lifetime.end.p0(i64 -1, ptr %18) nounwind
  br label %251

214:
  store i8 -1, ptr %7, align 1, !tbaa !8
  call void @llvm.lifetime.start.p0(i64 -1, ptr %19) nounwind
  store i8 0, ptr %19, align 1, !tbaa !8
  br label %215

215:
  %216 = load i8, ptr %19, align 1, !tbaa !8
  %217 = zext i8 %216 to i32
  %218 = load i8, ptr %6, align 1, !tbaa !8
  %219 = zext i8 %218 to i32
  %220 = add nsw i32 %219, 0
  %221 = trunc i32 %220 to i8
  %222 = zext i8 %221 to i32
  %223 = icmp slt i32 %217, %222
  br i1 %223, label %225, label %224

224:
  store i32 26, ptr %11, align 4
  call void @llvm.lifetime.end.p0(i64 -1, ptr %19) nounwind
  br label %247

225:
  %226 = load i8, ptr %4, align 1, !tbaa !8
  store i8 %226, ptr %9, align 1, !tbaa !8
  call void @llvm.lifetime.start.p0(i64 -1, ptr %20) nounwind
  store i8 0, ptr %20, align 1, !tbaa !8
  br label %227

227:
  %228 = load i8, ptr %20, align 1, !tbaa !8
  %229 = zext i8 %228 to i32
  %230 = load i8, ptr %6, align 1, !tbaa !8
  %231 = zext i8 %230 to i32
  %232 = add nsw i32 %231, 0
  %233 = trunc i32 %232 to i8
  %234 = zext i8 %233 to i32
  %235 = icmp slt i32 %229, %234
  br i1 %235, label %237, label %236

236:
  store i32 29, ptr %11, align 4
  call void @llvm.lifetime.end.p0(i64 -1, ptr %20) nounwind
  br label %243

237:
  store i8 -1, ptr %8, align 1, !tbaa !8
  %238 = load i8, ptr %7, align 1, !tbaa !8
  store i8 %238, ptr %9, align 1, !tbaa !8
  %239 = load i8, ptr %9, align 1, !tbaa !8
  store i8 %239, ptr %8, align 1, !tbaa !8
  br label %240

240:
  %241 = load i8, ptr %20, align 1, !tbaa !8
  %242 = add i8 %241, 1
  store i8 %242, ptr %20, align 1, !tbaa !8
  br label %227, !llvm.loop !16

243:
  br label %244

244:
  %245 = load i8, ptr %19, align 1, !tbaa !8
  %246 = add i8 %245, 1
  store i8 %246, ptr %19, align 1, !tbaa !8
  br label %215, !llvm.loop !17

247:
  br label %248

248:
  %249 = load i8, ptr %18, align 1, !tbaa !8
  %250 = add i8 %249, 1
  store i8 %250, ptr %18, align 1, !tbaa !8
  br label %204, !llvm.loop !18

251:
  br label %252

252:
  %253 = load i8, ptr %10, align 1, !tbaa !8
  %254 = add i8 %253, 1
  store i8 %254, ptr %10, align 1, !tbaa !8
  br label %21, !llvm.loop !19

255:
  %256 = load i8, ptr %4, align 1, !tbaa !8
  %257 = zext i8 %256 to i32
  %258 = load i8, ptr %5, align 1, !tbaa !8
  %259 = zext i8 %258 to i32
  %260 = add nsw i32 %257, %259
  %261 = load i8, ptr %7, align 1, !tbaa !8
  %262 = zext i8 %261 to i32
  %263 = add nsw i32 %260, %262
  %264 = load i8, ptr %8, align 1, !tbaa !8
  %265 = zext i8 %264 to i32
  %266 = add nsw i32 %263, %265
  %267 = load i8, ptr %9, align 1, !tbaa !8
  %268 = zext i8 %267 to i32
  %269 = add nsw i32 %266, %268
  %270 = trunc i32 %269 to i8
  store i32 1, ptr %11, align 4
  call void @llvm.lifetime.end.p0(i64 -1, ptr %9) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr %8) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr %7) nounwind
  ret i8 %270
}

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

!llvm.module.flags = !{!0, !1}
!llvm.ident = !{!2}
!llvm.errno.tbaa = !{!3}

!0 = !{i32 8, !"PIC Level", i32 2}
!1 = !{i32 7, !"PIE Level", i32 2}
!2 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!3 = !{!4, !5, i64 0}
!4 = !{!"__libc_errno", !5, i64 0}
!5 = !{!"int", !6, i64 0}
!6 = !{!"omnipotent char", !7, i64 0}
!7 = !{!"Simple C/C++ TBAA"}
!8 = !{!6, !6, i64 0}
!9 = distinct !{!9, !10}
!10 = !{!"llvm.loop.mustprogress"}
!11 = distinct !{!11, !10}
!12 = distinct !{!12, !10}
!13 = distinct !{!13, !10}
!14 = distinct !{!14, !10}
!15 = distinct !{!15, !10}
!16 = distinct !{!16, !10}
!17 = distinct !{!17, !10}
!18 = distinct !{!18, !10}
!19 = distinct !{!19, !10}
