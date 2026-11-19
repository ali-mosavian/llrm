target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"

%union.anon = type { i8 }
%union.anon.0 = type { i16 }
%union.anon.1 = type { i32 }
%union.anon.2 = type { i8 }
%union.anon.3 = type { i16 }
%union.anon.4 = type { i32 }
%union.anon.5 = type { i16 }
%union.anon.6 = type { i32 }
%union.anon.7 = type { i64 }
%union.anon.8 = type { i16 }
%union.anon.9 = type { i32 }
%union.anon.10 = type { i64 }
%union.anon.11 = type { i8 }
%union.anon.12 = type { i16 }
%union.anon.13 = type { i32 }
%union.anon.14 = type { i16 }
%union.anon.15 = type { i16 }
%union.anon.16 = type { i64 }
%union.anon.17 = type { i64 }

@sf = global [3 x i8] c"@\A0~", align 1
@f = global [3 x i16] [i16 16384, i16 -24576, i16 32440], align 2
@lf = global [3 x i32] [i32 1073741824, i32 -1610612736, i32 2126008811], align 4
@usf = global [3 x i8] c"\80\C0\FD", align 1
@uf = global [3 x i16] [i16 -32768, i16 -16384, i16 -656], align 2
@ulf = global [3 x i32] [i32 -2147483648, i32 -1073741824, i32 -42949673], align 4
@sa = global [3 x i16] [i16 192, i16 -288, i16 12800], align 2
@a = global [3 x i32] [i32 49152, i32 -73728, i32 3276800], align 4
@la = global [3 x i64] [i64 3221225472, i64 -4831838208, i64 64424509440000], align 8
@usa = global [3 x i16] [i16 384, i16 576, i16 25600], align 2
@ua = global [3 x i32] [i32 98304, i32 147456, i32 6553600], align 4
@ula = global [3 x i64] [i64 6442450944, i64 9663676416, i64 128849018880000], align 8

define i32 @main() nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" {
  %1 = alloca i32, align 2
  %2 = alloca i32, align 2
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  %5 = alloca i32, align 2
  %6 = alloca i32, align 2
  %7 = alloca i32, align 2
  br label %8

8:
  %9 = phi i32 [ 0, %0 ], [ %21, %20 ]
  br label %10

10:
  %11 = phi ptr [ %19, %10 ], [ @sf, %8 ]
  %12 = phi i32 [ %18, %10 ], [ 3, %8 ]
  %13 = getelementptr inbounds i8, ptr @sf, i32 %9
  %14 = load volatile i8, ptr %13, align 1, !tbaa !12
  %15 = load volatile i8, ptr %11, align 1, !tbaa !12
  %16 = tail call i8 @llvm.smul.fix.sat.i8(i8 %14, i8 %15, i32 7)
  %17 = zext i8 %16 to i32
  tail call void @report(i32 noundef %17) nobuiltin "no-builtins"
  %18 = add nsw i32 %12, -1
  %exitcond = icmp eq i32 %18, 0
  %19 = getelementptr i8, ptr %11, i32 1
  br i1 %exitcond, label %20, label %10, !llvm.loop !15

20:
  %21 = add nuw nsw i32 %9, 1
  %exitcond1 = icmp eq i32 %21, 3
  br i1 %exitcond1, label %22, label %8, !llvm.loop !17

22:
  br label %23

23:
  %24 = phi i32 [ %36, %35 ], [ 0, %22 ]
  %25 = getelementptr inbounds nuw [2 x i8], ptr @f, i32 %24
  br label %26

26:
  %27 = phi ptr [ %34, %26 ], [ @f, %23 ]
  %28 = phi i32 [ %33, %26 ], [ 3, %23 ]
  %29 = load volatile i16, ptr %25, align 2, !tbaa !20
  %30 = load volatile i16, ptr %27, align 2, !tbaa !20
  %31 = tail call i16 @llvm.smul.fix.sat.i16(i16 %29, i16 %30, i32 15)
  %32 = zext i16 %31 to i32
  tail call void @report(i32 noundef %32) nobuiltin "no-builtins"
  %33 = add nsw i32 %28, -1
  %exitcond2 = icmp eq i32 %33, 0
  %34 = getelementptr i8, ptr %27, i32 2
  br i1 %exitcond2, label %35, label %26, !llvm.loop !22

