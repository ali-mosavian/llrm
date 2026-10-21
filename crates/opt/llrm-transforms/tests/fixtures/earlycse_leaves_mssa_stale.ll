@$var_tw_re = internal global [512 x i8] zeroinitializer, align 4
@$var_tw_im = internal global [512 x i8] zeroinitializer, align 4
@$var_window = internal global [1024 x i8] zeroinitializer, align 4
@$var_re = internal global [1024 x i8] zeroinitializer, align 4
@$var_im = internal global [1024 x i8] zeroinitializer, align 4
@$var_rev = internal global [256 x i8] zeroinitializer
@$var_block = internal global [512 x i8] zeroinitializer, align 2
@$var_row = internal global [258 x i8] zeroinitializer, align 2
@$str9 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"
@$str10 = internal constant [13 x i8] c"\08\00\06\00\06\00SIGNAL\00"
@$str11 = internal constant [14 x i8] c"\08\00\07\00\07\00SPECTRA\00"

declare cc 1005 void @std.os.write(ptr addrspace(1), i16) addrspace(1) nearcode

declare cc 1005 ptr addrspace(1) @std.os.text_screen() addrspace(1) nearcode memory(none) willreturn nounwind norecurse

declare cc 1005 i32 @std.io.error(i16) addrspace(1) nearcode memory(none) willreturn nounwind norecurse

declare cc 1005 void @std.io.named(ptr addrspace(1) noalias readonly dereferenceable(8) nocapture, ptr addrspace(1) nonnull dereferenceable(80) noalias nocapture writeonly) addrspace(1) nearcode memory(read, argmem: readwrite, inaccessiblemem: readwrite)

declare cc 1005 void @std.io.opened(ptr addrspace(1) nocapture writeonly initializes((0, 1), (2, 6)), i16) addrspace(1) nearcode memory(argmem: readwrite) nounwind norecurse willreturn

declare cc 1005 void @std.io.File.open(ptr addrspace(1) nocapture writeonly, ptr addrspace(1) noalias readonly dereferenceable(8) nocapture) addrspace(1) nearcode

declare cc 1005 void @std.io.File.create(ptr addrspace(1) nocapture writeonly, ptr addrspace(1) noalias readonly dereferenceable(8) nocapture) addrspace(1) nearcode

declare cc 1005 void @std.io.File.write_raw(ptr addrspace(1) nocapture writeonly initializes((0, 1), (2, 4)), ptr addrspace(1) nonnull dereferenceable(134) noalias nocapture readonly, ptr addrspace(1), i16) addrspace(1) nearcode

declare cc 1005 void @std.io.File.write(ptr addrspace(1) nocapture writeonly initializes((0, 6)), ptr addrspace(1) nonnull dereferenceable(134) noalias nocapture readonly, ptr addrspace(1) noalias readonly dereferenceable(8) nocapture) addrspace(1) nearcode

declare cc 1005 void @std.io.File.read_raw(ptr addrspace(1) nocapture writeonly, ptr addrspace(1) nonnull dereferenceable(134) noalias nocapture, ptr addrspace(1)) addrspace(1) nearcode

declare cc 1005 void @std.io.File.read_line(ptr addrspace(1) nocapture writeonly, ptr addrspace(1) nonnull dereferenceable(134) noalias) addrspace(1) nearcode

declare cc 1005 void @std.io.File.drop(ptr addrspace(1) nonnull dereferenceable(134) noalias nocapture readonly) addrspace(1) nearcode

declare cc 1005 range(i32 0, -511) i32 @quarter(i16) addrspace(1) nearcode memory(none) willreturn nounwind norecurse

declare cc 1005 void @fft() addrspace(1) nearcode memory(readwrite, argmem: none)

declare cc 1005 range(i8 0, 2) i8 @fill(ptr addrspace(1) nonnull dereferenceable(134) noalias nocapture, i32) addrspace(1) nearcode

declare cc 1005 range(i8 0, 2) i8 @put(ptr addrspace(1) nonnull dereferenceable(134) noalias nocapture readonly) addrspace(1) nearcode

declare cc 1005 range(i32 -1, 536870912) i32 @bench_stft_fixed() addrspace(1) nearcode

declare cc 1005 i16 @main() addrspace(1) nearcode

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

declare cc 1005 void @N$PI4(i32) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @llvm.memset.p1.i16(ptr addrspace(1) nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: write)

