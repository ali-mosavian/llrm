target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [15 x i8] c"\08\00\08\00\08\00matmul: \00"
@$str2 = internal constant [19 x i8] c"\08\00\0C\00\0C\00matmul: bad \00"

define cc 1005 range(i16 0, 2) i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
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
  store i32 2, ptr %3, !tbaa !2
  store i32 2, ptr %2, !tbaa !2
  %6 = getelementptr inbounds nuw i8, ptr %3, i16 4
  store i32 3, ptr %6, !tbaa !2
  %7 = getelementptr inbounds nuw i8, ptr %2, i16 4
  store i32 1, ptr %7, !tbaa !2
  %8 = getelementptr inbounds nuw i8, ptr %3, i16 8
  store i32 4, ptr %8, !tbaa !2
  %9 = getelementptr inbounds nuw i8, ptr %2, i16 8
  store i32 2, ptr %9, !tbaa !2
  %10 = getelementptr inbounds nuw i8, ptr %3, i16 12
  store i32 5, ptr %10, !tbaa !2
  %11 = getelementptr inbounds nuw i8, ptr %2, i16 12
  store i32 0, ptr %11, !tbaa !2
  %12 = getelementptr inbounds nuw i8, ptr %3, i16 16
  store i32 6, ptr %12, !tbaa !2
  %13 = getelementptr inbounds nuw i8, ptr %2, i16 16
  store i32 1, ptr %13, !tbaa !2
  %14 = getelementptr inbounds nuw i8, ptr %3, i16 20
  store i32 7, ptr %14, !tbaa !2
  %15 = getelementptr inbounds nuw i8, ptr %2, i16 20
  store i32 2, ptr %15, !tbaa !2
  %16 = getelementptr inbounds nuw i8, ptr %3, i16 24
  store i32 8, ptr %16, !tbaa !2
  %17 = getelementptr inbounds nuw i8, ptr %2, i16 24
  store i32 0, ptr %17, !tbaa !2
  %18 = getelementptr inbounds nuw i8, ptr %3, i16 28
  store i32 9, ptr %18, !tbaa !2
  %19 = getelementptr inbounds nuw i8, ptr %2, i16 28
  store i32 1, ptr %19, !tbaa !2
  %20 = getelementptr inbounds nuw i8, ptr %3, i16 32
  store i32 5, ptr %20, !tbaa !2
  %21 = getelementptr inbounds nuw i8, ptr %2, i16 32
  store i32 1, ptr %21, !tbaa !2
  %22 = getelementptr inbounds nuw i8, ptr %3, i16 36
  store i32 6, ptr %22, !tbaa !2
  %23 = getelementptr inbounds nuw i8, ptr %2, i16 36
  store i32 2, ptr %23, !tbaa !2
  %24 = getelementptr inbounds nuw i8, ptr %3, i16 40
  store i32 7, ptr %24, !tbaa !2
  %25 = getelementptr inbounds nuw i8, ptr %2, i16 40
  store i32 0, ptr %25, !tbaa !2
  %26 = getelementptr inbounds nuw i8, ptr %3, i16 44
  store i32 8, ptr %26, !tbaa !2
  %27 = getelementptr inbounds nuw i8, ptr %2, i16 44
  store i32 1, ptr %27, !tbaa !2
  %28 = getelementptr inbounds nuw i8, ptr %3, i16 48
  store i32 9, ptr %28, !tbaa !2
  %29 = getelementptr inbounds nuw i8, ptr %2, i16 48
  store i32 2, ptr %29, !tbaa !2
  %30 = getelementptr inbounds nuw i8, ptr %3, i16 52
  store i32 10, ptr %30, !tbaa !2
  %31 = getelementptr inbounds nuw i8, ptr %2, i16 52
  store i32 0, ptr %31, !tbaa !2
  %32 = getelementptr inbounds nuw i8, ptr %3, i16 56
  store i32 11, ptr %32, !tbaa !2
  %33 = getelementptr inbounds nuw i8, ptr %2, i16 56
  store i32 1, ptr %33, !tbaa !2
  %34 = getelementptr inbounds nuw i8, ptr %3, i16 60
  store i32 12, ptr %34, !tbaa !2
  %35 = getelementptr inbounds nuw i8, ptr %2, i16 60
  store i32 2, ptr %35, !tbaa !2
  %36 = getelementptr inbounds nuw i8, ptr %3, i16 64
  store i32 8, ptr %36, !tbaa !2
  %37 = getelementptr inbounds nuw i8, ptr %2, i16 64
  store i32 2, ptr %37, !tbaa !2
  %38 = getelementptr inbounds nuw i8, ptr %3, i16 68
  store i32 9, ptr %38, !tbaa !2
  %39 = getelementptr inbounds nuw i8, ptr %2, i16 68
  store i32 0, ptr %39, !tbaa !2
  %40 = getelementptr inbounds nuw i8, ptr %3, i16 72
  store i32 10, ptr %40, !tbaa !2
  %41 = getelementptr inbounds nuw i8, ptr %2, i16 72
  store i32 2, ptr %41, !tbaa !2
  %42 = getelementptr inbounds nuw i8, ptr %3, i16 76
  store i32 11, ptr %42, !tbaa !2
  %43 = getelementptr inbounds nuw i8, ptr %2, i16 76
  store i32 2, ptr %43, !tbaa !2
  %44 = getelementptr inbounds nuw i8, ptr %3, i16 80
  store i32 12, ptr %44, !tbaa !2
  %45 = getelementptr inbounds nuw i8, ptr %2, i16 80
  store i32 0, ptr %45, !tbaa !2
  %46 = getelementptr inbounds nuw i8, ptr %3, i16 84
  store i32 13, ptr %46, !tbaa !2
  %47 = getelementptr inbounds nuw i8, ptr %2, i16 84
  store i32 1, ptr %47, !tbaa !2
  %48 = getelementptr inbounds nuw i8, ptr %3, i16 88
  store i32 14, ptr %48, !tbaa !2
  %49 = getelementptr inbounds nuw i8, ptr %2, i16 88
  store i32 2, ptr %49, !tbaa !2
  %50 = getelementptr inbounds nuw i8, ptr %3, i16 92
  store i32 15, ptr %50, !tbaa !2
  %51 = getelementptr inbounds nuw i8, ptr %2, i16 92
  store i32 0, ptr %51, !tbaa !2
  %52 = getelementptr inbounds nuw i8, ptr %3, i16 96
  store i32 11, ptr %52, !tbaa !2
  %53 = getelementptr inbounds nuw i8, ptr %2, i16 96
  store i32 0, ptr %53, !tbaa !2
  %54 = getelementptr inbounds nuw i8, ptr %3, i16 100
  store i32 12, ptr %54, !tbaa !2
  %55 = getelementptr inbounds nuw i8, ptr %2, i16 100
  store i32 1, ptr %55, !tbaa !2
  %56 = getelementptr inbounds nuw i8, ptr %3, i16 104
  store i32 13, ptr %56, !tbaa !2
  %57 = getelementptr inbounds nuw i8, ptr %2, i16 104
  store i32 2, ptr %57, !tbaa !2
  %58 = getelementptr inbounds nuw i8, ptr %3, i16 108
  store i32 14, ptr %58, !tbaa !2
  %59 = getelementptr inbounds nuw i8, ptr %2, i16 108
  store i32 2, ptr %59, !tbaa !2
  %60 = getelementptr inbounds nuw i8, ptr %3, i16 112
  store i32 15, ptr %60, !tbaa !2
  %61 = getelementptr inbounds nuw i8, ptr %2, i16 112
  store i32 1, ptr %61, !tbaa !2
  %62 = getelementptr inbounds nuw i8, ptr %3, i16 116
  store i32 16, ptr %62, !tbaa !2
  %63 = getelementptr inbounds nuw i8, ptr %2, i16 116
  store i32 2, ptr %63, !tbaa !2
  %64 = getelementptr inbounds nuw i8, ptr %3, i16 120
  store i32 17, ptr %64, !tbaa !2
  %65 = getelementptr inbounds nuw i8, ptr %2, i16 120
  store i32 0, ptr %65, !tbaa !2
  %66 = getelementptr inbounds nuw i8, ptr %3, i16 124
  store i32 18, ptr %66, !tbaa !2
  %67 = getelementptr inbounds nuw i8, ptr %2, i16 124
  store i32 1, ptr %67, !tbaa !2
  %68 = getelementptr inbounds nuw i8, ptr %3, i16 128
  store i32 14, ptr %68, !tbaa !2
  %69 = getelementptr inbounds nuw i8, ptr %2, i16 128
  store i32 1, ptr %69, !tbaa !2
  %70 = getelementptr inbounds nuw i8, ptr %3, i16 132
  store i32 15, ptr %70, !tbaa !2
  %71 = getelementptr inbounds nuw i8, ptr %2, i16 132
  store i32 2, ptr %71, !tbaa !2
  %72 = getelementptr inbounds nuw i8, ptr %3, i16 136
  store i32 16, ptr %72, !tbaa !2
  %73 = getelementptr inbounds nuw i8, ptr %2, i16 136
  store i32 0, ptr %73, !tbaa !2
  %74 = getelementptr inbounds nuw i8, ptr %3, i16 140
  store i32 17, ptr %74, !tbaa !2
  %75 = getelementptr inbounds nuw i8, ptr %2, i16 140
  store i32 1, ptr %75, !tbaa !2
  %76 = getelementptr inbounds nuw i8, ptr %3, i16 144
  store i32 18, ptr %76, !tbaa !2
  %77 = getelementptr inbounds nuw i8, ptr %2, i16 144
  store i32 2, ptr %77, !tbaa !2
  %78 = getelementptr inbounds nuw i8, ptr %3, i16 148
  store i32 19, ptr %78, !tbaa !2
  %79 = getelementptr inbounds nuw i8, ptr %2, i16 148
  store i32 0, ptr %79, !tbaa !2
  %80 = getelementptr inbounds nuw i8, ptr %3, i16 152
  store i32 20, ptr %80, !tbaa !2
  %81 = getelementptr inbounds nuw i8, ptr %2, i16 152
  store i32 1, ptr %81, !tbaa !2
  %82 = getelementptr inbounds nuw i8, ptr %3, i16 156
  store i32 21, ptr %82, !tbaa !2
  %83 = getelementptr inbounds nuw i8, ptr %2, i16 156
  store i32 2, ptr %83, !tbaa !2
  %84 = getelementptr inbounds nuw i8, ptr %3, i16 160
  store i32 17, ptr %84, !tbaa !2
  %85 = getelementptr inbounds nuw i8, ptr %2, i16 160
  store i32 2, ptr %85, !tbaa !2
  %86 = getelementptr inbounds nuw i8, ptr %3, i16 164
  store i32 18, ptr %86, !tbaa !2
  %87 = getelementptr inbounds nuw i8, ptr %2, i16 164
  store i32 0, ptr %87, !tbaa !2
  %88 = getelementptr inbounds nuw i8, ptr %3, i16 168
  store i32 19, ptr %88, !tbaa !2
  %89 = getelementptr inbounds nuw i8, ptr %2, i16 168
  store i32 1, ptr %89, !tbaa !2
  %90 = getelementptr inbounds nuw i8, ptr %3, i16 172
  store i32 20, ptr %90, !tbaa !2
  %91 = getelementptr inbounds nuw i8, ptr %2, i16 172
  store i32 2, ptr %91, !tbaa !2
  %92 = getelementptr inbounds nuw i8, ptr %3, i16 176
  store i32 21, ptr %92, !tbaa !2
  %93 = getelementptr inbounds nuw i8, ptr %2, i16 176
  store i32 0, ptr %93, !tbaa !2
  %94 = getelementptr inbounds nuw i8, ptr %3, i16 180
  store i32 22, ptr %94, !tbaa !2
  %95 = getelementptr inbounds nuw i8, ptr %2, i16 180
  store i32 2, ptr %95, !tbaa !2
  %96 = getelementptr inbounds nuw i8, ptr %3, i16 184
  store i32 23, ptr %96, !tbaa !2
  %97 = getelementptr inbounds nuw i8, ptr %2, i16 184
  store i32 2, ptr %97, !tbaa !2
  %98 = getelementptr inbounds nuw i8, ptr %3, i16 188
  store i32 24, ptr %98, !tbaa !2
  %99 = getelementptr inbounds nuw i8, ptr %2, i16 188
  store i32 0, ptr %99, !tbaa !2
  %100 = getelementptr inbounds nuw i8, ptr %3, i16 192
  store i32 20, ptr %100, !tbaa !2
  %101 = getelementptr inbounds nuw i8, ptr %2, i16 192
  store i32 0, ptr %101, !tbaa !2
  %102 = getelementptr inbounds nuw i8, ptr %3, i16 196
  store i32 21, ptr %102, !tbaa !2
  %103 = getelementptr inbounds nuw i8, ptr %2, i16 196
  store i32 1, ptr %103, !tbaa !2
  %104 = getelementptr inbounds nuw i8, ptr %3, i16 200
  store i32 22, ptr %104, !tbaa !2
  %105 = getelementptr inbounds nuw i8, ptr %2, i16 200
  store i32 2, ptr %105, !tbaa !2
  %106 = getelementptr inbounds nuw i8, ptr %3, i16 204
  store i32 23, ptr %106, !tbaa !2
  %107 = getelementptr inbounds nuw i8, ptr %2, i16 204
  store i32 0, ptr %107, !tbaa !2
  %108 = getelementptr inbounds nuw i8, ptr %3, i16 208
  store i32 24, ptr %108, !tbaa !2
  %109 = getelementptr inbounds nuw i8, ptr %2, i16 208
  store i32 1, ptr %109, !tbaa !2
  %110 = getelementptr inbounds nuw i8, ptr %3, i16 212
  store i32 25, ptr %110, !tbaa !2
  %111 = getelementptr inbounds nuw i8, ptr %2, i16 212
  store i32 2, ptr %111, !tbaa !2
  %112 = getelementptr inbounds nuw i8, ptr %3, i16 216
  store i32 26, ptr %112, !tbaa !2
  %113 = getelementptr inbounds nuw i8, ptr %2, i16 216
  store i32 2, ptr %113, !tbaa !2
  %114 = getelementptr inbounds nuw i8, ptr %3, i16 220
  store i32 27, ptr %114, !tbaa !2
  %115 = getelementptr inbounds nuw i8, ptr %2, i16 220
  store i32 1, ptr %115, !tbaa !2
  %116 = getelementptr inbounds nuw i8, ptr %3, i16 224
  store i32 23, ptr %116, !tbaa !2
  %117 = getelementptr inbounds nuw i8, ptr %2, i16 224
  store i32 1, ptr %117, !tbaa !2
  %118 = getelementptr inbounds nuw i8, ptr %3, i16 228
  store i32 24, ptr %118, !tbaa !2
  %119 = getelementptr inbounds nuw i8, ptr %2, i16 228
  store i32 2, ptr %119, !tbaa !2
  %120 = getelementptr inbounds nuw i8, ptr %3, i16 232
  store i32 25, ptr %120, !tbaa !2
  %121 = getelementptr inbounds nuw i8, ptr %2, i16 232
  store i32 0, ptr %121, !tbaa !2
  %122 = getelementptr inbounds nuw i8, ptr %3, i16 236
  store i32 26, ptr %122, !tbaa !2
  %123 = getelementptr inbounds nuw i8, ptr %2, i16 236
  store i32 1, ptr %123, !tbaa !2
  %124 = getelementptr inbounds nuw i8, ptr %3, i16 240
  store i32 27, ptr %124, !tbaa !2
  %125 = getelementptr inbounds nuw i8, ptr %2, i16 240
  store i32 2, ptr %125, !tbaa !2
  %126 = getelementptr inbounds nuw i8, ptr %3, i16 244
  store i32 28, ptr %126, !tbaa !2
  %127 = getelementptr inbounds nuw i8, ptr %2, i16 244
  store i32 0, ptr %127, !tbaa !2
  %128 = getelementptr inbounds nuw i8, ptr %3, i16 248
  store i32 29, ptr %128, !tbaa !2
  %129 = getelementptr inbounds nuw i8, ptr %2, i16 248
  store i32 1, ptr %129, !tbaa !2
  %130 = getelementptr inbounds nuw i8, ptr %3, i16 252
  store i32 30, ptr %130, !tbaa !2
  %131 = getelementptr inbounds nuw i8, ptr %2, i16 252
  store i32 2, ptr %131, !tbaa !2
  br label %132