35:
  %36 = add nuw nsw i32 %24, 1
  %exitcond3 = icmp eq i32 %36, 3
  br i1 %exitcond3, label %37, label %23, !llvm.loop !23

37:
  br label %38

38:
  %39 = phi i32 [ %50, %49 ], [ 0, %37 ]
  %40 = getelementptr inbounds nuw [4 x i8], ptr @lf, i32 %39
  br label %41

41:
  %42 = phi ptr [ %48, %41 ], [ @lf, %38 ]
  %43 = phi i32 [ %47, %41 ], [ 3, %38 ]
  %44 = load volatile i32, ptr %40, align 4, !tbaa !26
  %45 = load volatile i32, ptr %42, align 4, !tbaa !26
  %46 = tail call i32 @llvm.smul.fix.sat.i32(i32 %44, i32 %45, i32 31)
  tail call void @report(i32 noundef %46) nobuiltin "no-builtins"
  %47 = add nsw i32 %43, -1
  %exitcond4 = icmp eq i32 %47, 0
  %48 = getelementptr i8, ptr %42, i32 4
  br i1 %exitcond4, label %49, label %41, !llvm.loop !28

49:
  %50 = add nuw nsw i32 %39, 1
  %exitcond5 = icmp eq i32 %50, 3
  br i1 %exitcond5, label %51, label %38, !llvm.loop !29

51:
  br label %52

52:
  %53 = phi i32 [ %65, %64 ], [ 0, %51 ]
  br label %54

54:
  %55 = phi ptr [ %63, %54 ], [ @usf, %52 ]
  %56 = phi i32 [ %62, %54 ], [ 3, %52 ]
  %57 = getelementptr inbounds i8, ptr @usf, i32 %53
  %58 = load volatile i8, ptr %57, align 1, !tbaa !12
  %59 = load volatile i8, ptr %55, align 1, !tbaa !12
  %60 = tail call i8 @llvm.umul.fix.sat.i8(i8 %58, i8 %59, i32 8)
  %61 = zext i8 %60 to i32
  tail call void @report(i32 noundef %61) nobuiltin "no-builtins"
  %62 = add nsw i32 %56, -1
  %exitcond6 = icmp eq i32 %62, 0
  %63 = getelementptr i8, ptr %55, i32 1
  br i1 %exitcond6, label %64, label %54, !llvm.loop !30

64:
  %65 = add nuw nsw i32 %53, 1
  %exitcond7 = icmp eq i32 %65, 3
  br i1 %exitcond7, label %66, label %52, !llvm.loop !31

66:
  br label %67

67:
  %68 = phi i32 [ %80, %79 ], [ 0, %66 ]
  %69 = getelementptr inbounds nuw [2 x i8], ptr @uf, i32 %68
  br label %70

70:
  %71 = phi ptr [ %78, %70 ], [ @uf, %67 ]
  %72 = phi i32 [ %77, %70 ], [ 3, %67 ]
  %73 = load volatile i16, ptr %69, align 2, !tbaa !20
  %74 = load volatile i16, ptr %71, align 2, !tbaa !20
  %75 = tail call i16 @llvm.umul.fix.sat.i16(i16 %73, i16 %74, i32 16)
  %76 = zext i16 %75 to i32
  tail call void @report(i32 noundef %76) nobuiltin "no-builtins"
  %77 = add nsw i32 %72, -1
  %exitcond8 = icmp eq i32 %77, 0
  %78 = getelementptr i8, ptr %71, i32 2
  br i1 %exitcond8, label %79, label %70, !llvm.loop !32

79:
  %80 = add nuw nsw i32 %68, 1
  %exitcond9 = icmp eq i32 %80, 3
  br i1 %exitcond9, label %81, label %67, !llvm.loop !33