define internal cc 1005 range(i32 -1, 536870912) i32 @pipeline.body() nearcode {
b1:
  %0 = alloca [4 x i8]
  %1 = alloca [4 x i8]
  %2 = alloca [6 x i8]
  %3 = alloca [4 x i8]
  %4 = alloca [4 x i8]
  %5 = alloca [6 x i8]
  %6 = alloca [4 x i8]
  %7 = alloca [4 x i8]
  %8 = alloca [6 x i8]
  %9 = alloca [4 x i8]
  %10 = alloca [4 x i8]
  %11 = alloca [136 x i8]
  %12 = alloca [80 x i8]
  %13 = alloca [4 x i8]
  %14 = alloca [4 x i8]
  %15 = alloca [136 x i8]
  %16 = alloca [80 x i8]
  %17 = alloca [134 x i8]
  %18 = alloca [134 x i8]
  %19 = alloca [8 x i8]
  %20 = alloca [136 x i8]
  %21 = alloca [134 x i8]
  %22 = alloca [134 x i8]
  %23 = alloca [8 x i8]
  %24 = alloca [136 x i8]
  br label %b2

b2:
  %25 = phi i32 [ 0, %b1 ], [ %47, %b6 ]
  %26 = phi i32 [ 0, %b1 ], [ %46, %b6 ]
  %27 = phi i32 [ 32768, %b1 ], [ %43, %b6 ]
  %28 = icmp ult i32 %25, 128
  br i1 %28, label %b6, label %b5

b5:
  br label %b10

b6:
  %29 = icmp slt i32 %27, 0
  %30 = select i1 %29, i32 63, i32 0
  %31 = add i32 %27, %30
  %32 = ashr i32 %31, 6
  %33 = trunc nuw nsw i32 %25 to i16
  %34 = getelementptr inbounds nuw [4 x i8], ptr @$var_tw_re, i16 %33
  store i32 %32, ptr %34, !tbaa !2
  %35 = icmp slt i32 %26, 0
  %36 = select i1 %35, i32 63, i32 0
  %37 = add i32 %26, %36
  %38 = ashr i32 %37, 6
  %39 = sub nsw i32 0, %38
  %40 = getelementptr inbounds nuw [4 x i8], ptr @$var_tw_im, i16 %33
  store i32 %39, ptr %40, !tbaa !2
  %41 = call i32 @llvm.smul.fix.i32(i32 %27, i32 32758, i32 15)
  %42 = call i32 @llvm.smul.fix.i32(i32 %26, i32 804, i32 15)
  %43 = sub i32 %41, %42
  %44 = call i32 @llvm.smul.fix.i32(i32 %26, i32 32758, i32 15)
  %45 = call i32 @llvm.smul.fix.i32(i32 %27, i32 804, i32 15)
  %46 = add i32 %44, %45
  %47 = add nuw nsw i32 %25, 1
  br label %b2

b10:
  %48 = phi i32 [ 0, %b5 ], [ %60, %b16 ]
  %49 = icmp ult i32 %48, 128
  br i1 %49, label %b16, label %b13

b13:
  br label %b22

b16:
  %50 = trunc nuw nsw i32 %48 to i16
  %51 = getelementptr inbounds nuw [4 x i8], ptr @$var_tw_re, i16 %50
  %52 = load i32, ptr %51, !tbaa !2
  %53 = sub i32 512, %52
  %54 = call i32 @llvm.sdiv.fix.i32(i32 %53, i32 1024, i32 9)
  %55 = getelementptr inbounds nuw [4 x i8], ptr @$var_window, i16 %50
  store i32 %54, ptr %55, !tbaa !2
  %56 = sub i32 512, %54
  %57 = trunc nuw nsw i32 %48 to i16
  %58 = or i16 %57, 128
  %59 = getelementptr inbounds nuw [4 x i8], ptr @$var_window, i16 %58
  store i32 %56, ptr %59, !tbaa !2
  %60 = add nuw nsw i32 %48, 1
  br label %b10

b22:
  %61 = phi i32 [ 0, %b13 ], [ %130, %b30 ]
  %62 = icmp ult i32 %61, 256
  br i1 %62, label %b23, label %b25

b23:
  br label %b26

b25:
  %63 = addrspacecast ptr %24 to ptr addrspace(1)
  store i16 6, ptr %23, !tbaa !2
  %64 = getelementptr inbounds nuw i8, ptr %23, i16 2
  store i16 6, ptr %64, !tbaa !2
  %65 = getelementptr inbounds nuw i8, ptr %23, i16 4
  store ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str10, i16 6) to ptr addrspace(1)), ptr %65, !tbaa !2
  %66 = addrspacecast ptr %23 to ptr addrspace(1)
  br label %67

67:
  call void @llvm.lifetime.start.p0(i64 80, ptr %16)
  call void @llvm.lifetime.start.p0(i64 136, ptr %15)
  call void @llvm.memset.p0.i16(ptr nonnull align 1 %16, i8 0, i16 80, i1 false)
  %68 = addrspacecast ptr %16 to ptr addrspace(5)
  %69 = addrspacecast ptr %16 to ptr addrspace(1)
  %70 = load i16, ptr addrspace(1) %66
  %71 = icmp ne i16 %70, 0
  br i1 %71, label %87, label %90

72:
  %73 = phi i16 [ 0, %87 ], [ %79, %78 ]
  %74 = phi i16 [ 0, %87 ], [ %80, %78 ]
  %75 = icmp ult i16 %73, 79
  br i1 %75, label %81, label %78

76:
  %77 = icmp ult i16 %79, 80
  br i1 %77, label %90, label %86

78:
  %79 = phi i16 [ %85, %81 ], [ %73, %72 ]
  %80 = add nuw i16 %74, 1
  %exitcond2 = icmp ne i16 %80, %70
  br i1 %exitcond2, label %72, label %76

81:
  %82 = getelementptr inbounds i8, ptr addrspace(1) %89, i16 %74
  %83 = getelementptr inbounds nuw i8, ptr addrspace(5) %68, i16 %73
  %84 = load i8, ptr addrspace(1) %82
  store i8 %84, ptr addrspace(5) %83
  %85 = add nuw nsw i16 %73, 1
  br label %78

86:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

87:
  %88 = getelementptr inbounds nuw i8, ptr addrspace(1) %66, i16 4
  %89 = load ptr addrspace(1), ptr addrspace(1) %88
  br label %72

90:
  %91 = phi i16 [ %79, %76 ], [ 0, %67 ]
  %92 = getelementptr inbounds nuw i8, ptr addrspace(5) %68, i16 %91
  store i8 0, ptr addrspace(5) %92
  %93 = addrspacecast ptr %15 to ptr addrspace(5)
  %94 = addrspacecast ptr %15 to ptr addrspace(1)
  %95 = call addrspace(1) i16 @LL$OPEN(ptr addrspace(1) %69, i8 zeroext 0)
  call void @llvm.lifetime.start.p0(i64 4, ptr nonnull %14)
  %96 = icmp slt i16 %95, 0
  br i1 %96, label %97, label %110

97:
  call void @llvm.lifetime.start.p0(i64 4, ptr nonnull %13)
  %98 = add nsw i16 %95, 3
  %99 = icmp ult i16 %98, 2
  br i1 %99, label %107, label %100

100:
  %101 = icmp eq i16 %95, -5
  br i1 %101, label %107, label %102

102:
  %103 = sub i16 0, %95
  store i16 2, ptr %13, !tbaa !2
  %104 = getelementptr inbounds nuw i8, ptr %13, i16 2
  store i16 %103, ptr %104, !tbaa !2
  %105 = addrspacecast ptr %13 to ptr addrspace(5)
  %106 = load i32, ptr addrspace(5) %105, !tbaa !2
  br label %107

107:
  %common.op3 = phi i32 [ %106, %102 ], [ 0, %97 ], [ 1, %100 ]
  call void @llvm.lifetime.end.p0(i64 4, ptr nonnull %13)
  %108 = addrspacecast ptr %14 to ptr addrspace(5)
  store i32 %common.op3, ptr addrspace(5) %108, !tbaa !2
  store i8 1, ptr addrspace(5) %93
  %109 = getelementptr inbounds nuw i8, ptr addrspace(1) %94, i16 2
  call void @llvm.memcpy.p1.p0.i16(ptr addrspace(1) nonnull %109, ptr nonnull %14, i16 4, i1 false)
  br label %113