132:
  %133 = phi ptr [ %197, %195 ], [ %1, %0 ]
  %134 = phi i32 [ 0, %0 ], [ %196, %195 ]
  %135 = shl nuw nsw i32 %134, 3
  %.phi.trans.insert = trunc i32 %135 to i16
  %.phi.trans.insert1 = getelementptr inbounds nuw [4 x i8], ptr %3, i16 %.phi.trans.insert
  %136 = load i32, ptr %.phi.trans.insert1, !tbaa !2
  %137 = getelementptr inbounds nuw i8, ptr %.phi.trans.insert1, i16 4
  %138 = load i32, ptr %137, !tbaa !2
  %139 = getelementptr inbounds nuw i8, ptr %.phi.trans.insert1, i16 8
  %140 = load i32, ptr %139, !tbaa !2
  %141 = getelementptr inbounds nuw i8, ptr %.phi.trans.insert1, i16 12
  %142 = load i32, ptr %141, !tbaa !2
  %143 = getelementptr inbounds nuw i8, ptr %.phi.trans.insert1, i16 16
  %144 = load i32, ptr %143, !tbaa !2
  %145 = getelementptr inbounds nuw i8, ptr %.phi.trans.insert1, i16 20
  %146 = load i32, ptr %145, !tbaa !2
  %147 = getelementptr inbounds nuw i8, ptr %.phi.trans.insert1, i16 24
  %148 = load i32, ptr %147, !tbaa !2
  %149 = getelementptr inbounds nuw i8, ptr %.phi.trans.insert1, i16 28
  %150 = load i32, ptr %149, !tbaa !2
  br label %151

