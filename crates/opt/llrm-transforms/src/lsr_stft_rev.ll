target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$var_tw_re = internal unnamed_addr global [512 x i8] zeroinitializer, align 4
@$var_tw_im = internal unnamed_addr global [512 x i8] zeroinitializer, align 4
@$var_window = internal unnamed_addr global [1024 x i8] zeroinitializer, align 4
@$var_re = internal unnamed_addr global [1024 x i8] zeroinitializer, align 4
@$var_im = internal unnamed_addr global [1024 x i8] zeroinitializer, align 4
@$var_rev = internal unnamed_addr global [256 x i8] zeroinitializer
@$var_block = internal global [512 x i8] zeroinitializer, align 2
@$var_row = internal global [258 x i8] zeroinitializer, align 2
@$str10 = internal constant [13 x i8] c"\08\00\06\00\06\00SIGNAL\00"
@$str11 = internal constant [14 x i8] c"\08\00\07\00\07\00SPECTRA\00"

define internal cc 1005 void @std.io.named(ptr addrspace(5) nocapture nofree readonly "llrm.reads"="0:2,4:8" "llrm.writes"="" %0, ptr addrspace(5) nocapture nofree writeonly %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" optsize memory(read, argmem: readwrite, inaccessiblemem: readwrite) {
b1:
  %2 = load i16, ptr addrspace(5) %0
  %3 = icmp eq i16 %2, 0
  br i1 %3, label %b11, label %4

4:
  %5 = getelementptr inbounds nuw i8, ptr addrspace(5) %0, i16 4
  %6 = load ptr addrspace(1), ptr addrspace(5) %5
  br label %b3

b3:
  %7 = phi i16 [ 0, %4 ], [ %11, %b8 ]
  %8 = phi i16 [ 0, %4 ], [ %12, %b8 ]
  %9 = icmp ult i16 %7, 79
  br i1 %9, label %b9, label %b8

b5:
  %10 = icmp ult i16 %11, 80
  br i1 %10, label %b11, label %b12

b8:
  %11 = phi i16 [ %16, %b9 ], [ %7, %b3 ]
  %12 = add nuw i16 %8, 1
  %exitcond = icmp eq i16 %12, %2
  br i1 %exitcond, label %b5, label %b3

b9:
  %13 = getelementptr inbounds i8, ptr addrspace(1) %6, i16 %8
  %14 = getelementptr inbounds nuw i8, ptr addrspace(5) %1, i16 %7
  %15 = load i8, ptr addrspace(1) %13
  store i8 %15, ptr addrspace(5) %14
  %16 = add nuw nsw i16 %7, 1
  br label %b8

b11:
  %17 = phi i16 [ %11, %b5 ], [ 0, %b1 ]
  %18 = getelementptr inbounds nuw i8, ptr addrspace(5) %1, i16 %17
  store i8 0, ptr addrspace(5) %18
  ret void

b12:
  tail call cc 1005 addrspace(1) void @N$EBND()
  unreachable
}

define internal cc 1005 range(i8 0, 2) i8 @fill(ptr addrspace(5) nocapture nofree %0, i16 range(i16 0, 129) %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" optsize {
  %3 = shl nuw nsw i16 %1, 1
  %4 = getelementptr i8, ptr @$var_block, i16 %3
  %5 = addrspacecast ptr %4 to ptr addrspace(1)
  %6 = getelementptr inbounds nuw i8, ptr addrspace(5) %0, i16 130
  %7 = getelementptr inbounds nuw i8, ptr addrspace(5) %0, i16 132
  %8 = load i16, ptr addrspace(5) %7
  %9 = load i16, ptr addrspace(5) %6, align 2
  %10 = getelementptr inbounds nuw i8, ptr addrspace(5) %0, i16 2
  %11 = tail call i16 @llvm.usub.sat.i16(i16 %8, i16 %9)
  br label %18

12:
  %13 = getelementptr i8, ptr %4, i16 %19
  %14 = getelementptr inbounds i8, ptr addrspace(5) %10, i16 %20
  %15 = load i8, ptr addrspace(5) %14
  store i8 %15, ptr %13
  %16 = add nuw i16 %20, 1
  store i16 %16, ptr addrspace(5) %6
  %17 = add nuw nsw i16 %19, 1
  %exitcond1 = icmp eq i16 %17, 256
  br i1 %exitcond1, label %32, label %18

18:
  %19 = phi i16 [ 0, %2 ], [ %17, %12 ]
  %20 = phi i16 [ %9, %2 ], [ %16, %12 ]
  %exitcond = icmp eq i16 %19, %11
  br i1 %exitcond, label %21, label %12

21:
  %22 = load i16, ptr addrspace(5) %0
  %23 = getelementptr i8, ptr addrspace(1) %5, i16 %19
  %24 = sub nuw nsw i16 256, %19
  %25 = tail call addrspace(1) i32 @LL$READ(i16 %22, ptr addrspace(1) %23, i16 %24)
  %26 = icmp slt i32 %25, 0
  br i1 %26, label %common.ret, label %27

27:
  %28 = trunc i32 %25 to i16
  %29 = add i16 %19, %28
  %30 = icmp eq i16 %29, 256
  %31 = zext i1 %30 to i8
  br label %common.ret

32:
  br label %common.ret

common.ret:
  %common.op = phi i8 [ 0, %21 ], [ %31, %27 ], [ 1, %32 ]
  ret i8 %common.op
}

define cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" optsize {
  %1 = alloca [80 x i8]
  %2 = alloca [80 x i8]
  %3 = alloca [8 x i8]
  %4 = alloca [134 x i8], align 2
  %5 = alloca [8 x i8]
  call void @llvm.lifetime.start.p0(i64 8, ptr nonnull %5)
  call void @llvm.lifetime.start.p0(i64 134, ptr nonnull %4)
  call void @llvm.lifetime.start.p0(i64 8, ptr nonnull %3)
  br label %6

6:
  %7 = phi i32 [ 32768, %0 ], [ %23, %6 ]
  %8 = phi i32 [ 0, %0 ], [ %26, %6 ]
  %9 = phi i16 [ 0, %0 ], [ %27, %6 ]
  %10 = icmp slt i32 %7, 0
  %11 = select i1 %10, i32 63, i32 0
  %12 = add i32 %11, %7
  %13 = ashr i32 %12, 6
  %14 = getelementptr inbounds nuw [4 x i8], ptr @$var_tw_re, i16 %9
  store i32 %13, ptr %14, align 4, !tbaa !2
  %15 = icmp slt i32 %8, 0
  %16 = select i1 %15, i32 63, i32 0
  %17 = add i32 %16, %8
  %18 = ashr i32 %17, 6
  %19 = sub nsw i32 0, %18
  %20 = getelementptr inbounds nuw [4 x i8], ptr @$var_tw_im, i16 %9
  store i32 %19, ptr %20, align 4, !tbaa !2
  %21 = tail call i32 @llvm.smul.fix.i32(i32 %7, i32 32758, i32 15)
  %22 = tail call i32 @llvm.smul.fix.i32(i32 %8, i32 804, i32 15)
  %23 = sub i32 %21, %22
  %24 = tail call i32 @llvm.smul.fix.i32(i32 %8, i32 32758, i32 15)
  %25 = tail call i32 @llvm.smul.fix.i32(i32 %7, i32 804, i32 15)
  %26 = add i32 %24, %25
  %27 = add nuw nsw i16 %9, 1
  %exitcond2 = icmp eq i16 %27, 128
  br i1 %exitcond2, label %28, label %6

28:
  br label %29

29:
  %30 = phi i16 [ %38, %29 ], [ 0, %28 ]
  %31 = getelementptr inbounds nuw [4 x i8], ptr @$var_tw_re, i16 %30
  %32 = load i32, ptr %31, align 4, !tbaa !2
  %33 = sub i32 512, %32
  %34 = sdiv i32 %33, 2
  %35 = getelementptr inbounds nuw [4 x i8], ptr @$var_window, i16 %30
  store i32 %34, ptr %35, align 4, !tbaa !2
  %36 = sub nsw i32 512, %34
  %37 = getelementptr inbounds nuw i8, ptr %35, i16 512
  store i32 %36, ptr %37, align 4, !tbaa !2
  %38 = add nuw nsw i16 %30, 1
  %exitcond3 = icmp eq i16 %38, 128
  br i1 %exitcond3, label %39, label %29

39:
  br label %40

40:
  %41 = phi i16 [ %61, %58 ], [ 0, %39 ]
  br label %49