110:
  store i8 0, ptr addrspace(5) %93
  %111 = getelementptr inbounds nuw i8, ptr addrspace(5) %93, i16 2
  store i16 %95, ptr addrspace(5) %111
  %112 = getelementptr inbounds nuw i8, ptr addrspace(1) %94, i16 4
  call void @llvm.memset.p1.i16(ptr addrspace(1) %112, i8 0, i16 132, i1 false)
  br label %113

113:
  call void @llvm.lifetime.end.p0(i64 4, ptr nonnull %14)
  call void @llvm.memcpy.p1.p0.i16(ptr addrspace(1) %63, ptr nonnull %15, i16 136, i1 false)
  call void @llvm.lifetime.end.p0(i64 136, ptr %15)
  call void @llvm.lifetime.end.p0(i64 80, ptr %16)
  br label %114

114:
  %115 = load i8, ptr %24, !tbaa !2, !range !24
  %116 = icmp eq i8 %115, 1
  br i1 %116, label %common.ret, label %b33

b26:
  %117 = phi i16 [ 0, %b23 ], [ %125, %b27 ]
  %118 = phi i32 [ %61, %b23 ], [ %124, %b27 ]
  %119 = phi i32 [ 0, %b23 ], [ %123, %b27 ]
  %120 = icmp ult i16 %117, 8
  br i1 %120, label %b27, label %b29

b27:
  %121 = shl i32 %119, 1
  %122 = and i32 %118, 1
  %123 = or disjoint i32 %121, %122
  %124 = lshr i32 %118, 1
  %125 = add nsw i16 %117, 1
  br label %b26

b29:
  %126 = icmp ult i32 %61, 256
  br i1 %126, label %b30, label %b31

b30:
  %127 = trunc i32 %119 to i8
  %128 = trunc nuw nsw i32 %61 to i16
  %129 = getelementptr inbounds nuw i8, ptr @$var_rev, i16 %128
  store i8 %127, ptr %129, !tbaa !2
  %130 = add nuw nsw i32 %61, 1
  br label %b22

b31:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

common.ret:
  %common.op = phi i32 [ %326, %398 ], [ -1, %114 ], [ -1, %601 ], [ -1, %231 ], [ -1, %324 ]
  ret i32 %common.op

b33:
  %131 = getelementptr inbounds nuw i8, ptr %24, i16 2
  %132 = load i16, ptr %131, !tbaa !2
  %133 = getelementptr inbounds nuw i8, ptr %24, i16 132
  %134 = load i16, ptr %133, !tbaa !2
  %135 = getelementptr inbounds nuw i8, ptr %24, i16 134
  %136 = load i16, ptr %135, !tbaa !2
  store i16 %132, ptr %22, !tbaa !2
  %137 = getelementptr inbounds nuw i8, ptr %22, i16 2
  %138 = addrspacecast ptr %137 to ptr addrspace(1)
  %139 = getelementptr inbounds nuw i8, ptr %24, i16 4
  %140 = addrspacecast ptr %139 to ptr addrspace(1)
  br label %b36

b36:
  %141 = phi i16 [ 0, %b33 ], [ %146, %b37 ]
  %142 = icmp ult i16 %141, 128
  br i1 %142, label %b37, label %b39

b37:
  %143 = getelementptr inbounds nuw i8, ptr addrspace(1) %138, i16 %141
  %144 = getelementptr inbounds nuw i8, ptr addrspace(1) %140, i16 %141
  %145 = load i8, ptr addrspace(1) %144
  store i8 %145, ptr addrspace(1) %143
  %146 = add nsw i16 %141, 1
  br label %b36

b39:
  %147 = getelementptr inbounds nuw i8, ptr %22, i16 130
  store i16 %134, ptr %147, !tbaa !2
  %148 = getelementptr inbounds nuw i8, ptr %22, i16 132
  store i16 %136, ptr %148, !tbaa !2
  store i16 %132, ptr %21, !tbaa !2
  %149 = getelementptr inbounds nuw i8, ptr %21, i16 2
  %150 = addrspacecast ptr %149 to ptr addrspace(1)
  br label %b40

b40:
  %151 = phi i16 [ 0, %b39 ], [ %156, %b41 ]
  %152 = icmp ult i16 %151, 128
  br i1 %152, label %b41, label %b43

b41:
  %153 = getelementptr inbounds nuw i8, ptr addrspace(1) %150, i16 %151
  %154 = getelementptr inbounds nuw i8, ptr addrspace(1) %138, i16 %151
  %155 = load i8, ptr addrspace(1) %154
  store i8 %155, ptr addrspace(1) %153
  %156 = add nsw i16 %151, 1
  br label %b40

b43:
  %157 = getelementptr inbounds nuw i8, ptr %21, i16 130
  store i16 %134, ptr %157, !tbaa !2
  %158 = getelementptr inbounds nuw i8, ptr %21, i16 132
  store i16 %136, ptr %158, !tbaa !2
  store i16 0, ptr %22, !tbaa !2
  br label %b44

b44:
  %159 = phi i16 [ 0, %b43 ], [ %162, %b45 ]
  %160 = icmp ult i16 %159, 128
  br i1 %160, label %b45, label %b47

b45:
  %161 = getelementptr inbounds nuw i8, ptr addrspace(1) %138, i16 %159
  store i8 0, ptr addrspace(1) %161
  %162 = add nsw i16 %159, 1
  br label %b44

b47:
  store i16 0, ptr %147, !tbaa !2
  store i16 0, ptr %148, !tbaa !2
  %163 = addrspacecast ptr %20 to ptr addrspace(1)
  store i16 7, ptr %19, !tbaa !2
  %164 = getelementptr inbounds nuw i8, ptr %19, i16 2
  store i16 7, ptr %164, !tbaa !2
  %165 = getelementptr inbounds nuw i8, ptr %19, i16 4
  store ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str11, i16 6) to ptr addrspace(1)), ptr %165, !tbaa !2
  %166 = addrspacecast ptr %19 to ptr addrspace(1)
  br label %167

167:
  call void @llvm.lifetime.start.p0(i64 80, ptr %12)
  call void @llvm.lifetime.start.p0(i64 136, ptr %11)
  call void @llvm.memset.p0.i16(ptr nonnull align 1 %12, i8 0, i16 80, i1 false)
  %168 = addrspacecast ptr %12 to ptr addrspace(5)
  %169 = addrspacecast ptr %12 to ptr addrspace(1)
  %170 = load i16, ptr addrspace(1) %166
  %171 = icmp ne i16 %170, 0
  br i1 %171, label %187, label %190