81:
  br label %82

82:
  %83 = phi i32 [ %94, %93 ], [ 0, %81 ]
  %84 = getelementptr inbounds nuw [4 x i8], ptr @ulf, i32 %83
  br label %85

85:
  %86 = phi ptr [ %92, %85 ], [ @ulf, %82 ]
  %87 = phi i32 [ %91, %85 ], [ 3, %82 ]
  %88 = load volatile i32, ptr %84, align 4, !tbaa !26
  %89 = load volatile i32, ptr %86, align 4, !tbaa !26
  %90 = tail call i32 @llvm.umul.fix.sat.i32(i32 %88, i32 %89, i32 32)
  tail call void @report(i32 noundef %90) nobuiltin "no-builtins"
  %91 = add nsw i32 %87, -1
  %exitcond10 = icmp eq i32 %91, 0
  %92 = getelementptr i8, ptr %86, i32 4
  br i1 %exitcond10, label %93, label %85, !llvm.loop !34

93:
  %94 = add nuw nsw i32 %83, 1
  %exitcond11 = icmp eq i32 %94, 3
  br i1 %exitcond11, label %95, label %82, !llvm.loop !35

95:
  br label %96

96:
  %97 = phi i32 [ %109, %108 ], [ 0, %95 ]
  %98 = getelementptr inbounds nuw [2 x i8], ptr @sa, i32 %97
  br label %99

99:
  %100 = phi ptr [ %107, %99 ], [ @sa, %96 ]
  %101 = phi i32 [ %106, %99 ], [ 3, %96 ]
  %102 = load volatile i16, ptr %98, align 2, !tbaa !38
  %103 = load volatile i16, ptr %100, align 2, !tbaa !38
  %104 = tail call i16 @llvm.smul.fix.sat.i16(i16 %102, i16 %103, i32 7)
  %105 = zext i16 %104 to i32
  tail call void @report(i32 noundef %105) nobuiltin "no-builtins"
  %106 = add nsw i32 %101, -1
  %exitcond12 = icmp eq i32 %106, 0
  %107 = getelementptr i8, ptr %100, i32 2
  br i1 %exitcond12, label %108, label %99, !llvm.loop !40

108:
  %109 = add nuw nsw i32 %97, 1
  %exitcond13 = icmp eq i32 %109, 3
  br i1 %exitcond13, label %110, label %96, !llvm.loop !41

110:
  br label %111

111:
  %112 = phi i32 [ %123, %122 ], [ 0, %110 ]
  %113 = getelementptr inbounds nuw [4 x i8], ptr @a, i32 %112
  br label %114

114:
  %115 = phi ptr [ %121, %114 ], [ @a, %111 ]
  %116 = phi i32 [ %120, %114 ], [ 3, %111 ]
  %117 = load volatile i32, ptr %113, align 4, !tbaa !44
  %118 = load volatile i32, ptr %115, align 4, !tbaa !44
  %119 = tail call i32 @llvm.smul.fix.sat.i32(i32 %117, i32 %118, i32 15)
  tail call void @report(i32 noundef %119) nobuiltin "no-builtins"
  %120 = add nsw i32 %116, -1
  %exitcond14 = icmp eq i32 %120, 0
  %121 = getelementptr i8, ptr %115, i32 4
  br i1 %exitcond14, label %122, label %114, !llvm.loop !46

122:
  %123 = add nuw nsw i32 %112, 1
  %exitcond15 = icmp eq i32 %123, 3
  br i1 %exitcond15, label %124, label %111, !llvm.loop !47

124:
  br label %125

125:
  %126 = phi i32 [ %138, %137 ], [ 0, %124 ]
  %127 = getelementptr inbounds nuw [8 x i8], ptr @la, i32 %126
  br label %128

