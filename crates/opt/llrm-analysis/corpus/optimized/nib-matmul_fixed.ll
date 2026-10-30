target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [15 x i8] c"\08\00\08\00\08\00matmul: \00"

define cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
  %1 = alloca [256 x i8]
  %2 = alloca [256 x i8]
  %3 = alloca [256 x i8]
  call void @llvm.lifetime.start.p0(i64 256, ptr nonnull %3)
  call void @llvm.lifetime.start.p0(i64 256, ptr nonnull %2)
  call void @llvm.lifetime.start.p0(i64 256, ptr nonnull %1)
  %4 = getelementptr inbounds nuw i8, ptr %3, i16 176
  call void @llvm.memset.p0.i16(ptr nonnull align 2 %4, i8 0, i16 80, i1 false)
  %5 = getelementptr inbounds nuw i8, ptr %2, i16 176
  call void @llvm.memset.p0.i16(ptr nonnull align 2 %5, i8 0, i16 80, i1 false)
  call void @llvm.memset.p0.i16(ptr nonnull align 2 %1, i8 0, i16 256, i1 false)
  store i32 128, ptr %3, !tbaa !2
  store i32 512, ptr %2, !tbaa !2
  %6 = getelementptr inbounds nuw i8, ptr %3, i16 4
  store i32 192, ptr %6, !tbaa !2
  %7 = getelementptr inbounds nuw i8, ptr %2, i16 4
  store i32 128, ptr %7, !tbaa !2
  %8 = getelementptr inbounds nuw i8, ptr %3, i16 8
  store i32 256, ptr %8, !tbaa !2
  %9 = getelementptr inbounds nuw i8, ptr %2, i16 8
  store i32 256, ptr %9, !tbaa !2
  %10 = getelementptr inbounds nuw i8, ptr %3, i16 12
  store i32 320, ptr %10, !tbaa !2
  %11 = getelementptr inbounds nuw i8, ptr %2, i16 12
  store i32 0, ptr %11, !tbaa !2
  %12 = getelementptr inbounds nuw i8, ptr %3, i16 16
  store i32 384, ptr %12, !tbaa !2
  %13 = getelementptr inbounds nuw i8, ptr %2, i16 16
  store i32 128, ptr %13, !tbaa !2
  %14 = getelementptr inbounds nuw i8, ptr %3, i16 20
  store i32 448, ptr %14, !tbaa !2
  %15 = getelementptr inbounds nuw i8, ptr %2, i16 20
  store i32 256, ptr %15, !tbaa !2
  %16 = getelementptr inbounds nuw i8, ptr %3, i16 24
  store i32 512, ptr %16, !tbaa !2
  %17 = getelementptr inbounds nuw i8, ptr %2, i16 24
  store i32 0, ptr %17, !tbaa !2
  %18 = getelementptr inbounds nuw i8, ptr %3, i16 28
  store i32 576, ptr %18, !tbaa !2
  %19 = getelementptr inbounds nuw i8, ptr %2, i16 28
  store i32 128, ptr %19, !tbaa !2
  %20 = getelementptr inbounds nuw i8, ptr %3, i16 32
  store i32 320, ptr %20, !tbaa !2
  %21 = getelementptr inbounds nuw i8, ptr %2, i16 32
  store i32 128, ptr %21, !tbaa !2
  %22 = getelementptr inbounds nuw i8, ptr %3, i16 36
  store i32 384, ptr %22, !tbaa !2
  %23 = getelementptr inbounds nuw i8, ptr %2, i16 36
  store i32 512, ptr %23, !tbaa !2
  %24 = getelementptr inbounds nuw i8, ptr %3, i16 40
  store i32 448, ptr %24, !tbaa !2
  %25 = getelementptr inbounds nuw i8, ptr %2, i16 40
  store i32 0, ptr %25, !tbaa !2
  %26 = getelementptr inbounds nuw i8, ptr %3, i16 44
  store i32 512, ptr %26, !tbaa !2
  %27 = getelementptr inbounds nuw i8, ptr %2, i16 44
  store i32 128, ptr %27, !tbaa !2
  %28 = getelementptr inbounds nuw i8, ptr %3, i16 48
  store i32 576, ptr %28, !tbaa !2
  %29 = getelementptr inbounds nuw i8, ptr %2, i16 48
  store i32 256, ptr %29, !tbaa !2
  %30 = getelementptr inbounds nuw i8, ptr %3, i16 52
  store i32 640, ptr %30, !tbaa !2
  %31 = getelementptr inbounds nuw i8, ptr %2, i16 52
  store i32 0, ptr %31, !tbaa !2
  %32 = getelementptr inbounds nuw i8, ptr %3, i16 56
  store i32 704, ptr %32, !tbaa !2
  %33 = getelementptr inbounds nuw i8, ptr %2, i16 56
  store i32 128, ptr %33, !tbaa !2
  %34 = getelementptr inbounds nuw i8, ptr %3, i16 60
  store i32 768, ptr %34, !tbaa !2
  %35 = getelementptr inbounds nuw i8, ptr %2, i16 60
  store i32 256, ptr %35, !tbaa !2
  %36 = getelementptr inbounds nuw i8, ptr %3, i16 64
  store i32 512, ptr %36, !tbaa !2
  %37 = getelementptr inbounds nuw i8, ptr %2, i16 64
  store i32 256, ptr %37, !tbaa !2
  %38 = getelementptr inbounds nuw i8, ptr %3, i16 68
  store i32 576, ptr %38, !tbaa !2
  %39 = getelementptr inbounds nuw i8, ptr %2, i16 68
  store i32 0, ptr %39, !tbaa !2
  %40 = getelementptr inbounds nuw i8, ptr %3, i16 72
  store i32 640, ptr %40, !tbaa !2
  %41 = getelementptr inbounds nuw i8, ptr %2, i16 72
  store i32 512, ptr %41, !tbaa !2
  %42 = getelementptr inbounds nuw i8, ptr %3, i16 76
  store i32 704, ptr %42, !tbaa !2
  %43 = getelementptr inbounds nuw i8, ptr %2, i16 76
  store i32 256, ptr %43, !tbaa !2
  %44 = getelementptr inbounds nuw i8, ptr %3, i16 80
  store i32 768, ptr %44, !tbaa !2
  %45 = getelementptr inbounds nuw i8, ptr %2, i16 80
  store i32 0, ptr %45, !tbaa !2
  %46 = getelementptr inbounds nuw i8, ptr %3, i16 84
  store i32 832, ptr %46, !tbaa !2
  %47 = getelementptr inbounds nuw i8, ptr %2, i16 84
  store i32 128, ptr %47, !tbaa !2
  %48 = getelementptr inbounds nuw i8, ptr %3, i16 88
  store i32 896, ptr %48, !tbaa !2
  %49 = getelementptr inbounds nuw i8, ptr %2, i16 88
  store i32 256, ptr %49, !tbaa !2
  %50 = getelementptr inbounds nuw i8, ptr %3, i16 92
  store i32 960, ptr %50, !tbaa !2
  %51 = getelementptr inbounds nuw i8, ptr %2, i16 92
  store i32 0, ptr %51, !tbaa !2
  %52 = getelementptr inbounds nuw i8, ptr %3, i16 96
  store i32 704, ptr %52, !tbaa !2
  %53 = getelementptr inbounds nuw i8, ptr %2, i16 96
  store i32 0, ptr %53, !tbaa !2
  %54 = getelementptr inbounds nuw i8, ptr %3, i16 100
  store i32 768, ptr %54, !tbaa !2
  %55 = getelementptr inbounds nuw i8, ptr %2, i16 100
  store i32 128, ptr %55, !tbaa !2
  %56 = getelementptr inbounds nuw i8, ptr %3, i16 104
  store i32 832, ptr %56, !tbaa !2
  %57 = getelementptr inbounds nuw i8, ptr %2, i16 104
  store i32 256, ptr %57, !tbaa !2
  %58 = getelementptr inbounds nuw i8, ptr %3, i16 108
  store i32 896, ptr %58, !tbaa !2
  %59 = getelementptr inbounds nuw i8, ptr %2, i16 108
  store i32 512, ptr %59, !tbaa !2
  %60 = getelementptr inbounds nuw i8, ptr %3, i16 112
  store i32 960, ptr %60, !tbaa !2
  %61 = getelementptr inbounds nuw i8, ptr %2, i16 112
  store i32 128, ptr %61, !tbaa !2
  %62 = getelementptr inbounds nuw i8, ptr %3, i16 116
  store i32 1024, ptr %62, !tbaa !2
  %63 = getelementptr inbounds nuw i8, ptr %2, i16 116
  store i32 256, ptr %63, !tbaa !2
  %64 = getelementptr inbounds nuw i8, ptr %3, i16 120
  store i32 1088, ptr %64, !tbaa !2
  %65 = getelementptr inbounds nuw i8, ptr %2, i16 120
  store i32 0, ptr %65, !tbaa !2
  %66 = getelementptr inbounds nuw i8, ptr %3, i16 124
  store i32 1152, ptr %66, !tbaa !2
  %67 = getelementptr inbounds nuw i8, ptr %2, i16 124
  store i32 128, ptr %67, !tbaa !2
  %68 = getelementptr inbounds nuw i8, ptr %3, i16 128
  store i32 896, ptr %68, !tbaa !2
  %69 = getelementptr inbounds nuw i8, ptr %2, i16 128
  store i32 128, ptr %69, !tbaa !2
  %70 = getelementptr inbounds nuw i8, ptr %3, i16 132
  store i32 960, ptr %70, !tbaa !2
  %71 = getelementptr inbounds nuw i8, ptr %2, i16 132
  store i32 256, ptr %71, !tbaa !2
  %72 = getelementptr inbounds nuw i8, ptr %3, i16 136
  store i32 1024, ptr %72, !tbaa !2
  %73 = getelementptr inbounds nuw i8, ptr %2, i16 136
  store i32 0, ptr %73, !tbaa !2
  %74 = getelementptr inbounds nuw i8, ptr %3, i16 140
  store i32 1088, ptr %74, !tbaa !2
  %75 = getelementptr inbounds nuw i8, ptr %2, i16 140
  store i32 128, ptr %75, !tbaa !2
  %76 = getelementptr inbounds nuw i8, ptr %3, i16 144
  store i32 1152, ptr %76, !tbaa !2
  %77 = getelementptr inbounds nuw i8, ptr %2, i16 144
  store i32 512, ptr %77, !tbaa !2
  %78 = getelementptr inbounds nuw i8, ptr %3, i16 148
  store i32 1216, ptr %78, !tbaa !2
  %79 = getelementptr inbounds nuw i8, ptr %2, i16 148
  store i32 0, ptr %79, !tbaa !2
  %80 = getelementptr inbounds nuw i8, ptr %3, i16 152
  store i32 1280, ptr %80, !tbaa !2
  %81 = getelementptr inbounds nuw i8, ptr %2, i16 152
  store i32 128, ptr %81, !tbaa !2
  %82 = getelementptr inbounds nuw i8, ptr %3, i16 156
  store i32 1344, ptr %82, !tbaa !2
  %83 = getelementptr inbounds nuw i8, ptr %2, i16 156
  store i32 256, ptr %83, !tbaa !2
  %84 = getelementptr inbounds nuw i8, ptr %3, i16 160
  store i32 1088, ptr %84, !tbaa !2
  %85 = getelementptr inbounds nuw i8, ptr %2, i16 160
  store i32 256, ptr %85, !tbaa !2
  %86 = getelementptr inbounds nuw i8, ptr %3, i16 164
  store i32 1152, ptr %86, !tbaa !2
  %87 = getelementptr inbounds nuw i8, ptr %2, i16 164
  store i32 0, ptr %87, !tbaa !2
  %88 = getelementptr inbounds nuw i8, ptr %3, i16 168
  store i32 1216, ptr %88, !tbaa !2
  %89 = getelementptr inbounds nuw i8, ptr %2, i16 168
  store i32 128, ptr %89, !tbaa !2
  %90 = getelementptr inbounds nuw i8, ptr %3, i16 172
  store i32 1280, ptr %90, !tbaa !2
  %91 = getelementptr inbounds nuw i8, ptr %2, i16 172
  store i32 256, ptr %91, !tbaa !2
  %92 = getelementptr inbounds nuw i8, ptr %3, i16 176
  store i32 1344, ptr %92, !tbaa !2
  %93 = getelementptr inbounds nuw i8, ptr %2, i16 176
  store i32 0, ptr %93, !tbaa !2
  %94 = getelementptr inbounds nuw i8, ptr %3, i16 180
  store i32 1408, ptr %94, !tbaa !2
  %95 = getelementptr inbounds nuw i8, ptr %2, i16 180
  store i32 512, ptr %95, !tbaa !2
  %96 = getelementptr inbounds nuw i8, ptr %3, i16 184
  store i32 1472, ptr %96, !tbaa !2
  %97 = getelementptr inbounds nuw i8, ptr %2, i16 184
  store i32 256, ptr %97, !tbaa !2
  %98 = getelementptr inbounds nuw i8, ptr %3, i16 188
  store i32 1536, ptr %98, !tbaa !2
  %99 = getelementptr inbounds nuw i8, ptr %2, i16 188
  store i32 0, ptr %99, !tbaa !2
  %100 = getelementptr inbounds nuw i8, ptr %3, i16 192
  store i32 1280, ptr %100, !tbaa !2
  %101 = getelementptr inbounds nuw i8, ptr %2, i16 192
  store i32 0, ptr %101, !tbaa !2
  %102 = getelementptr inbounds nuw i8, ptr %3, i16 196
  store i32 1344, ptr %102, !tbaa !2
  %103 = getelementptr inbounds nuw i8, ptr %2, i16 196
  store i32 128, ptr %103, !tbaa !2
  %104 = getelementptr inbounds nuw i8, ptr %3, i16 200
  store i32 1408, ptr %104, !tbaa !2
  %105 = getelementptr inbounds nuw i8, ptr %2, i16 200
  store i32 256, ptr %105, !tbaa !2
  %106 = getelementptr inbounds nuw i8, ptr %3, i16 204
  store i32 1472, ptr %106, !tbaa !2
  %107 = getelementptr inbounds nuw i8, ptr %2, i16 204
  store i32 0, ptr %107, !tbaa !2
  %108 = getelementptr inbounds nuw i8, ptr %3, i16 208
  store i32 1536, ptr %108, !tbaa !2
  %109 = getelementptr inbounds nuw i8, ptr %2, i16 208
  store i32 128, ptr %109, !tbaa !2
  %110 = getelementptr inbounds nuw i8, ptr %3, i16 212
  store i32 1600, ptr %110, !tbaa !2
  %111 = getelementptr inbounds nuw i8, ptr %2, i16 212
  store i32 256, ptr %111, !tbaa !2
  %112 = getelementptr inbounds nuw i8, ptr %3, i16 216
  store i32 1664, ptr %112, !tbaa !2
  %113 = getelementptr inbounds nuw i8, ptr %2, i16 216
  store i32 512, ptr %113, !tbaa !2
  %114 = getelementptr inbounds nuw i8, ptr %3, i16 220
  store i32 1728, ptr %114, !tbaa !2
  %115 = getelementptr inbounds nuw i8, ptr %2, i16 220
  store i32 128, ptr %115, !tbaa !2
  %116 = getelementptr inbounds nuw i8, ptr %3, i16 224
  store i32 1472, ptr %116, !tbaa !2
  %117 = getelementptr inbounds nuw i8, ptr %2, i16 224
  store i32 128, ptr %117, !tbaa !2
  %118 = getelementptr inbounds nuw i8, ptr %3, i16 228
  store i32 1536, ptr %118, !tbaa !2
  %119 = getelementptr inbounds nuw i8, ptr %2, i16 228
  store i32 256, ptr %119, !tbaa !2
  %120 = getelementptr inbounds nuw i8, ptr %3, i16 232
  store i32 1600, ptr %120, !tbaa !2
  %121 = getelementptr inbounds nuw i8, ptr %2, i16 232
  store i32 0, ptr %121, !tbaa !2
  %122 = getelementptr inbounds nuw i8, ptr %3, i16 236
  store i32 1664, ptr %122, !tbaa !2
  %123 = getelementptr inbounds nuw i8, ptr %2, i16 236
  store i32 128, ptr %123, !tbaa !2
  %124 = getelementptr inbounds nuw i8, ptr %3, i16 240
  store i32 1728, ptr %124, !tbaa !2
  %125 = getelementptr inbounds nuw i8, ptr %2, i16 240
  store i32 256, ptr %125, !tbaa !2
  %126 = getelementptr inbounds nuw i8, ptr %3, i16 244
  store i32 1792, ptr %126, !tbaa !2
  %127 = getelementptr inbounds nuw i8, ptr %2, i16 244
  store i32 0, ptr %127, !tbaa !2
  %128 = getelementptr inbounds nuw i8, ptr %3, i16 248
  store i32 1856, ptr %128, !tbaa !2
  %129 = getelementptr inbounds nuw i8, ptr %2, i16 248
  store i32 128, ptr %129, !tbaa !2
  %130 = getelementptr inbounds nuw i8, ptr %3, i16 252
  store i32 1920, ptr %130, !tbaa !2
  %131 = getelementptr inbounds nuw i8, ptr %2, i16 252
  store i32 512, ptr %131, !tbaa !2
  br label %132