172:
  %173 = phi i16 [ 0, %187 ], [ %179, %178 ]
  %174 = phi i16 [ 0, %187 ], [ %180, %178 ]
  %175 = icmp ult i16 %173, 79
  br i1 %175, label %181, label %178

176:
  %177 = icmp ult i16 %179, 80
  br i1 %177, label %190, label %186

178:
  %179 = phi i16 [ %185, %181 ], [ %173, %172 ]
  %180 = add nuw i16 %174, 1
  %exitcond1 = icmp ne i16 %180, %170
  br i1 %exitcond1, label %172, label %176

181:
  %182 = getelementptr inbounds i8, ptr addrspace(1) %189, i16 %174
  %183 = getelementptr inbounds nuw i8, ptr addrspace(5) %168, i16 %173
  %184 = load i8, ptr addrspace(1) %182
  store i8 %184, ptr addrspace(5) %183
  %185 = add nuw nsw i16 %173, 1
  br label %178

186:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

187:
  %188 = getelementptr inbounds nuw i8, ptr addrspace(1) %166, i16 4
  %189 = load ptr addrspace(1), ptr addrspace(1) %188
  br label %172

190:
  %191 = phi i16 [ %179, %176 ], [ 0, %167 ]
  %192 = getelementptr inbounds nuw i8, ptr addrspace(5) %168, i16 %191
  store i8 0, ptr addrspace(5) %192
  %193 = addrspacecast ptr %11 to ptr addrspace(5)
  %194 = addrspacecast ptr %11 to ptr addrspace(1)
  %195 = call addrspace(1) i16 @LL$CREATE(ptr addrspace(1) %169)
  call void @llvm.lifetime.start.p0(i64 4, ptr nonnull %10)
  %196 = icmp slt i16 %195, 0
  br i1 %196, label %197, label %210

197:
  call void @llvm.lifetime.start.p0(i64 4, ptr nonnull %9)
  %198 = add nsw i16 %195, 3
  %199 = icmp ult i16 %198, 2
  br i1 %199, label %207, label %200

200:
  %201 = icmp eq i16 %195, -5
  br i1 %201, label %207, label %202

202:
  %203 = sub i16 0, %195
  store i16 2, ptr %9, !tbaa !2
  %204 = getelementptr inbounds nuw i8, ptr %9, i16 2
  store i16 %203, ptr %204, !tbaa !2
  %205 = addrspacecast ptr %9 to ptr addrspace(5)
  %206 = load i32, ptr addrspace(5) %205, !tbaa !2
  br label %207

207:
  %common.op2 = phi i32 [ %206, %202 ], [ 0, %197 ], [ 1, %200 ]
  call void @llvm.lifetime.end.p0(i64 4, ptr nonnull %9)
  %208 = addrspacecast ptr %10 to ptr addrspace(5)
  store i32 %common.op2, ptr addrspace(5) %208, !tbaa !2
  store i8 1, ptr addrspace(5) %193
  %209 = getelementptr inbounds nuw i8, ptr addrspace(1) %194, i16 2
  call void @llvm.memcpy.p1.p0.i16(ptr addrspace(1) nonnull %209, ptr nonnull %10, i16 4, i1 false)
  br label %213

210:
  store i8 0, ptr addrspace(5) %193
  %211 = getelementptr inbounds nuw i8, ptr addrspace(5) %193, i16 2
  store i16 %195, ptr addrspace(5) %211
  %212 = getelementptr inbounds nuw i8, ptr addrspace(1) %194, i16 4
  call void @llvm.memset.p1.i16(ptr addrspace(1) %212, i8 0, i16 132, i1 false)
  br label %213

213:
  call void @llvm.lifetime.end.p0(i64 4, ptr nonnull %10)
  call void @llvm.memcpy.p1.p0.i16(ptr addrspace(1) %163, ptr nonnull %11, i16 136, i1 false)
  call void @llvm.lifetime.end.p0(i64 136, ptr %11)
  call void @llvm.lifetime.end.p0(i64 80, ptr %12)
  br label %214

214:
  %215 = load i8, ptr %20, !tbaa !2, !range !24
  %216 = icmp eq i8 %215, 1
  br i1 %216, label %b52, label %b49

b49:
  %217 = getelementptr inbounds nuw i8, ptr %20, i16 2
  %218 = load i16, ptr %217, !tbaa !2
  %219 = getelementptr inbounds nuw i8, ptr %20, i16 132
  %220 = load i16, ptr %219, !tbaa !2
  %221 = getelementptr inbounds nuw i8, ptr %20, i16 134
  %222 = load i16, ptr %221, !tbaa !2
  store i16 %218, ptr %18, !tbaa !2
  %223 = getelementptr inbounds nuw i8, ptr %18, i16 2
  %224 = addrspacecast ptr %223 to ptr addrspace(1)
  %225 = getelementptr inbounds nuw i8, ptr %20, i16 4
  %226 = addrspacecast ptr %225 to ptr addrspace(1)
  br label %b56

b52:
  %227 = addrspacecast ptr %21 to ptr addrspace(1)
  br label %228

228:
  %229 = load i16, ptr addrspace(1) %227
  %230 = call addrspace(1) i16 @LL$CLOSE(i16 %229)
  br label %231

231:
  br label %common.ret

b56:
  %232 = phi i16 [ 0, %b49 ], [ %237, %b57 ]
  %233 = icmp ult i16 %232, 128
  br i1 %233, label %b57, label %b59

b57:
  %234 = getelementptr inbounds nuw i8, ptr addrspace(1) %224, i16 %232
  %235 = getelementptr inbounds nuw i8, ptr addrspace(1) %226, i16 %232
  %236 = load i8, ptr addrspace(1) %235
  store i8 %236, ptr addrspace(1) %234
  %237 = add nsw i16 %232, 1
  br label %b56

b59:
  %238 = getelementptr inbounds nuw i8, ptr %18, i16 130
  store i16 %220, ptr %238, !tbaa !2
  %239 = getelementptr inbounds nuw i8, ptr %18, i16 132
  store i16 %222, ptr %239, !tbaa !2
  store i16 %218, ptr %17, !tbaa !2
  %240 = getelementptr inbounds nuw i8, ptr %17, i16 2
  %241 = addrspacecast ptr %240 to ptr addrspace(1)
  br label %b60