151:
  %152 = phi i16 [ %194, %151 ], [ 0, %132 ]
  %153 = phi i32 [ %193, %151 ], [ 8, %132 ]
  %154 = getelementptr i8, ptr %2, i16 %152
  %155 = load i32, ptr %154, !tbaa !2
  %156 = mul i32 %155, %136
  %157 = getelementptr i8, ptr %2, i16 %152
  %158 = getelementptr i8, ptr %157, i16 32
  %159 = load i32, ptr %158, !tbaa !2
  %160 = mul i32 %159, %138
  %161 = add i32 %160, %156
  %162 = getelementptr i8, ptr %2, i16 %152
  %163 = getelementptr i8, ptr %162, i16 64
  %164 = load i32, ptr %163, !tbaa !2
  %165 = mul i32 %164, %140
  %166 = add i32 %161, %165
  %167 = getelementptr i8, ptr %2, i16 %152
  %168 = getelementptr i8, ptr %167, i16 96
  %169 = load i32, ptr %168, !tbaa !2
  %170 = mul i32 %169, %142
  %171 = add i32 %166, %170
  %172 = getelementptr i8, ptr %2, i16 %152
  %173 = getelementptr i8, ptr %172, i16 128
  %174 = load i32, ptr %173, !tbaa !2
  %175 = mul i32 %174, %144
  %176 = add i32 %171, %175
  %177 = getelementptr i8, ptr %2, i16 %152
  %178 = getelementptr i8, ptr %177, i16 160
  %179 = load i32, ptr %178, !tbaa !2
  %180 = mul i32 %179, %146
  %181 = add i32 %176, %180
  %182 = getelementptr i8, ptr %2, i16 %152
  %183 = getelementptr i8, ptr %182, i16 192
  %184 = load i32, ptr %183, !tbaa !2
  %185 = mul i32 %184, %148
  %186 = add i32 %181, %185
  %187 = getelementptr i8, ptr %2, i16 %152
  %188 = getelementptr i8, ptr %187, i16 224
  %189 = load i32, ptr %188, !tbaa !2
  %190 = mul i32 %189, %150
  %191 = add i32 %186, %190
  %192 = getelementptr i8, ptr %133, i16 %152
  store i32 %191, ptr %192, !tbaa !2
  %193 = add nsw i32 %153, -1
  %194 = add nuw nsw i16 %152, 4
  %exitcond3 = icmp eq i32 %193, 0
  br i1 %exitcond3, label %195, label %151