132:
  %133 = phi ptr [ %205, %203 ], [ %1, %0 ]
  %134 = phi i16 [ 0, %0 ], [ %204, %203 ]
  %135 = shl nuw nsw i16 %134, 5
  %136 = getelementptr inbounds nuw i8, ptr %3, i16 %135
  %137 = load i32, ptr %136, !tbaa !2
  %138 = getelementptr i8, ptr %3, i16 %135
  %139 = getelementptr i8, ptr %138, i16 4
  %140 = load i32, ptr %139, !tbaa !2
  %141 = getelementptr i8, ptr %3, i16 %135
  %142 = getelementptr i8, ptr %141, i16 8
  %143 = load i32, ptr %142, !tbaa !2
  %144 = getelementptr i8, ptr %3, i16 %135
  %145 = getelementptr i8, ptr %144, i16 12
  %146 = load i32, ptr %145, !tbaa !2
  %147 = getelementptr i8, ptr %3, i16 %135
  %148 = getelementptr i8, ptr %147, i16 16
  %149 = load i32, ptr %148, !tbaa !2
  %150 = getelementptr i8, ptr %3, i16 %135
  %151 = getelementptr i8, ptr %150, i16 20
  %152 = load i32, ptr %151, !tbaa !2
  %153 = getelementptr i8, ptr %3, i16 %135
  %154 = getelementptr i8, ptr %153, i16 24
  %155 = load i32, ptr %154, !tbaa !2
  %156 = getelementptr i8, ptr %3, i16 %135
  %157 = getelementptr i8, ptr %156, i16 28
  %158 = load i32, ptr %157, !tbaa !2
  br label %159