128:
  %129 = phi ptr [ %136, %128 ], [ @la, %125 ]
  %130 = phi i32 [ %135, %128 ], [ 3, %125 ]
  %131 = load volatile i64, ptr %127, align 8, !tbaa !50
  %132 = load volatile i64, ptr %129, align 8, !tbaa !50
  %133 = tail call i64 @llvm.smul.fix.sat.i64(i64 %131, i64 %132, i32 31)
  %134 = trunc i64 %133 to i32
  tail call void @report(i32 noundef %134) nobuiltin "no-builtins"
  %135 = add nsw i32 %130, -1
  %136 = getelementptr i8, ptr %129, i32 8
  %exitcond16 = icmp eq i32 %135, 0
  br i1 %exitcond16, label %137, label %128, !llvm.loop !52

137:
  %138 = add nuw nsw i32 %126, 1
  %exitcond17 = icmp eq i32 %138, 3
  br i1 %exitcond17, label %139, label %125, !llvm.loop !53

139:
  br label %140

140:
  %141 = phi i32 [ %153, %152 ], [ 0, %139 ]
  %142 = getelementptr inbounds nuw [2 x i8], ptr @usa, i32 %141
  br label %143

143:
  %144 = phi ptr [ %151, %143 ], [ @usa, %140 ]
  %145 = phi i32 [ %150, %143 ], [ 3, %140 ]
  %146 = load volatile i16, ptr %142, align 2, !tbaa !38
  %147 = load volatile i16, ptr %144, align 2, !tbaa !38
  %148 = tail call i16 @llvm.umul.fix.sat.i16(i16 %146, i16 %147, i32 8)
  %149 = zext i16 %148 to i32
  tail call void @report(i32 noundef %149) nobuiltin "no-builtins"
  %150 = add nsw i32 %145, -1
  %exitcond18 = icmp eq i32 %150, 0
  %151 = getelementptr i8, ptr %144, i32 2
  br i1 %exitcond18, label %152, label %143, !llvm.loop !54

152:
  %153 = add nuw nsw i32 %141, 1
  %exitcond19 = icmp eq i32 %153, 3
  br i1 %exitcond19, label %154, label %140, !llvm.loop !55

154:
  br label %155

155:
  %156 = phi i32 [ %167, %166 ], [ 0, %154 ]
  %157 = getelementptr inbounds nuw [4 x i8], ptr @ua, i32 %156
  br label %158

158:
  %159 = phi ptr [ %165, %158 ], [ @ua, %155 ]
  %160 = phi i32 [ %164, %158 ], [ 3, %155 ]
  %161 = load volatile i32, ptr %157, align 4, !tbaa !44
  %162 = load volatile i32, ptr %159, align 4, !tbaa !44
  %163 = tail call i32 @llvm.umul.fix.sat.i32(i32 %161, i32 %162, i32 16)
  tail call void @report(i32 noundef %163) nobuiltin "no-builtins"
  %164 = add nsw i32 %160, -1
  %exitcond20 = icmp eq i32 %164, 0
  %165 = getelementptr i8, ptr %159, i32 4
  br i1 %exitcond20, label %166, label %158, !llvm.loop !56

166:
  %167 = add nuw nsw i32 %156, 1
  %exitcond21 = icmp eq i32 %167, 3
  br i1 %exitcond21, label %168, label %155, !llvm.loop !57

168:
  br label %169

169:
  %170 = phi i32 [ %182, %181 ], [ 0, %168 ]
  %171 = getelementptr inbounds nuw [8 x i8], ptr @ula, i32 %170
  br label %172

172:
  %173 = phi ptr [ %180, %172 ], [ @ula, %169 ]
  %174 = phi i32 [ %179, %172 ], [ 3, %169 ]
  %175 = load volatile i64, ptr %171, align 8, !tbaa !50
  %176 = load volatile i64, ptr %173, align 8, !tbaa !50
  %177 = tail call i64 @llvm.umul.fix.sat.i64(i64 %175, i64 %176, i32 32)
  %178 = trunc i64 %177 to i32
  tail call void @report(i32 noundef %178) nobuiltin "no-builtins"
  %179 = add nsw i32 %174, -1
  %180 = getelementptr i8, ptr %173, i32 8
  %exitcond22 = icmp eq i32 %179, 0
  br i1 %exitcond22, label %181, label %172, !llvm.loop !58