195:
  %196 = add nuw nsw i32 %134, 1
  %exitcond4 = icmp eq i32 %196, 8
  %197 = getelementptr i8, ptr %133, i16 32
  br i1 %exitcond4, label %198, label %132

198:
  %199 = load i32, ptr %1, !tbaa !2
  %200 = getelementptr inbounds nuw i8, ptr %1, i16 4
  %201 = load i32, ptr %200, !tbaa !2
  %202 = shl i32 %201, 1
  %203 = add i32 %202, %199
  %204 = getelementptr inbounds nuw i8, ptr %1, i16 8
  %205 = load i32, ptr %204, !tbaa !2
  %206 = mul i32 %205, 3
  %207 = add i32 %203, %206
  %208 = getelementptr inbounds nuw i8, ptr %1, i16 12
  %209 = load i32, ptr %208, !tbaa !2
  %210 = shl i32 %209, 2
  %211 = add i32 %207, %210
  %212 = getelementptr inbounds nuw i8, ptr %1, i16 16
  %213 = load i32, ptr %212, !tbaa !2
  %214 = mul i32 %213, 5
  %215 = add i32 %211, %214
  %216 = getelementptr inbounds nuw i8, ptr %1, i16 20
  %217 = load i32, ptr %216, !tbaa !2
  %218 = mul i32 %217, 6
  %219 = add i32 %215, %218
  %220 = getelementptr inbounds nuw i8, ptr %1, i16 24
  %221 = load i32, ptr %220, !tbaa !2
  %222 = mul i32 %221, 7
  %223 = add i32 %219, %222
  %224 = getelementptr inbounds nuw i8, ptr %1, i16 28
  %225 = load i32, ptr %224, !tbaa !2
  %226 = shl i32 %225, 3
  %227 = add i32 %223, %226
  %228 = getelementptr inbounds nuw i8, ptr %1, i16 32
  %229 = load i32, ptr %228, !tbaa !2
  %230 = mul i32 %229, 9
  %231 = add i32 %227, %230
  %232 = getelementptr inbounds nuw i8, ptr %1, i16 36
  %233 = load i32, ptr %232, !tbaa !2
  %234 = mul i32 %233, 10
  %235 = add i32 %231, %234
  %236 = getelementptr inbounds nuw i8, ptr %1, i16 40
  %237 = load i32, ptr %236, !tbaa !2
  %238 = mul i32 %237, 11
  %239 = add i32 %235, %238
  %240 = getelementptr inbounds nuw i8, ptr %1, i16 44
  %241 = load i32, ptr %240, !tbaa !2
  %242 = mul i32 %241, 12
  %243 = add i32 %239, %242
  %244 = getelementptr inbounds nuw i8, ptr %1, i16 48
  %245 = load i32, ptr %244, !tbaa !2
  %246 = mul i32 %245, 13
  %247 = add i32 %243, %246
  %248 = getelementptr inbounds nuw i8, ptr %1, i16 52
  %249 = load i32, ptr %248, !tbaa !2
  %250 = mul i32 %249, 14
  %251 = add i32 %247, %250
  %252 = getelementptr inbounds nuw i8, ptr %1, i16 56
  %253 = load i32, ptr %252, !tbaa !2
  %254 = mul i32 %253, 15
  %255 = add i32 %251, %254
  %256 = getelementptr inbounds nuw i8, ptr %1, i16 60
  %257 = load i32, ptr %256, !tbaa !2
  %258 = shl i32 %257, 4
  %259 = add i32 %255, %258
  %260 = getelementptr inbounds nuw i8, ptr %1, i16 64
  %261 = load i32, ptr %260, !tbaa !2
  %262 = mul i32 %261, 17
  %263 = add i32 %259, %262
  %264 = getelementptr inbounds nuw i8, ptr %1, i16 68
  %265 = load i32, ptr %264, !tbaa !2
  %266 = mul i32 %265, 18
  %267 = add i32 %263, %266
  %268 = getelementptr inbounds nuw i8, ptr %1, i16 72
  %269 = load i32, ptr %268, !tbaa !2
  %270 = mul i32 %269, 19
  %271 = add i32 %267, %270
  %272 = getelementptr inbounds nuw i8, ptr %1, i16 76
  %273 = load i32, ptr %272, !tbaa !2
  %274 = mul i32 %273, 20
  %275 = add i32 %271, %274
  %276 = getelementptr inbounds nuw i8, ptr %1, i16 80
  %277 = load i32, ptr %276, !tbaa !2
  %278 = mul i32 %277, 21
  %279 = add i32 %275, %278
  %280 = getelementptr inbounds nuw i8, ptr %1, i16 84
  %281 = load i32, ptr %280, !tbaa !2
  %282 = mul i32 %281, 22
  %283 = add i32 %279, %282
  %284 = getelementptr inbounds nuw i8, ptr %1, i16 88
  %285 = load i32, ptr %284, !tbaa !2
  %286 = mul i32 %285, 23
  %287 = add i32 %283, %286
  %288 = getelementptr inbounds nuw i8, ptr %1, i16 92
  %289 = load i32, ptr %288, !tbaa !2
  %290 = mul i32 %289, 24
  %291 = add i32 %287, %290
  %292 = getelementptr inbounds nuw i8, ptr %1, i16 96
  %293 = load i32, ptr %292, !tbaa !2
  %294 = mul i32 %293, 25
  %295 = add i32 %291, %294
  %296 = getelementptr inbounds nuw i8, ptr %1, i16 100
  %297 = load i32, ptr %296, !tbaa !2
  %298 = mul i32 %297, 26
  %299 = add i32 %295, %298
  %300 = getelementptr inbounds nuw i8, ptr %1, i16 104
  %301 = load i32, ptr %300, !tbaa !2
  %302 = mul i32 %301, 27
  %303 = add i32 %299, %302
  %304 = getelementptr inbounds nuw i8, ptr %1, i16 108
  %305 = load i32, ptr %304, !tbaa !2
  %306 = mul i32 %305, 28
  %307 = add i32 %303, %306
  %308 = getelementptr inbounds nuw i8, ptr %1, i16 112
  %309 = load i32, ptr %308, !tbaa !2
  %310 = mul i32 %309, 29
  %311 = add i32 %307, %310
  %312 = getelementptr inbounds nuw i8, ptr %1, i16 116
  %313 = load i32, ptr %312, !tbaa !2
  %314 = mul i32 %313, 30
  %315 = add i32 %311, %314
  %316 = getelementptr inbounds nuw i8, ptr %1, i16 120
  %317 = load i32, ptr %316, !tbaa !2
  %318 = mul i32 %317, 31
  %319 = add i32 %315, %318
  %320 = getelementptr inbounds nuw i8, ptr %1, i16 124
  %321 = load i32, ptr %320, !tbaa !2
  %322 = shl i32 %321, 5
  %323 = add i32 %319, %322
  %324 = getelementptr inbounds nuw i8, ptr %1, i16 128
  %325 = load i32, ptr %324, !tbaa !2
  %326 = mul i32 %325, 33
  %327 = add i32 %323, %326
  %328 = getelementptr inbounds nuw i8, ptr %1, i16 132
  %329 = load i32, ptr %328, !tbaa !2
  %330 = mul i32 %329, 34
  %331 = add i32 %327, %330
  %332 = getelementptr inbounds nuw i8, ptr %1, i16 136
  %333 = load i32, ptr %332, !tbaa !2
  %334 = mul i32 %333, 35
  %335 = add i32 %331, %334
  %336 = getelementptr inbounds nuw i8, ptr %1, i16 140
  %337 = load i32, ptr %336, !tbaa !2
  %338 = mul i32 %337, 36
  %339 = add i32 %335, %338
  %340 = getelementptr inbounds nuw i8, ptr %1, i16 144
  %341 = load i32, ptr %340, !tbaa !2
  %342 = mul i32 %341, 37
  %343 = add i32 %339, %342
  %344 = getelementptr inbounds nuw i8, ptr %1, i16 148
  %345 = load i32, ptr %344, !tbaa !2
  %346 = mul i32 %345, 38
  %347 = add i32 %343, %346
  %348 = getelementptr inbounds nuw i8, ptr %1, i16 152
  %349 = load i32, ptr %348, !tbaa !2
  %350 = mul i32 %349, 39
  %351 = add i32 %347, %350
  %352 = getelementptr inbounds nuw i8, ptr %1, i16 156
  %353 = load i32, ptr %352, !tbaa !2
  %354 = mul i32 %353, 40
  %355 = add i32 %351, %354
  %356 = getelementptr inbounds nuw i8, ptr %1, i16 160
  %357 = load i32, ptr %356, !tbaa !2
  %358 = mul i32 %357, 41
  %359 = add i32 %355, %358
  %360 = getelementptr inbounds nuw i8, ptr %1, i16 164
  %361 = load i32, ptr %360, !tbaa !2
  %362 = mul i32 %361, 42
  %363 = add i32 %359, %362
  %364 = getelementptr inbounds nuw i8, ptr %1, i16 168
  %365 = load i32, ptr %364, !tbaa !2
  %366 = mul i32 %365, 43
  %367 = add i32 %363, %366
  %368 = getelementptr inbounds nuw i8, ptr %1, i16 172
  %369 = load i32, ptr %368, !tbaa !2
  %370 = mul i32 %369, 44
  %371 = add i32 %367, %370
  %372 = getelementptr inbounds nuw i8, ptr %1, i16 176
  %373 = load i32, ptr %372, !tbaa !2
  %374 = mul i32 %373, 45
  %375 = add i32 %371, %374
  %376 = getelementptr inbounds nuw i8, ptr %1, i16 180
  %377 = load i32, ptr %376, !tbaa !2
  %378 = mul i32 %377, 46
  %379 = add i32 %375, %378
  %380 = getelementptr inbounds nuw i8, ptr %1, i16 184
  %381 = load i32, ptr %380, !tbaa !2
  %382 = mul i32 %381, 47
  %383 = add i32 %379, %382
  %384 = getelementptr inbounds nuw i8, ptr %1, i16 188
  %385 = load i32, ptr %384, !tbaa !2
  %386 = mul i32 %385, 48
  %387 = add i32 %383, %386
  %388 = getelementptr inbounds nuw i8, ptr %1, i16 192
  %389 = load i32, ptr %388, !tbaa !2
  %390 = mul i32 %389, 49
  %391 = add i32 %387, %390
  %392 = getelementptr inbounds nuw i8, ptr %1, i16 196
  %393 = load i32, ptr %392, !tbaa !2
  %394 = mul i32 %393, 50
  %395 = add i32 %391, %394
  %396 = getelementptr inbounds nuw i8, ptr %1, i16 200
  %397 = load i32, ptr %396, !tbaa !2
  %398 = mul i32 %397, 51
  %399 = add i32 %395, %398
  %400 = getelementptr inbounds nuw i8, ptr %1, i16 204
  %401 = load i32, ptr %400, !tbaa !2
  %402 = mul i32 %401, 52
  %403 = add i32 %399, %402
  %404 = getelementptr inbounds nuw i8, ptr %1, i16 208
  %405 = load i32, ptr %404, !tbaa !2
  %406 = mul i32 %405, 53
  %407 = add i32 %403, %406
  %408 = getelementptr inbounds nuw i8, ptr %1, i16 212
  %409 = load i32, ptr %408, !tbaa !2
  %410 = mul i32 %409, 54
  %411 = add i32 %407, %410
  %412 = getelementptr inbounds nuw i8, ptr %1, i16 216
  %413 = load i32, ptr %412, !tbaa !2
  %414 = mul i32 %413, 55
  %415 = add i32 %411, %414
  %416 = getelementptr inbounds nuw i8, ptr %1, i16 220
  %417 = load i32, ptr %416, !tbaa !2
  %418 = mul i32 %417, 56
  %419 = add i32 %415, %418
  %420 = getelementptr inbounds nuw i8, ptr %1, i16 224
  %421 = load i32, ptr %420, !tbaa !2
  %422 = mul i32 %421, 57
  %423 = add i32 %419, %422
  %424 = getelementptr inbounds nuw i8, ptr %1, i16 228
  %425 = load i32, ptr %424, !tbaa !2
  %426 = mul i32 %425, 58
  %427 = add i32 %423, %426
  %428 = getelementptr inbounds nuw i8, ptr %1, i16 232
  %429 = load i32, ptr %428, !tbaa !2
  %430 = mul i32 %429, 59
  %431 = add i32 %427, %430
  %432 = getelementptr inbounds nuw i8, ptr %1, i16 236
  %433 = load i32, ptr %432, !tbaa !2
  %434 = mul i32 %433, 60
  %435 = add i32 %431, %434
  %436 = getelementptr inbounds nuw i8, ptr %1, i16 240
  %437 = load i32, ptr %436, !tbaa !2
  %438 = mul i32 %437, 61
  %439 = add i32 %435, %438
  %440 = getelementptr inbounds nuw i8, ptr %1, i16 244
  %441 = load i32, ptr %440, !tbaa !2
  %442 = mul i32 %441, 62
  %443 = add i32 %439, %442
  %444 = getelementptr inbounds nuw i8, ptr %1, i16 248
  %445 = load i32, ptr %444, !tbaa !2
  %446 = mul i32 %445, 63
  %447 = add i32 %443, %446
  %448 = getelementptr inbounds nuw i8, ptr %1, i16 252
  %449 = load i32, ptr %448, !tbaa !2
  %450 = shl i32 %449, 6
  %451 = add i32 %447, %450
  call void @llvm.lifetime.end.p0(i64 256, ptr nonnull %1)
  call void @llvm.lifetime.end.p0(i64 256, ptr nonnull %2)
  call void @llvm.lifetime.end.p0(i64 256, ptr nonnull %3)
  %452 = icmp eq i32 %451, 372432
  %453 = select i1 %452, ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6), ptr getelementptr inbounds nuw (i8, ptr @$str2, i16 6)
  %454 = xor i1 %452, true
  %455 = zext i1 %454 to i16
  tail call cc 1005 addrspace(1) void @N$PS(ptr %453)
  tail call cc 1005 addrspace(1) void @N$PI4(i32 %451)
  tail call cc 1005 addrspace(1) void @N$PN()
  ret i16 %455
}

declare cc 1005 void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PI4(i32) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