159:
  %160 = phi i16 [ %202, %159 ], [ -32, %132 ]
  %161 = getelementptr i8, ptr %2, i16 %160
  %162 = getelementptr i8, ptr %161, i16 32
  %163 = load i32, ptr %162, !tbaa !2
  %164 = tail call i32 @llvm.smul.fix.i32(i32 %137, i32 %163, i32 8)
  %165 = getelementptr i8, ptr %2, i16 %160
  %166 = getelementptr i8, ptr %165, i16 64
  %167 = load i32, ptr %166, !tbaa !2
  %168 = tail call i32 @llvm.smul.fix.i32(i32 %140, i32 %167, i32 8)
  %169 = add i32 %168, %164
  %170 = getelementptr i8, ptr %2, i16 %160
  %171 = getelementptr i8, ptr %170, i16 96
  %172 = load i32, ptr %171, !tbaa !2
  %173 = tail call i32 @llvm.smul.fix.i32(i32 %143, i32 %172, i32 8)
  %174 = add i32 %169, %173
  %175 = getelementptr i8, ptr %2, i16 %160
  %176 = getelementptr i8, ptr %175, i16 128
  %177 = load i32, ptr %176, !tbaa !2
  %178 = tail call i32 @llvm.smul.fix.i32(i32 %146, i32 %177, i32 8)
  %179 = add i32 %174, %178
  %180 = getelementptr i8, ptr %2, i16 %160
  %181 = getelementptr i8, ptr %180, i16 160
  %182 = load i32, ptr %181, !tbaa !2
  %183 = tail call i32 @llvm.smul.fix.i32(i32 %149, i32 %182, i32 8)
  %184 = add i32 %179, %183
  %185 = getelementptr i8, ptr %2, i16 %160
  %186 = getelementptr i8, ptr %185, i16 192
  %187 = load i32, ptr %186, !tbaa !2
  %188 = tail call i32 @llvm.smul.fix.i32(i32 %152, i32 %187, i32 8)
  %189 = add i32 %184, %188
  %190 = getelementptr i8, ptr %2, i16 %160
  %191 = getelementptr i8, ptr %190, i16 224
  %192 = load i32, ptr %191, !tbaa !2
  %193 = tail call i32 @llvm.smul.fix.i32(i32 %155, i32 %192, i32 8)
  %194 = add i32 %189, %193
  %195 = getelementptr i8, ptr %2, i16 %160
  %196 = getelementptr i8, ptr %195, i16 256
  %197 = load i32, ptr %196, !tbaa !2
  %198 = tail call i32 @llvm.smul.fix.i32(i32 %158, i32 %197, i32 8)
  %199 = add i32 %194, %198
  %200 = getelementptr i8, ptr %133, i16 %160
  %201 = getelementptr i8, ptr %200, i16 32
  store i32 %199, ptr %201, !tbaa !2
  %202 = add nsw i16 %160, 4
  %exitcond3 = icmp eq i16 %202, 0
  br i1 %exitcond3, label %203, label %159