181:
  %182 = add nuw nsw i32 %170, 1
  %exitcond23 = icmp eq i32 %182, 3
  br i1 %exitcond23, label %183, label %169, !llvm.loop !59

183:
  call void @llvm.lifetime.start.p0(i64 4, ptr nonnull %1) nounwind
  store volatile i32 3, ptr %1, align 2
  br label %184

184:
  %185 = phi ptr [ %193, %184 ], [ @sf, %183 ]
  %186 = phi i32 [ %192, %184 ], [ 3, %183 ]
  %187 = load volatile i8, ptr %185, align 1, !tbaa !12
  %188 = load volatile i32, ptr %1, align 2, !tbaa !11
  %189 = trunc i32 %188 to i8
  %190 = tail call i8 @llvm.sshl.sat.i8(i8 %187, i8 %189)
  %191 = zext i8 %190 to i32
  tail call void @report(i32 noundef %191) nobuiltin "no-builtins"
  %192 = add nsw i32 %186, -1
  %exitcond24 = icmp eq i32 %192, 0
  %193 = getelementptr i8, ptr %185, i32 1
  br i1 %exitcond24, label %194, label %184, !llvm.loop !60

194:
  call void @llvm.lifetime.end.p0(i64 4, ptr nonnull %1) nounwind
  call void @llvm.lifetime.start.p0(i64 4, ptr nonnull %2) nounwind
  store volatile i32 3, ptr %2, align 2
  br label %195

195:
  %196 = phi ptr [ %204, %195 ], [ @f, %194 ]
  %197 = phi i32 [ %203, %195 ], [ 3, %194 ]
  %198 = load volatile i16, ptr %196, align 2, !tbaa !20
  %199 = load volatile i32, ptr %2, align 2, !tbaa !11
  %200 = trunc i32 %199 to i16
  %201 = tail call i16 @llvm.sshl.sat.i16(i16 %198, i16 %200)
  %202 = zext i16 %201 to i32
  tail call void @report(i32 noundef %202) nobuiltin "no-builtins"
  %203 = add nsw i32 %197, -1
  %exitcond25 = icmp eq i32 %203, 0
  %204 = getelementptr i8, ptr %196, i32 2
  br i1 %exitcond25, label %205, label %195, !llvm.loop !61

205:
  call void @llvm.lifetime.end.p0(i64 4, ptr nonnull %2) nounwind
  call void @llvm.lifetime.start.p0(i64 4, ptr nonnull %3) nounwind
  store volatile i32 3, ptr %3, align 2
  br label %206

206:
  %207 = phi ptr [ %213, %206 ], [ @lf, %205 ]
  %208 = phi i32 [ %212, %206 ], [ 3, %205 ]
  %209 = load volatile i32, ptr %207, align 4, !tbaa !26
  %210 = load volatile i32, ptr %3, align 2, !tbaa !11
  %211 = tail call i32 @llvm.sshl.sat.i32(i32 %209, i32 %210)
  tail call void @report(i32 noundef %211) nobuiltin "no-builtins"
  %212 = add nsw i32 %208, -1
  %exitcond26 = icmp eq i32 %212, 0
  %213 = getelementptr i8, ptr %207, i32 4
  br i1 %exitcond26, label %214, label %206, !llvm.loop !62

214:
  call void @llvm.lifetime.end.p0(i64 4, ptr nonnull %3) nounwind
  call void @llvm.lifetime.start.p0(i64 4, ptr nonnull %4) nounwind
  store volatile i32 3, ptr %4, align 2
  br label %215