42:
  store i16 6, ptr %5, !tbaa !2
  %43 = getelementptr inbounds nuw i8, ptr %5, i16 4
  store ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str10, i16 6) to ptr addrspace(1)), ptr %43, !tbaa !2
  %44 = addrspacecast ptr %5 to ptr addrspace(5)
  call void @llvm.lifetime.start.p0(i64 80, ptr nonnull %2)
  call void @llvm.memset.p0.i16(ptr nonnull align 1 %2, i8 0, i16 80, i1 false)
  %45 = addrspacecast ptr %2 to ptr addrspace(5)
  %46 = addrspacecast ptr %2 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @std.io.named(ptr addrspace(5) %44, ptr addrspace(5) %45)
  %47 = call addrspace(1) i16 @LL$OPEN(ptr addrspace(1) %46, i8 zeroext 0)
  %48 = icmp slt i16 %47, 0
  call void @llvm.lifetime.end.p0(i64 80, ptr nonnull %2)
  br i1 %48, label %234, label %69

49:
  %50 = phi i16 [ 0, %40 ], [ %55, %49 ]
  %51 = phi i16 [ %41, %40 ], [ %56, %49 ]
  %52 = phi i16 [ 0, %40 ], [ %57, %49 ]
  %53 = shl i16 %50, 1
  %54 = and i16 %51, 1
  %55 = or disjoint i16 %54, %53
  %56 = lshr i16 %51, 1
  %57 = add nuw nsw i16 %52, 1
  %exitcond4 = icmp eq i16 %57, 8
  br i1 %exitcond4, label %58, label %49

58:
  %59 = trunc i16 %55 to i8
  %60 = getelementptr inbounds nuw i8, ptr @$var_rev, i16 %41
  store i8 %59, ptr %60, !tbaa !2
  %61 = add nuw nsw i16 %41, 1
  %exitcond5 = icmp eq i16 %61, 256
  br i1 %exitcond5, label %42, label %40

62:
  %63 = phi i32 [ -1, %196 ], [ %83, %81 ]
  br label %64

64:
  %common.op.ph.ph = phi i32 [ -1, %77 ], [ %63, %62 ]
  %65 = call addrspace(1) i16 @LL$CLOSE(i16 %74)
  br label %66

66:
  %common.op.ph = phi i32 [ -1, %69 ], [ %common.op.ph.ph, %64 ]
  %67 = load i16, ptr addrspace(5) %76
  %68 = call addrspace(1) i16 @LL$CLOSE(i16 %67)
  br label %234

69:
  %insert.ext3 = zext nneg i16 %47 to i32
  store i32 %insert.ext3, ptr %4, align 2
  %sroa_idx6 = getelementptr inbounds nuw i8, ptr %4, i16 4
  call void @llvm.memset.p0.i16(ptr nonnull align 2 %sroa_idx6, i8 0, i16 130, i1 false)
  store i16 7, ptr %3, !tbaa !2
  %70 = getelementptr inbounds nuw i8, ptr %3, i16 4
  store ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str11, i16 6) to ptr addrspace(1)), ptr %70, !tbaa !2
  %71 = addrspacecast ptr %3 to ptr addrspace(5)
  call void @llvm.lifetime.start.p0(i64 80, ptr nonnull %1)
  call void @llvm.memset.p0.i16(ptr nonnull align 1 %1, i8 0, i16 80, i1 false)
  %72 = addrspacecast ptr %1 to ptr addrspace(5)
  %73 = addrspacecast ptr %1 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @std.io.named(ptr addrspace(5) %71, ptr addrspace(5) %72)
  %74 = call addrspace(1) i16 @LL$CREATE(ptr addrspace(1) %73)
  %75 = icmp slt i16 %74, 0
  call void @llvm.lifetime.end.p0(i64 80, ptr nonnull %1)
  %76 = addrspacecast ptr %4 to ptr addrspace(5)
  br i1 %75, label %66, label %77

77:
  %78 = call cc 1005 addrspace(1) i8 @fill(ptr addrspace(5) %76, i16 0)
  %79 = icmp eq i8 %78, 0
  br i1 %79, label %64, label %80

80:
  br label %81