b60:
  %242 = phi i16 [ 0, %b59 ], [ %247, %b61 ]
  %243 = icmp ult i16 %242, 128
  br i1 %243, label %b61, label %b63

b61:
  %244 = getelementptr inbounds nuw i8, ptr addrspace(1) %241, i16 %242
  %245 = getelementptr inbounds nuw i8, ptr addrspace(1) %224, i16 %242
  %246 = load i8, ptr addrspace(1) %245
  store i8 %246, ptr addrspace(1) %244
  %247 = add nsw i16 %242, 1
  br label %b60

b63:
  %248 = getelementptr inbounds nuw i8, ptr %17, i16 130
  store i16 %220, ptr %248, !tbaa !2
  %249 = getelementptr inbounds nuw i8, ptr %17, i16 132
  store i16 %222, ptr %249, !tbaa !2
  store i16 0, ptr %18, !tbaa !2
  br label %b64

b64:
  %250 = phi i16 [ 0, %b63 ], [ %253, %b65 ]
  %251 = icmp ult i16 %250, 128
  br i1 %251, label %b65, label %b67

b65:
  %252 = getelementptr inbounds nuw i8, ptr addrspace(1) %224, i16 %250
  store i8 0, ptr addrspace(1) %252
  %253 = add nsw i16 %250, 1
  br label %b64

b67:
  store i16 0, ptr %238, !tbaa !2
  store i16 0, ptr %239, !tbaa !2
  %254 = addrspacecast ptr %21 to ptr addrspace(1)
  br label %255

255:
  call void @llvm.lifetime.start.p0(i64 6, ptr %8)
  %256 = addrspacecast ptr %8 to ptr addrspace(5)
  %257 = addrspacecast ptr %8 to ptr addrspace(1)
  %258 = trunc i32 0 to i16
  %259 = shl i16 %258, 1
  %260 = getelementptr i8, ptr @$var_block, i16 %259
  %261 = addrspacecast ptr %260 to ptr addrspace(1)
  call void @llvm.lifetime.start.p0(i64 4, ptr nonnull %7)
  %262 = getelementptr inbounds nuw i8, ptr addrspace(1) %254, i16 130
  %263 = getelementptr inbounds nuw i8, ptr addrspace(1) %254, i16 132
  %264 = load i16, ptr addrspace(1) %263
  %265 = load i16, ptr addrspace(1) %262, align 1
  %266 = icmp ult i16 %265, %264
  br i1 %266, label %304, label %279

267:
  %268 = phi i16 [ 0, %304 ], [ %274, %267 ]
  %269 = phi i16 [ %265, %304 ], [ %273, %267 ]
  %270 = getelementptr i8, ptr %260, i16 %268
  %271 = getelementptr inbounds i8, ptr addrspace(1) %305, i16 %269
  %272 = load i8, ptr addrspace(1) %271
  store i8 %272, ptr %270
  %273 = add nuw i16 %269, 1
  store i16 %273, ptr addrspace(1) %262
  %274 = add nuw nsw i16 %268, 1
  %275 = icmp samesign ult i16 %274, 256
  %276 = icmp ult i16 %273, %264
  %277 = and i1 %275, %276
  br i1 %277, label %267, label %278

278:
  br i1 %275, label %279, label %306

279:
  %280 = phi i16 [ %274, %278 ], [ 0, %255 ]
  %281 = load i16, ptr addrspace(1) %254
  %282 = getelementptr i8, ptr addrspace(1) %261, i16 %280
  %283 = sub nuw nsw i16 256, %280
  %284 = call addrspace(1) i32 @LL$READ(i16 %281, ptr addrspace(1) %282, i16 %283)
  %285 = icmp slt i32 %284, 0
  %286 = trunc i32 %284 to i16
  br i1 %285, label %287, label %302

287:
  call void @llvm.lifetime.start.p0(i64 4, ptr nonnull %6)
  %288 = add i16 %286, 3
  %289 = icmp ult i16 %288, 2
  br i1 %289, label %297, label %290

290:
  %291 = icmp eq i16 %286, -5
  br i1 %291, label %297, label %292

292:
  %293 = sub i16 0, %286
  store i16 2, ptr %6, !tbaa !2
  %294 = getelementptr inbounds nuw i8, ptr %6, i16 2
  store i16 %293, ptr %294, !tbaa !2
  %295 = addrspacecast ptr %6 to ptr addrspace(5)
  %296 = load i32, ptr addrspace(5) %295, !tbaa !2
  br label %297

297:
  %common.op12 = phi i32 [ %296, %292 ], [ 0, %287 ], [ 1, %290 ]
  call void @llvm.lifetime.end.p0(i64 4, ptr nonnull %6)
  %298 = addrspacecast ptr %7 to ptr addrspace(5)
  store i32 %common.op12, ptr addrspace(5) %298, !tbaa !2
  store i8 1, ptr addrspace(5) %256
  %299 = getelementptr inbounds nuw i8, ptr addrspace(1) %257, i16 2
  call void @llvm.memcpy.p1.p0.i16(ptr addrspace(1) nonnull %299, ptr nonnull %7, i16 4, i1 false)
  %300 = load i8, ptr %8, !tbaa !2, !range !22
  %.phi.trans.insert2 = getelementptr inbounds nuw i8, ptr %8, i16 2
  %301 = load i16, ptr %.phi.trans.insert2
  br label %306

302:
  %303 = add i16 %280, %286
  br label %306

304:
  %305 = getelementptr inbounds nuw i8, ptr addrspace(1) %254, i16 2
  br label %267

306:
  %307 = phi i16 [ %301, %297 ], [ %303, %302 ], [ 256, %278 ]
  %308 = phi i8 [ %300, %297 ], [ 0, %302 ], [ 0, %278 ]
  call void @llvm.lifetime.end.p0(i64 4, ptr nonnull %7)
  %309 = icmp eq i8 %308, 0
  %310 = icmp eq i16 %307, 256
  %311 = select i1 %309, i1 %310, i1 false
  %312 = zext i1 %311 to i8
  call void @llvm.lifetime.end.p0(i64 6, ptr %8)
  br label %313

313:
  %314 = xor i8 %312, 1
  %315 = icmp ne i8 %314, 0
  br i1 %315, label %b72, label %b70

b70:
  br label %b79

b72:
  %316 = addrspacecast ptr %17 to ptr addrspace(1)
  br label %317