215:
  %216 = phi ptr [ %224, %215 ], [ @uf, %214 ]
  %217 = phi i32 [ %223, %215 ], [ 3, %214 ]
  %218 = load volatile i16, ptr %216, align 2, !tbaa !20
  %219 = load volatile i32, ptr %4, align 2, !tbaa !11
  %220 = trunc i32 %219 to i16
  %221 = tail call i16 @llvm.ushl.sat.i16(i16 %218, i16 %220)
  %222 = zext i16 %221 to i32
  tail call void @report(i32 noundef %222) nobuiltin "no-builtins"
  %223 = add nsw i32 %217, -1
  %exitcond27 = icmp eq i32 %223, 0
  %224 = getelementptr i8, ptr %216, i32 2
  br i1 %exitcond27, label %225, label %215, !llvm.loop !63

225:
  call void @llvm.lifetime.end.p0(i64 4, ptr nonnull %4) nounwind
  call void @llvm.lifetime.start.p0(i64 4, ptr nonnull %5) nounwind
  store volatile i32 3, ptr %5, align 2
  br label %226

226:
  %227 = phi ptr [ %235, %226 ], [ @sa, %225 ]
  %228 = phi i32 [ %234, %226 ], [ 3, %225 ]
  %229 = load volatile i16, ptr %227, align 2, !tbaa !38
  %230 = load volatile i32, ptr %5, align 2, !tbaa !11
  %231 = trunc i32 %230 to i16
  %232 = tail call i16 @llvm.sshl.sat.i16(i16 %229, i16 %231)
  %233 = zext i16 %232 to i32
  tail call void @report(i32 noundef %233) nobuiltin "no-builtins"
  %234 = add nsw i32 %228, -1
  %exitcond28 = icmp eq i32 %234, 0
  %235 = getelementptr i8, ptr %227, i32 2
  br i1 %exitcond28, label %236, label %226, !llvm.loop !64

236:
  call void @llvm.lifetime.end.p0(i64 4, ptr nonnull %5) nounwind
  call void @llvm.lifetime.start.p0(i64 4, ptr nonnull %6) nounwind
  store volatile i32 3, ptr %6, align 2
  br label %237

237:
  %238 = phi ptr [ %246, %237 ], [ @la, %236 ]
  %239 = phi i32 [ %245, %237 ], [ 3, %236 ]
  %240 = load volatile i64, ptr %238, align 8, !tbaa !50
  %241 = load volatile i32, ptr %6, align 2, !tbaa !11
  %242 = zext i32 %241 to i64
  %243 = tail call i64 @llvm.sshl.sat.i64(i64 %240, i64 %242)
  %244 = trunc i64 %243 to i32
  tail call void @report(i32 noundef %244) nobuiltin "no-builtins"
  %245 = add nsw i32 %239, -1
  %246 = getelementptr i8, ptr %238, i32 8
  %exitcond29 = icmp eq i32 %245, 0
  br i1 %exitcond29, label %247, label %237, !llvm.loop !65

247:
  call void @llvm.lifetime.end.p0(i64 4, ptr nonnull %6) nounwind
  call void @llvm.lifetime.start.p0(i64 4, ptr nonnull %7) nounwind
  store volatile i32 3, ptr %7, align 2
  br label %248

248:
  %249 = phi ptr [ %257, %248 ], [ @ula, %247 ]
  %250 = phi i32 [ %256, %248 ], [ 3, %247 ]
  %251 = load volatile i64, ptr %249, align 8, !tbaa !50
  %252 = load volatile i32, ptr %7, align 2, !tbaa !11
  %253 = zext i32 %252 to i64
  %254 = tail call i64 @llvm.ushl.sat.i64(i64 %251, i64 %253)
  %255 = trunc i64 %254 to i32
  tail call void @report(i32 noundef %255) nobuiltin "no-builtins"
  %256 = add nsw i32 %250, -1
  %257 = getelementptr i8, ptr %249, i32 8
  %exitcond30 = icmp eq i32 %256, 0
  br i1 %exitcond30, label %258, label %248, !llvm.loop !66