81:
  %82 = phi i32 [ %114, %196 ], [ 0, %80 ]
  %83 = phi i32 [ %231, %196 ], [ 0, %80 ]
  %84 = phi i16 [ %85, %196 ], [ 0, %80 ]
  %85 = xor i16 %84, 1
  %86 = shl nuw nsw i16 %85, 7
  %87 = call cc 1005 addrspace(1) i8 @fill(ptr addrspace(5) %76, i16 %86)
  %88 = icmp eq i8 %87, 0
  br i1 %88, label %62, label %89

89:
  %90 = shl nuw nsw i16 %84, 8
  %91 = getelementptr inbounds nuw i8, ptr @$var_block, i16 %90
  %92 = getelementptr inbounds nuw [2 x i8], ptr @$var_block, i16 %86
  br label %159

93:
  %94 = phi i16 [ %111, %110 ], [ 2, %195 ]
  %95 = ashr exact i16 %94, 1
  %96 = call i16 @llvm.cttz.i16(i16 %94, i1 true)
  %97 = lshr i16 256, %96
  %98 = add nsw i16 %94, 510
  %99 = and i16 %98, 256
  %100 = icmp eq i16 %99, 0
  %101 = ashr i16 %94, 15
  %102 = select i1 %100, i16 0, i16 %101
  %103 = add nsw i16 %102, %97
  %104 = icmp sgt i16 %95, 0
  br i1 %104, label %105, label %110

105:
  br label %106

106:
  %107 = phi i16 [ %120, %117 ], [ 256, %105 ]
  %108 = phi i16 [ %118, %117 ], [ 0, %105 ]
  br label %115

109:
  br label %110

110:
  %111 = shl i16 %94, 1
  %112 = icmp slt i16 %111, 257
  br i1 %112, label %93, label %113

113:
  %114 = add i32 %82, 129
  br label %200

115:
  %116 = phi i16 [ 0, %106 ], [ %157, %134 ]
  %exitcond = icmp eq i16 %116, %107
  br i1 %exitcond, label %129, label %121

117:
  %118 = add nsw i16 %108, %94
  %119 = icmp samesign ult i16 %118, 256
  %120 = sub i16 %107, %94
  br i1 %119, label %106, label %109

121:
  %122 = add nuw nsw i16 %116, %108
  %123 = getelementptr inbounds nuw [4 x i8], ptr @$var_re, i16 %122
  %124 = load i32, ptr %123, align 4, !tbaa !2
  %125 = getelementptr inbounds nuw [4 x i8], ptr @$var_im, i16 %122
  %126 = load i32, ptr %125, align 4, !tbaa !2
  %127 = add nuw nsw i16 %122, %95
  %128 = icmp ult i16 %127, 256
  br i1 %128, label %130, label %133

129:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

130:
  %131 = mul i16 %116, %103
  %132 = icmp ult i16 %131, 128
  br i1 %132, label %134, label %158

133:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

134:
  %135 = getelementptr inbounds nuw [4 x i8], ptr @$var_im, i16 %127
  %136 = load i32, ptr %135, align 4, !tbaa !2
  %137 = getelementptr inbounds nuw [4 x i8], ptr @$var_re, i16 %127
  %138 = load i32, ptr %137, align 4, !tbaa !2
  %139 = getelementptr inbounds nuw [4 x i8], ptr @$var_tw_re, i16 %131
  %140 = load i32, ptr %139, align 4, !tbaa !2
  %141 = call i32 @llvm.smul.fix.i32(i32 %140, i32 %138, i32 9)
  %142 = getelementptr inbounds nuw [4 x i8], ptr @$var_tw_im, i16 %131
  %143 = load i32, ptr %142, align 4, !tbaa !2
  %144 = call i32 @llvm.smul.fix.i32(i32 %143, i32 %136, i32 9)
  %145 = sub i32 %141, %144
  %146 = call i32 @llvm.smul.fix.i32(i32 %140, i32 %136, i32 9)
  %147 = call i32 @llvm.smul.fix.i32(i32 %143, i32 %138, i32 9)
  %148 = add i32 %147, %146
  %149 = add i32 %145, %124
  %150 = sdiv i32 %149, 2
  store i32 %150, ptr %123, align 4, !tbaa !2
  %151 = add i32 %148, %126
  %152 = sdiv i32 %151, 2
  store i32 %152, ptr %125, align 4, !tbaa !2
  %153 = sub i32 %124, %145
  %154 = sdiv i32 %153, 2
  store i32 %154, ptr %137, align 4, !tbaa !2
  %155 = sub i32 %126, %148
  %156 = sdiv i32 %155, 2
  store i32 %156, ptr %135, align 4, !tbaa !2
  %157 = add nuw nsw i16 %116, 1
  %exitcond1 = icmp eq i16 %157, %95
  br i1 %exitcond1, label %117, label %115

158:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

159:
  %160 = phi i16 [ 0, %89 ], [ %194, %159 ]
  %161 = getelementptr inbounds nuw i8, ptr @$var_rev, i16 %160
  %162 = load i8, ptr %161, !tbaa !2
  %163 = zext i8 %162 to i16
  %164 = getelementptr inbounds nuw [2 x i8], ptr %91, i16 %160
  %165 = load i16, ptr %164, !tbaa !2
  %166 = sext i16 %165 to i32
  %167 = shl nsw i32 %166, 7
  %168 = ashr i32 %166, 24
  %169 = and i32 %168, 384
  %170 = add nsw i32 %169, %167
  %171 = and i32 %170, -512
  %172 = getelementptr inbounds nuw [4 x i8], ptr @$var_window, i16 %160
  %173 = load i32, ptr %172, align 4, !tbaa !2
  %174 = call i32 @llvm.smul.fix.i32(i32 %171, i32 %173, i32 9)
  %175 = getelementptr inbounds nuw [4 x i8], ptr @$var_re, i16 %163
  store i32 %174, ptr %175, align 4, !tbaa !2
  %176 = or disjoint i16 %160, 128
  %177 = getelementptr inbounds nuw i8, ptr @$var_rev, i16 %176
  %178 = load i8, ptr %177, !tbaa !2
  %179 = zext i8 %178 to i16
  %180 = getelementptr inbounds nuw [2 x i8], ptr %92, i16 %160
  %181 = load i16, ptr %180, !tbaa !2
  %182 = sext i16 %181 to i32
  %183 = shl nsw i32 %182, 7
  %184 = ashr i32 %182, 24
  %185 = and i32 %184, 384
  %186 = add nsw i32 %185, %183
  %187 = and i32 %186, -512
  %188 = getelementptr inbounds nuw [4 x i8], ptr @$var_window, i16 %176
  %189 = load i32, ptr %188, align 4, !tbaa !2
  %190 = call i32 @llvm.smul.fix.i32(i32 %187, i32 %189, i32 9)
  %191 = getelementptr inbounds nuw [4 x i8], ptr @$var_re, i16 %179
  store i32 %190, ptr %191, align 4, !tbaa !2
  %192 = getelementptr inbounds nuw [4 x i8], ptr @$var_im, i16 %163
  store i32 0, ptr %192, align 4, !tbaa !2
  %193 = getelementptr inbounds nuw [4 x i8], ptr @$var_im, i16 %179
  store i32 0, ptr %193, align 4, !tbaa !2
  %194 = add nuw nsw i16 %160, 1
  %exitcond6 = icmp eq i16 %194, 128
  br i1 %exitcond6, label %195, label %159

195:
  br label %93

196:
  %197 = call addrspace(1) i32 @LL$WRITE_FILE(i16 %74, ptr addrspace(1) addrspacecast (ptr @$var_row to ptr addrspace(1)), i16 258)
  %198 = and i32 %197, -2147418113
  %199 = icmp eq i32 %198, 258
  br i1 %199, label %81, label %62