317:
  %318 = load i16, ptr addrspace(1) %316
  %319 = call addrspace(1) i16 @LL$CLOSE(i16 %318)
  br label %320

320:
  br label %321

321:
  %322 = load i16, ptr addrspace(1) %254
  %323 = call addrspace(1) i16 @LL$CLOSE(i16 %322)
  br label %324

324:
  br label %common.ret

b79:
  %325 = phi i32 [ 0, %b70 ], [ %526, %b125 ]
  %326 = phi i32 [ 0, %b70 ], [ %527, %b125 ]
  %327 = phi i32 [ 0, %b70 ], [ %328, %b125 ]
  %328 = xor i32 %327, 1
  %329 = shl nuw nsw i32 %328, 7
  br label %330

330:
  call void @llvm.lifetime.start.p0(i64 6, ptr %5)
  %331 = addrspacecast ptr %5 to ptr addrspace(5)
  %332 = addrspacecast ptr %5 to ptr addrspace(1)
  %333 = trunc i32 %329 to i16
  %334 = shl i16 %333, 1
  %335 = getelementptr i8, ptr @$var_block, i16 %334
  %336 = addrspacecast ptr %335 to ptr addrspace(1)
  call void @llvm.lifetime.start.p0(i64 4, ptr nonnull %4)
  %337 = getelementptr inbounds nuw i8, ptr addrspace(1) %254, i16 130
  %338 = getelementptr inbounds nuw i8, ptr addrspace(1) %254, i16 132
  %339 = load i16, ptr addrspace(1) %338
  %340 = load i16, ptr addrspace(1) %337, align 1
  %341 = icmp ult i16 %340, %339
  br i1 %341, label %379, label %354

342:
  %343 = phi i16 [ 0, %379 ], [ %349, %342 ]
  %344 = phi i16 [ %340, %379 ], [ %348, %342 ]
  %345 = getelementptr i8, ptr %335, i16 %343
  %346 = getelementptr inbounds i8, ptr addrspace(1) %380, i16 %344
  %347 = load i8, ptr addrspace(1) %346
  store i8 %347, ptr %345
  %348 = add nuw i16 %344, 1
  store i16 %348, ptr addrspace(1) %337
  %349 = add nuw nsw i16 %343, 1
  %350 = icmp samesign ult i16 %349, 256
  %351 = icmp ult i16 %348, %339
  %352 = and i1 %350, %351
  br i1 %352, label %342, label %353

353:
  br i1 %350, label %354, label %381

354:
  %355 = phi i16 [ %349, %353 ], [ 0, %330 ]
  %356 = load i16, ptr addrspace(1) %254
  %357 = getelementptr i8, ptr addrspace(1) %336, i16 %355
  %358 = sub nuw nsw i16 256, %355
  %359 = call addrspace(1) i32 @LL$READ(i16 %356, ptr addrspace(1) %357, i16 %358)
  %360 = icmp slt i32 %359, 0
  %361 = trunc i32 %359 to i16
  br i1 %360, label %362, label %377

362:
  call void @llvm.lifetime.start.p0(i64 4, ptr nonnull %3)
  %363 = add i16 %361, 3
  %364 = icmp ult i16 %363, 2
  br i1 %364, label %372, label %365

365:
  %366 = icmp eq i16 %361, -5
  br i1 %366, label %372, label %367

367:
  %368 = sub i16 0, %361
  store i16 2, ptr %3, !tbaa !2
  %369 = getelementptr inbounds nuw i8, ptr %3, i16 2
  store i16 %368, ptr %369, !tbaa !2
  %370 = addrspacecast ptr %3 to ptr addrspace(5)
  %371 = load i32, ptr addrspace(5) %370, !tbaa !2
  br label %372

372:
  %common.op11 = phi i32 [ %371, %367 ], [ 0, %362 ], [ 1, %365 ]
  call void @llvm.lifetime.end.p0(i64 4, ptr nonnull %3)
  %373 = addrspacecast ptr %4 to ptr addrspace(5)
  store i32 %common.op11, ptr addrspace(5) %373, !tbaa !2
  store i8 1, ptr addrspace(5) %331
  %374 = getelementptr inbounds nuw i8, ptr addrspace(1) %332, i16 2
  call void @llvm.memcpy.p1.p0.i16(ptr addrspace(1) nonnull %374, ptr nonnull %4, i16 4, i1 false)
  %375 = load i8, ptr %5, !tbaa !2, !range !22
  %.phi.trans.insert1 = getelementptr inbounds nuw i8, ptr %5, i16 2
  %376 = load i16, ptr %.phi.trans.insert1
  br label %381

377:
  %378 = add i16 %355, %361
  br label %381

379:
  %380 = getelementptr inbounds nuw i8, ptr addrspace(1) %254, i16 2
  br label %342

381:
  %382 = phi i16 [ %376, %372 ], [ %378, %377 ], [ 256, %353 ]
  %383 = phi i8 [ %375, %372 ], [ 0, %377 ], [ 0, %353 ]
  call void @llvm.lifetime.end.p0(i64 4, ptr nonnull %4)
  %384 = icmp eq i8 %383, 0
  %385 = icmp eq i16 %382, 256
  %386 = select i1 %384, i1 %385, i1 false
  %387 = zext i1 %386 to i8
  call void @llvm.lifetime.end.p0(i64 6, ptr %5)
  br label %388

388:
  %389 = icmp ne i8 %387, 0
  br i1 %389, label %b80, label %b81

b80:
  br label %b82

b81:
  %390 = addrspacecast ptr %17 to ptr addrspace(1)
  br label %391

391:
  %392 = load i16, ptr addrspace(1) %390
  %393 = call addrspace(1) i16 @LL$CLOSE(i16 %392)
  br label %394

394:
  br label %395

395:
  %396 = load i16, ptr addrspace(1) %254
  %397 = call addrspace(1) i16 @LL$CLOSE(i16 %396)
  br label %398

398:
  br label %common.ret

b82:
  %399 = phi i32 [ 0, %b80 ], [ %525, %517 ]
  %400 = icmp ult i32 %399, 128
  br i1 %400, label %b86, label %b85

b85:
  br label %401

401:
  br label %402

402:
  %403 = phi i32 [ 2, %401 ], [ %478, %477 ]
  %404 = ashr exact i32 %403, 1
  %405 = udiv i32 256, %403
  %406 = srem i32 256, %403
  %407 = icmp ne i32 %406, 0
  %408 = xor i32 %406, %403
  %409 = ashr i32 %408, 31
  %410 = select i1 %407, i32 %409, i32 0
  %411 = add nsw i32 %410, %405
  %412 = icmp sgt i32 %404, 0
  br i1 %412, label %419, label %418