258:
  call void @llvm.lifetime.end.p0(i64 4, ptr nonnull %7) nounwind
  ret i32 0
}

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare i8 @llvm.smul.fix.sat.i8(i8, i8, i32 immarg) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare void @report(i32 noundef) "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000"

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare i16 @llvm.smul.fix.sat.i16(i16, i16, i32 immarg) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare i32 @llvm.smul.fix.sat.i32(i32, i32, i32 immarg) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare i8 @llvm.umul.fix.sat.i8(i8, i8, i32 immarg) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare i16 @llvm.umul.fix.sat.i16(i16, i16, i32 immarg) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare i32 @llvm.umul.fix.sat.i32(i32, i32, i32 immarg) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare i64 @llvm.smul.fix.sat.i64(i64, i64, i32 immarg) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare i64 @llvm.umul.fix.sat.i64(i64, i64, i32 immarg) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare i8 @llvm.sshl.sat.i8(i8, i8) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare i16 @llvm.sshl.sat.i16(i16, i16) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare i32 @llvm.sshl.sat.i32(i32, i32) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare i16 @llvm.ushl.sat.i16(i16, i16) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare i64 @llvm.sshl.sat.i64(i64, i64) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare i64 @llvm.ushl.sat.i64(i64, i64) nocallback nofree nosync nounwind speculatable willreturn memory(none)

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
!8 = !{!9, !9, i64 0}
!9 = !{!"p1 _Sat short _Fract", !10, i64 0}
!10 = !{!"any pointer", !6, i64 0}
!11 = !{!5, !5, i64 0}
!12 = !{!13, !13, i64 0}
!13 = !{!"_Sat short _Fract", !6, i64 0}
!14 = !{!6, !6, i64 0}
!15 = distinct !{!15, !16}
!16 = !{!"llvm.loop.mustprogress"}
!17 = distinct !{!17, !16}
!18 = !{!19, !19, i64 0}
!19 = !{!"p1 _Sat _Fract", !10, i64 0}
!20 = !{!21, !21, i64 0}
!21 = !{!"_Sat _Fract", !6, i64 0}
!22 = distinct !{!22, !16}
!23 = distinct !{!23, !16}
!24 = !{!25, !25, i64 0}
!25 = !{!"p1 _Sat long _Fract", !10, i64 0}
!26 = !{!27, !27, i64 0}
!27 = !{!"_Sat long _Fract", !6, i64 0}
!28 = distinct !{!28, !16}
!29 = distinct !{!29, !16}
!30 = distinct !{!30, !16}
!31 = distinct !{!31, !16}
!32 = distinct !{!32, !16}
!33 = distinct !{!33, !16}
!34 = distinct !{!34, !16}
!35 = distinct !{!35, !16}
!36 = !{!37, !37, i64 0}
!37 = !{!"p1 _Sat short _Accum", !10, i64 0}
!38 = !{!39, !39, i64 0}
!39 = !{!"_Sat short _Accum", !6, i64 0}
!40 = distinct !{!40, !16}
!41 = distinct !{!41, !16}
!42 = !{!43, !43, i64 0}
!43 = !{!"p1 _Sat _Accum", !10, i64 0}
!44 = !{!45, !45, i64 0}
!45 = !{!"_Sat _Accum", !6, i64 0}
!46 = distinct !{!46, !16}
!47 = distinct !{!47, !16}
!48 = !{!49, !49, i64 0}
!49 = !{!"p1 _Sat long _Accum", !10, i64 0}
!50 = !{!51, !51, i64 0}
!51 = !{!"_Sat long _Accum", !6, i64 0}
!52 = distinct !{!52, !16}
!53 = distinct !{!53, !16}
!54 = distinct !{!54, !16}
!55 = distinct !{!55, !16}
!56 = distinct !{!56, !16}
!57 = distinct !{!57, !16}
!58 = distinct !{!58, !16}
!59 = distinct !{!59, !16}
!60 = distinct !{!60, !16}
!61 = distinct !{!61, !16}
!62 = distinct !{!62, !16}
!63 = distinct !{!63, !16}
!64 = distinct !{!64, !16}
!65 = distinct !{!65, !16}
!66 = distinct !{!66, !16}