203:
  %204 = add nuw nsw i16 %134, 1
  %exitcond4 = icmp eq i16 %204, 8
  %205 = getelementptr i8, ptr %133, i16 32
  br i1 %exitcond4, label %206, label %132

206:
  %207 = getelementptr i8, ptr %1, i16 28
  br label %208

208:
  %209 = phi ptr [ %207, %206 ], [ %251, %208 ]
  %210 = phi i32 [ 2048, %206 ], [ %250, %208 ]
  %211 = phi i32 [ %249, %208 ], [ 0, %206 ]
  %212 = getelementptr i8, ptr %209, i16 -28
  %213 = load i32, ptr %212, !tbaa !2
  %214 = add i32 %210, -1792
  %215 = tail call i32 @llvm.smul.fix.i32(i32 %213, i32 %214, i32 8)
  %216 = add i32 %215, %211
  %217 = getelementptr i8, ptr %209, i16 -24
  %218 = load i32, ptr %217, !tbaa !2
  %219 = add i32 %210, -1536
  %220 = tail call i32 @llvm.smul.fix.i32(i32 %218, i32 %219, i32 8)
  %221 = add i32 %216, %220
  %222 = getelementptr i8, ptr %209, i16 -20
  %223 = load i32, ptr %222, !tbaa !2
  %224 = add i32 %210, -1280
  %225 = tail call i32 @llvm.smul.fix.i32(i32 %223, i32 %224, i32 8)
  %226 = add i32 %221, %225
  %227 = getelementptr i8, ptr %209, i16 -16
  %228 = load i32, ptr %227, !tbaa !2
  %229 = add i32 %210, -1024
  %230 = tail call i32 @llvm.smul.fix.i32(i32 %228, i32 %229, i32 8)
  %231 = add i32 %226, %230
  %232 = getelementptr i8, ptr %209, i16 -12
  %233 = load i32, ptr %232, !tbaa !2
  %234 = add i32 %210, -768
  %235 = tail call i32 @llvm.smul.fix.i32(i32 %233, i32 %234, i32 8)
  %236 = add i32 %231, %235
  %237 = getelementptr i8, ptr %209, i16 -8
  %238 = load i32, ptr %237, !tbaa !2
  %239 = add i32 %210, -512
  %240 = tail call i32 @llvm.smul.fix.i32(i32 %238, i32 %239, i32 8)
  %241 = add i32 %236, %240
  %242 = getelementptr i8, ptr %209, i16 -4
  %243 = load i32, ptr %242, !tbaa !2
  %244 = add i32 %210, -256
  %245 = tail call i32 @llvm.smul.fix.i32(i32 %243, i32 %244, i32 8)
  %246 = add i32 %241, %245
  %247 = load i32, ptr %209, !tbaa !2
  %248 = tail call i32 @llvm.smul.fix.i32(i32 %247, i32 %210, i32 8)
  %249 = add i32 %246, %248
  %250 = add nuw nsw i32 %210, 2048
  %tmp = trunc i32 %250 to i16
  %251 = getelementptr i8, ptr %209, i16 32
  %exitcond1 = icmp eq i16 %tmp, 18432
  br i1 %exitcond1, label %252, label %208

252:
  call void @llvm.lifetime.end.p0(i64 256, ptr nonnull %1)
  call void @llvm.lifetime.end.p0(i64 256, ptr nonnull %2)
  call void @llvm.lifetime.end.p0(i64 256, ptr nonnull %3)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6))
  tail call cc 1005 addrspace(1) void @N$PQ4(i32 %249, i8 zeroext 8)
  tail call cc 1005 addrspace(1) void @N$PN()
  ret i16 0
}

declare i32 @llvm.sdiv.fix.i32(i32, i32, i32 immarg) nocallback nofree nosync nounwind willreturn memory(none)

declare cc 1005 void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

declare i32 @llvm.smul.fix.i32(i32, i32, i32 immarg) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PQ4(i32, i8) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