413:
  br label %480

414:
  %415 = phi i32 [ 0, %418 ], [ %416, %414 ]
  %416 = add i32 %415, %403
  %417 = icmp slt i32 %416, 256
  br i1 %417, label %414, label %477

418:
  br label %414

419:
  br label %420

420:
  %421 = phi i32 [ 0, %419 ], [ %472, %471 ]
  br label %422

422:
  %423 = phi i32 [ 0, %420 ], [ %470, %437 ]
  %424 = add nuw nsw i32 %423, %421
  %425 = icmp ult i32 %424, 256
  br i1 %425, label %426, label %474

426:
  %427 = trunc nuw nsw i32 %424 to i16
  %428 = getelementptr inbounds nuw [4 x i8], ptr @$var_re, i16 %427
  %429 = load i32, ptr %428, !tbaa !2
  %430 = getelementptr inbounds nuw [4 x i8], ptr @$var_im, i16 %427
  %431 = load i32, ptr %430, !tbaa !2
  %432 = add nuw nsw i32 %424, %404
  %433 = icmp ult i32 %432, 256
  br i1 %433, label %434, label %475

434:
  %435 = mul nsw i32 %423, %411
  %436 = icmp ult i32 %435, 128
  br i1 %436, label %437, label %476

437:
  %438 = trunc nuw nsw i32 %432 to i16
  %439 = getelementptr inbounds nuw [4 x i8], ptr @$var_im, i16 %438
  %440 = load i32, ptr %439, !tbaa !2
  %441 = getelementptr inbounds nuw [4 x i8], ptr @$var_re, i16 %438
  %442 = load i32, ptr %441, !tbaa !2
  %443 = trunc nuw nsw i32 %435 to i16
  %444 = getelementptr inbounds nuw [4 x i8], ptr @$var_tw_re, i16 %443
  %445 = load i32, ptr %444, !tbaa !2
  %446 = call i32 @llvm.smul.fix.i32(i32 %445, i32 %442, i32 9)
  %447 = getelementptr inbounds nuw [4 x i8], ptr @$var_tw_im, i16 %443
  %448 = load i32, ptr %447, !tbaa !2
  %449 = call i32 @llvm.smul.fix.i32(i32 %448, i32 %440, i32 9)
  %450 = sub i32 %446, %449
  %451 = call i32 @llvm.smul.fix.i32(i32 %445, i32 %440, i32 9)
  %452 = call i32 @llvm.smul.fix.i32(i32 %448, i32 %442, i32 9)
  %453 = add i32 %452, %451
  %454 = add i32 %450, %429
  %455 = lshr i32 %454, 31
  %456 = add i32 %455, %454
  %457 = ashr i32 %456, 1
  store i32 %457, ptr %428, !tbaa !2
  %458 = add i32 %453, %431
  %459 = lshr i32 %458, 31
  %460 = add i32 %459, %458
  %461 = ashr i32 %460, 1
  store i32 %461, ptr %430, !tbaa !2
  %462 = sub i32 %429, %450
  %463 = lshr i32 %462, 31
  %464 = add i32 %463, %462
  %465 = ashr i32 %464, 1
  store i32 %465, ptr %441, !tbaa !2
  %466 = sub i32 %431, %453
  %467 = lshr i32 %466, 31
  %468 = add i32 %467, %466
  %469 = ashr i32 %468, 1
  store i32 %469, ptr %439, !tbaa !2
  %470 = add nuw nsw i32 %423, 1
  %exitcond = icmp ne i32 %470, %404
  br i1 %exitcond, label %422, label %471

471:
  %472 = add i32 %421, %403
  %473 = icmp slt i32 %472, 256
  br i1 %473, label %420, label %477

474:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

475:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

476:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

477:
  %478 = shl i32 %403, 1
  %479 = icmp slt i32 %478, 257
  br i1 %479, label %402, label %413

480:
  br label %b110

b86:
  %481 = trunc nuw nsw i32 %399 to i16
  %482 = getelementptr inbounds nuw i8, ptr @$var_rev, i16 %481
  %483 = load i8, ptr %482, !tbaa !2
  %484 = shl nuw nsw i32 %327, 7
  %485 = or disjoint i32 %484, %399
  %486 = trunc nuw nsw i32 %485 to i16
  %487 = getelementptr inbounds nuw [2 x i8], ptr @$var_block, i16 %486
  %488 = load i16, ptr %487, !tbaa !2
  br label %489

489:
  %490 = sext i16 %488 to i32
  %491 = shl nsw i32 %490, 7
  %492 = icmp slt i32 %491, 0
  %493 = select i1 %492, i32 511, i32 0
  %494 = add nsw i32 %493, %491
  %495 = and i32 %494, -512
  br label %496

496:
  %497 = getelementptr inbounds nuw [4 x i8], ptr @$var_window, i16 %481
  %498 = load i32, ptr %497, !tbaa !2
  %499 = call i32 @llvm.smul.fix.i32(i32 %495, i32 %498, i32 9)
  %500 = zext i8 %483 to i16
  %501 = getelementptr inbounds nuw [4 x i8], ptr @$var_re, i16 %500
  store i32 %499, ptr %501, !tbaa !2
  %502 = trunc nuw nsw i32 %399 to i16
  %503 = or i16 %502, 128
  %504 = getelementptr inbounds nuw i8, ptr @$var_rev, i16 %503
  %505 = load i8, ptr %504, !tbaa !2
  %506 = or disjoint i32 %329, %399
  %507 = trunc nuw nsw i32 %506 to i16
  %508 = getelementptr inbounds nuw [2 x i8], ptr @$var_block, i16 %507
  %509 = load i16, ptr %508, !tbaa !2
  br label %510

510:
  %511 = sext i16 %509 to i32
  %512 = shl nsw i32 %511, 7
  %513 = icmp slt i32 %512, 0
  %514 = select i1 %513, i32 511, i32 0
  %515 = add nsw i32 %514, %512
  %516 = and i32 %515, -512
  br label %517