200:
  %201 = phi i16 [ %233, %200 ], [ 0, %113 ]
  %202 = phi i32 [ %231, %200 ], [ %83, %113 ]
  %203 = phi i32 [ %232, %200 ], [ %82, %113 ]
  %204 = getelementptr inbounds nuw [4 x i8], ptr @$var_re, i16 %201
  %205 = load i32, ptr %204, align 4, !tbaa !2
  %206 = call i32 @llvm.smul.fix.i32(i32 %205, i32 2048, i32 9)
  %207 = icmp slt i32 %206, 0
  %208 = select i1 %207, i32 511, i32 0
  %209 = add i32 %208, %206
  %210 = ashr i32 %209, 9
  %211 = mul i32 %210, %210
  %212 = getelementptr inbounds nuw [4 x i8], ptr @$var_im, i16 %201
  %213 = load i32, ptr %212, align 4, !tbaa !2
  %214 = call i32 @llvm.smul.fix.i32(i32 %213, i32 2048, i32 9)
  %215 = icmp slt i32 %214, 0
  %216 = select i1 %215, i32 511, i32 0
  %217 = add i32 %216, %214
  %218 = ashr i32 %217, 9
  %219 = mul i32 %218, %218
  %220 = add i32 %219, %211
  %221 = srem i32 %220, 256
  %222 = ashr i32 %221, 31
  %223 = sdiv i32 %220, 256
  %224 = add nsw i32 %222, %223
  %225 = call i32 @llvm.smin.i32(i32 %224, i32 65535)
  %226 = trunc i32 %225 to i16
  %227 = getelementptr inbounds nuw [2 x i8], ptr @$var_row, i16 %201
  store i16 %226, ptr %227, !tbaa !2
  %228 = mul nuw nsw i32 %202, 3
  %229 = add i32 %228, %203
  %230 = add i32 %229, %225
  %231 = and i32 %230, 536870911
  %232 = add i32 %203, 1
  %233 = add nuw nsw i16 %201, 1
  %exitcond7 = icmp eq i16 %233, 129
  br i1 %exitcond7, label %196, label %200

234:
  %common.op = phi i32 [ -1, %42 ], [ %common.op.ph, %66 ]
  call void @llvm.lifetime.end.p0(i64 8, ptr nonnull %3)
  call void @llvm.lifetime.end.p0(i64 134, ptr nonnull %4)
  call void @llvm.lifetime.end.p0(i64 8, ptr nonnull %5)
  call cc 1005 addrspace(1) void @N$PI4(i32 %common.op)
  call cc 1005 addrspace(1) void @N$PN()
  ret i16 0
}

declare i32 @LL$WRITE_FILE(i16, ptr addrspace(1), i16) addrspace(1)

declare cc 1005 void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

declare void @llvm.memcpy.p1.p0.i16(ptr addrspace(1) noalias nocapture writeonly, ptr noalias nocapture readonly, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)

declare i16 @LL$OPEN(ptr addrspace(1), i8) addrspace(1)

declare i16 @LL$CREATE(ptr addrspace(1)) addrspace(1)

declare i32 @LL$READ(i16, ptr addrspace(1), i16) addrspace(1)

declare cc 1005 void @N$BDRP(ptr) addrspace(1)

declare cc 1005 ptr @N$BGRW(ptr, i16, i16) addrspace(1)

declare i16 @LL$CLOSE(i16) addrspace(1)

declare i32 @llvm.sdiv.fix.i32(i32, i32, i32 immarg) nocallback nofree nosync nounwind willreturn memory(none)

declare i32 @llvm.smul.fix.i32(i32, i32, i32 immarg) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare void @llvm.memcpy.p0.p0.i16(ptr noalias nocapture writeonly, ptr noalias nocapture readonly, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)

declare cc 1005 void @N$PI4(i32) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare i16 @llvm.cttz.i16(i16, i1 immarg) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare i32 @llvm.smin.i32(i32, i32) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare void @llvm.memset.p1.i16(ptr addrspace(1) nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare i16 @llvm.umax.i16(i16, i16) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare i16 @llvm.usub.sat.i16(i16, i16) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

!0 = !{!"llrm hir"}
!1 = !{!"place", !5, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !5, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{!"referent", !0, i64 0}
!6 = !{!5, !5, i64 0}
!7 = !{!"absolute", !0, i64 0}
!8 = !{!7, !7, i64 0}
!9 = !{i8 0, i8 2}
!10 = !{i8 0, i8 2}
!11 = !{i8 0, i8 2}
!12 = !{i8 0, i8 2}
!13 = !{i8 0, i8 2}
!14 = !{i8 0, i8 2}
!15 = !{i8 0, i8 2}
!16 = !{i8 0, i8 2}
!17 = !{i8 0, i8 2}
!18 = !{i8 0, i8 2}
!19 = !{i8 0, i8 2}
!20 = !{i8 0, i8 2}
!21 = !{i8 0, i8 2}
!22 = !{i8 0, i8 2}
!23 = !{i8 0, i8 2}
!24 = !{i8 0, i8 2}
!25 = !{i8 0, i8 2}
!26 = !{i8 0, i8 2}