517:
  %518 = getelementptr inbounds nuw [4 x i8], ptr @$var_window, i16 %503
  %519 = load i32, ptr %518, !tbaa !2
  %520 = call i32 @llvm.smul.fix.i32(i32 %516, i32 %519, i32 9)
  %521 = zext i8 %505 to i16
  %522 = getelementptr inbounds nuw [4 x i8], ptr @$var_re, i16 %521
  store i32 %520, ptr %522, !tbaa !2
  %523 = getelementptr inbounds nuw [4 x i8], ptr @$var_im, i16 %500
  store i32 0, ptr %523, !tbaa !2
  %524 = getelementptr inbounds nuw [4 x i8], ptr @$var_im, i16 %521
  store i32 0, ptr %524, !tbaa !2
  %525 = add nuw nsw i32 %399, 1
  br label %b82

b110:
  %526 = phi i32 [ %325, %480 ], [ %592, %b114 ]
  %527 = phi i32 [ %326, %480 ], [ %591, %b114 ]
  %528 = phi i32 [ 0, %480 ], [ %593, %b114 ]
  %529 = icmp ult i32 %528, 129
  br i1 %529, label %b114, label %b113

b113:
  %530 = addrspacecast ptr %17 to ptr addrspace(1)
  br label %531

531:
  call void @llvm.lifetime.start.p0(i64 6, ptr %2)
  %532 = addrspacecast ptr %2 to ptr addrspace(5)
  %533 = addrspacecast ptr %2 to ptr addrspace(1)
  call void @llvm.lifetime.start.p0(i64 4, ptr nonnull %1)
  %534 = load i16, ptr addrspace(1) %530
  %535 = call addrspace(1) i32 @LL$WRITE_FILE(i16 %534, ptr addrspace(1) addrspacecast (ptr @$var_row to ptr addrspace(1)), i16 258)
  %536 = icmp slt i32 %535, 0
  %537 = trunc i32 %535 to i16
  br i1 %536, label %538, label %553

538:
  call void @llvm.lifetime.start.p0(i64 4, ptr nonnull %0)
  %539 = add i16 %537, 3
  %540 = icmp ult i16 %539, 2
  br i1 %540, label %548, label %541

541:
  %542 = icmp eq i16 %537, -5
  br i1 %542, label %548, label %543

543:
  %544 = sub i16 0, %537
  store i16 2, ptr %0, !tbaa !2
  %545 = getelementptr inbounds nuw i8, ptr %0, i16 2
  store i16 %544, ptr %545, !tbaa !2
  %546 = addrspacecast ptr %0 to ptr addrspace(5)
  %547 = load i32, ptr addrspace(5) %546, !tbaa !2
  br label %548

548:
  %common.op1 = phi i32 [ %547, %543 ], [ 0, %538 ], [ 1, %541 ]
  call void @llvm.lifetime.end.p0(i64 4, ptr nonnull %0)
  %549 = addrspacecast ptr %1 to ptr addrspace(5)
  store i32 %common.op1, ptr addrspace(5) %549, !tbaa !2
  store i8 1, ptr addrspace(5) %532
  %550 = getelementptr inbounds nuw i8, ptr addrspace(1) %533, i16 2
  call void @llvm.memcpy.p1.p0.i16(ptr addrspace(1) nonnull %550, ptr nonnull %1, i16 4, i1 false)
  %551 = load i8, ptr %2, !tbaa !2, !range !23
  %.phi.trans.insert = getelementptr inbounds nuw i8, ptr %2, i16 2
  %552 = load i16, ptr %.phi.trans.insert
  br label %553

553:
  %554 = phi i16 [ %552, %548 ], [ %537, %531 ]
  %555 = phi i8 [ %551, %548 ], [ 0, %531 ]
  call void @llvm.lifetime.end.p0(i64 4, ptr nonnull %1)
  %556 = icmp eq i8 %555, 0
  %557 = icmp eq i16 %554, 258
  %558 = select i1 %556, i1 %557, i1 false
  %559 = zext i1 %558 to i8
  call void @llvm.lifetime.end.p0(i64 6, ptr %2)
  br label %560

560:
  %561 = xor i8 %559, 1
  %562 = icmp ne i8 %561, 0
  br i1 %562, label %b123, label %b125

b114:
  %563 = trunc nuw nsw i32 %528 to i16
  %564 = getelementptr inbounds nuw [4 x i8], ptr @$var_re, i16 %563
  %565 = load i32, ptr %564, !tbaa !2
  %566 = call i32 @llvm.smul.fix.i32(i32 %565, i32 2048, i32 9)
  %567 = icmp slt i32 %566, 0
  %568 = select i1 %567, i32 511, i32 0
  %569 = add i32 %566, %568
  %570 = ashr i32 %569, 9
  %571 = mul i32 %570, %570
  %572 = getelementptr inbounds nuw [4 x i8], ptr @$var_im, i16 %563
  %573 = load i32, ptr %572, !tbaa !2
  %574 = call i32 @llvm.smul.fix.i32(i32 %573, i32 2048, i32 9)
  %575 = icmp slt i32 %574, 0
  %576 = select i1 %575, i32 511, i32 0
  %577 = add i32 %574, %576
  %578 = ashr i32 %577, 9
  %579 = mul i32 %578, %578
  %580 = add i32 %571, %579
  %581 = srem i32 %580, 256
  %582 = ashr i32 %581, 31
  %583 = sdiv i32 %580, 256
  %584 = add i32 %582, %583
  %585 = icmp sgt i32 %584, 65535
  %spec.store.select = select i1 %585, i32 65535, i32 %584
  %586 = trunc i32 %spec.store.select to i16
  %587 = getelementptr inbounds nuw [2 x i8], ptr @$var_row, i16 %563
  store i16 %586, ptr %587, !tbaa !2
  %588 = mul nuw nsw i32 %527, 3
  %589 = add nsw i32 %588, %spec.store.select
  %590 = add i32 %589, %526
  %591 = and i32 %590, 536870911
  %592 = add i32 %526, 1
  %593 = add nuw nsw i32 %528, 1
  br label %b110

b123:
  br label %594

594:
  %595 = load i16, ptr addrspace(1) %530
  %596 = call addrspace(1) i16 @LL$CLOSE(i16 %595)
  br label %597

597:
  br label %598

598:
  %599 = load i16, ptr addrspace(1) %254
  %600 = call addrspace(1) i16 @LL$CLOSE(i16 %599)
  br label %601

601:
  br label %common.ret

b125:
  br label %b79
}

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{i8 0, i8 2}
!6 = !{i8 0, i8 2}
!7 = !{i8 0, i8 2}
!8 = !{i8 0, i8 2}
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
