target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"
@$str2 = internal constant [11 x i8] c"\08\00\04\00\04\00bolt\00"
@$str3 = internal constant [11 x i8] c"\08\00\04\00\04\00gear\00"
@$str4 = internal constant [10 x i8] c"\08\00\03\00\03\00cog\00"
@$str5 = internal constant [10 x i8] c"\08\00\03\00\03\00pin\00"
@$str6 = internal constant [9 x i8] c"\08\00\02\00\02\00: \00"
@$str7 = internal constant [11 x i8] c"\08\00\04\00\04\00 at \00"
@$str8 = internal constant [13 x i8] c"\08\00\06\00\06\00no pin\00"
@$str9 = internal constant [21 x i8] c"\08\00\0E\00\0E\00cheapest gear \00"
@$str10 = internal constant [14 x i8] c"\08\00\07\00\07\00no gear\00"
@$str11 = internal constant [24 x i8] c"\08\00\11\00\11\00 under 20, first \00"
@$str12 = internal constant [12 x i8] c"\08\00\05\00\05\00low: \00"
@$str13 = internal constant [9 x i8] c"\08\00\02\00\02\00 (\00"
@$str14 = internal constant [8 x i8] c"\08\00\01\00\01\00)\00"
@$str15 = internal constant [10 x i8] c"\08\00\03\00\03\00nut\00"
@$str16 = internal constant [13 x i8] c"\08\00\06\00\06\00 parts\00"

define internal cc 1006 void @find(ptr addrspace(5) nocapture writeonly %0, ptr %1, ptr %2, ptr addrspace(5) readonly %3) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" memory(read, argmem: readwrite) {
b1:
  %4 = addrspacecast ptr addrspace(5) %3 to ptr addrspace(1)
  %5 = alloca [8 x i8]
  %6 = alloca [8 x i8]
  %7 = getelementptr i8, ptr %1, i16 -4
  %8 = load i16, ptr %7
  %9 = icmp eq i16 %8, 0
  br i1 %9, label %b5, label %10

10:
  %11 = addrspacecast ptr %6 to ptr addrspace(1)
  br label %b3

b3:
  %12 = phi ptr [ %30, %b8 ], [ %1, %10 ]
  %13 = phi i16 [ %29, %b8 ], [ %8, %10 ]
  %14 = load ptr, ptr %12
  %15 = getelementptr i8, ptr %14, i16 -4
  %16 = load i16, ptr %15
  %17 = addrspacecast ptr %14 to ptr addrspace(1)
  store i16 %16, ptr %6, !tbaa !2
  %18 = getelementptr i8, ptr %6, i16 2
  store i16 %16, ptr %18, !tbaa !2
  %19 = getelementptr i8, ptr %6, i16 4
  store ptr addrspace(1) %17, ptr %19, !tbaa !2
  %20 = call cc 1005 addrspace(1) i8 @N$VCMP(ptr addrspace(1) %11, ptr addrspace(1) %4)
  %21 = icmp eq i8 %20, 0
  br i1 %21, label %b6, label %b8

b5:
  %22 = getelementptr i8, ptr %2, i16 -4
  %23 = load i16, ptr %22
  %24 = icmp eq i16 %23, 0
  br i1 %24, label %b16, label %25

25:
  %26 = addrspacecast ptr %5 to ptr addrspace(1)
  br label %b14

b6:
  %27 = addrspacecast ptr %12 to ptr addrspace(1)
  store i8 0, ptr addrspace(5) %0
  %28 = getelementptr i8, ptr addrspace(5) %0, i16 2
  store ptr addrspace(1) %27, ptr addrspace(5) %28
  br label %common.ret

b8:
  %29 = add i16 %13, -1
  %30 = getelementptr i8, ptr %12, i16 6
  %exitcond3 = icmp eq i16 %29, 0
  br i1 %exitcond3, label %b5, label %b3

common.ret:
  ret void

b14:
  %31 = phi ptr [ %44, %b19 ], [ %2, %25 ]
  %32 = phi i16 [ %43, %b19 ], [ %23, %25 ]
  %33 = load ptr, ptr %31
  %34 = getelementptr i8, ptr %33, i16 -4
  %35 = load i16, ptr %34
  %36 = addrspacecast ptr %33 to ptr addrspace(1)
  store i16 %35, ptr %5, !tbaa !2
  %37 = getelementptr i8, ptr %5, i16 2
  store i16 %35, ptr %37, !tbaa !2
  %38 = getelementptr i8, ptr %5, i16 4
  store ptr addrspace(1) %36, ptr %38, !tbaa !2
  %39 = call cc 1005 addrspace(1) i8 @N$VCMP(ptr addrspace(1) %26, ptr addrspace(1) %4)
  %40 = icmp eq i8 %39, 0
  br i1 %40, label %b17, label %b19

b16:
  store i8 1, ptr addrspace(5) %0
  br label %common.ret

b17:
  %41 = addrspacecast ptr %31 to ptr addrspace(1)
  store i8 0, ptr addrspace(5) %0
  %42 = getelementptr i8, ptr addrspace(5) %0, i16 2
  store ptr addrspace(1) %41, ptr addrspace(5) %42
  br label %common.ret

b19:
  %43 = add i16 %32, -1
  %44 = getelementptr i8, ptr %31, i16 6
  %exitcond1 = icmp eq i16 %43, 0
  br i1 %exitcond1, label %b16, label %b14
}

define cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
  %1 = alloca [8 x i8]
  %2 = alloca [8 x i8]
  %3 = alloca [8 x i8]
  %4 = alloca [8 x i8]
  %5 = alloca [8 x i8]
  %6 = alloca [8 x i8]
  %7 = alloca [8 x i8]
  %8 = alloca [8 x i8]
  %9 = alloca [6 x i8]
  %10 = alloca [8 x i8]
  %11 = alloca [6 x i8]
  %12 = alloca [8 x i8]
  %13 = alloca [6 x i8]
  call void @llvm.lifetime.start.p0(i64 8, ptr nonnull %5)
  call void @llvm.lifetime.start.p0(i64 8, ptr nonnull %4)
  call void @llvm.lifetime.start.p0(i64 8, ptr nonnull %3)
  store i32 262148, ptr %5
  %14 = getelementptr inbounds nuw i8, ptr %5, i16 4
  store ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str2, i16 6) to ptr addrspace(1)), ptr %14, !tbaa !2
  %15 = addrspacecast ptr %5 to ptr addrspace(1)
  %16 = tail call cc 1005 addrspace(1) ptr @N$BGRW(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6), i16 1, i16 6)
  %17 = call cc 1005 addrspace(1) ptr @N$VCPY(ptr addrspace(1) %15)
  store ptr %17, ptr %16
  %18 = getelementptr i8, ptr %16, i16 2
  store i16 5, ptr %18
  %19 = getelementptr i8, ptr %16, i16 4
  store i16 40, ptr %19
  store i32 262148, ptr %4
  %20 = getelementptr inbounds nuw i8, ptr %4, i16 4
  store ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str3, i16 6) to ptr addrspace(1)), ptr %20, !tbaa !2
  %21 = addrspacecast ptr %4 to ptr addrspace(1)
  %22 = getelementptr i8, ptr %16, i16 -4
  %23 = load i16, ptr %22
  %24 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %16, i16 1, i16 6)
  %25 = mul i16 %23, 6
  %26 = getelementptr inbounds i8, ptr %24, i16 %25
  %27 = call cc 1005 addrspace(1) ptr @N$VCPY(ptr addrspace(1) %21)
  store ptr %27, ptr %26
  %28 = getelementptr i8, ptr %26, i16 2
  store i16 30, ptr %28
  %29 = getelementptr i8, ptr %26, i16 4
  store i16 3, ptr %29
  store i32 196611, ptr %3
  %30 = getelementptr inbounds nuw i8, ptr %3, i16 4
  store ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str4, i16 6) to ptr addrspace(1)), ptr %30, !tbaa !2
  %31 = addrspacecast ptr %3 to ptr addrspace(1)
  %32 = getelementptr i8, ptr %24, i16 -4
  %33 = load i16, ptr %32
  %34 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %24, i16 1, i16 6)
  %35 = mul i16 %33, 6
  %36 = getelementptr inbounds i8, ptr %34, i16 %35
  %37 = call cc 1005 addrspace(1) ptr @N$VCPY(ptr addrspace(1) %31)
  store ptr %37, ptr %36
  %38 = getelementptr i8, ptr %36, i16 2
  store i16 12, ptr %38
  %39 = getelementptr i8, ptr %36, i16 4
  store i16 0, ptr %39
  call cc 1005 addrspace(1) void @N$BDRP(ptr null)
  call void @llvm.lifetime.end.p0(i64 8, ptr nonnull %3)
  call void @llvm.lifetime.end.p0(i64 8, ptr nonnull %4)
  call void @llvm.lifetime.end.p0(i64 8, ptr nonnull %5)
  call void @llvm.lifetime.start.p0(i64 8, ptr nonnull %2)
  call void @llvm.lifetime.start.p0(i64 8, ptr nonnull %1)
  store i32 262148, ptr %2
  %40 = getelementptr inbounds nuw i8, ptr %2, i16 4
  store ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str3, i16 6) to ptr addrspace(1)), ptr %40, !tbaa !2
  %41 = addrspacecast ptr %2 to ptr addrspace(1)
  %42 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6), i16 1, i16 6)
  %43 = call cc 1005 addrspace(1) ptr @N$VCPY(ptr addrspace(1) %41)
  store ptr %43, ptr %42
  %44 = getelementptr i8, ptr %42, i16 2
  store i16 28, ptr %44
  %45 = getelementptr i8, ptr %42, i16 4
  store i16 9, ptr %45
  store i32 196611, ptr %1
  %46 = getelementptr inbounds nuw i8, ptr %1, i16 4
  store ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str5, i16 6) to ptr addrspace(1)), ptr %46, !tbaa !2
  %47 = addrspacecast ptr %1 to ptr addrspace(1)
  %48 = getelementptr i8, ptr %42, i16 -4
  %49 = load i16, ptr %48
  %50 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %42, i16 1, i16 6)
  %51 = mul i16 %49, 6
  %52 = getelementptr inbounds i8, ptr %50, i16 %51
  %53 = call cc 1005 addrspace(1) ptr @N$VCPY(ptr addrspace(1) %47)
  store ptr %53, ptr %52
  %54 = getelementptr i8, ptr %52, i16 2
  store i16 2, ptr %54
  %55 = getelementptr i8, ptr %52, i16 4
  store i16 100, ptr %55
  call cc 1005 addrspace(1) void @N$BDRP(ptr null)
  call void @llvm.lifetime.end.p0(i64 8, ptr nonnull %1)
  call void @llvm.lifetime.end.p0(i64 8, ptr nonnull %2)
  %56 = addrspacecast ptr %13 to ptr addrspace(5)
  store i32 196611, ptr %12
  %57 = getelementptr inbounds nuw i8, ptr %12, i16 4
  store ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str5, i16 6) to ptr addrspace(1)), ptr %57, !tbaa !2
  %58 = addrspacecast ptr %12 to ptr addrspace(5)
  call cc 1006 addrspace(1) void @find(ptr addrspace(5) %56, ptr %34, ptr %50, ptr addrspace(5) %58)
  %59 = load i8, ptr %13, !tbaa !2, !range !6
  %60 = icmp eq i8 %59, 0
  br i1 %60, label %b4, label %b3

b2:
  call cc 1005 addrspace(1) void @N$PN()
  %61 = addrspacecast ptr %11 to ptr addrspace(5)
  store i32 262148, ptr %10
  %62 = getelementptr inbounds nuw i8, ptr %10, i16 4
  store ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str3, i16 6) to ptr addrspace(1)), ptr %62, !tbaa !2
  %63 = addrspacecast ptr %10 to ptr addrspace(5)
  call cc 1006 addrspace(1) void @find(ptr addrspace(5) %61, ptr nonnull %34, ptr nonnull %50, ptr addrspace(5) %63)
  %64 = addrspacecast ptr %9 to ptr addrspace(5)
  store i32 262148, ptr %8
  %65 = getelementptr inbounds nuw i8, ptr %8, i16 4
  store ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str3, i16 6) to ptr addrspace(1)), ptr %65, !tbaa !2
  %66 = addrspacecast ptr %8 to ptr addrspace(5)
  call cc 1006 addrspace(1) void @find(ptr addrspace(5) %64, ptr nonnull %50, ptr nonnull %34, ptr addrspace(5) %66)
  %copyload = load i8, ptr %11, align 1
  %67 = icmp eq i8 %copyload, 0
  %68 = load i8, ptr %9, align 1
  %69 = icmp eq i8 %68, 0
  %or.cond = select i1 %67, i1 %69, i1 false
  br i1 %or.cond, label %93, label %b7

b3:
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str8, i16 6))
  br label %b2

b4:
  %70 = getelementptr inbounds nuw i8, ptr %13, i16 2
  %71 = load ptr addrspace(1), ptr %70, !tbaa !2
  %72 = load ptr, ptr addrspace(1) %71
  call cc 1005 addrspace(1) void @N$PS(ptr %72)
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str6, i16 6))
  %73 = getelementptr i8, ptr addrspace(1) %71, i16 4
  %74 = load i16, ptr addrspace(1) %73
  call cc 1005 addrspace(1) void @N$PU2(i16 %74)
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str7, i16 6))
  %75 = getelementptr i8, ptr addrspace(1) %71, i16 2
  %76 = load i16, ptr addrspace(1) %75
  call cc 1005 addrspace(1) void @N$PU2(i16 %76)
  br label %b2

77:
  call cc 1005 addrspace(1) void @N$PN()
  %78 = getelementptr i8, ptr %34, i16 -4
  %79 = load i16, ptr %78
  %80 = icmp eq i16 %79, 0
  br i1 %80, label %b12, label %81

81:
  br label %87

82:
  %83 = add nuw i16 %89, 1
  %84 = add i16 %88, 6
  %exitcond = icmp eq i16 %79, %83
  br i1 %exitcond, label %102, label %87

85:
  %86 = icmp eq i16 %89, 0
  br i1 %86, label %b12, label %102

87:
  %88 = phi i16 [ 2, %81 ], [ %84, %82 ]
  %89 = phi i16 [ %83, %82 ], [ 0, %81 ]
  %90 = getelementptr i8, ptr %34, i16 %88
  %91 = load i16, ptr %90
  %92 = icmp ult i16 %91, 21
  br i1 %92, label %82, label %85

b7:
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str10, i16 6))
  br label %77

93:
  %sroa_idx3 = getelementptr inbounds nuw i8, ptr %9, i16 2
  %copyload5 = load ptr addrspace(1), ptr %sroa_idx3, align 1
  %sroa_idx1 = getelementptr inbounds nuw i8, ptr %11, i16 2
  %copyload2 = load ptr addrspace(1), ptr %sroa_idx1, align 1
  %94 = getelementptr inbounds nuw i8, ptr addrspace(1) %copyload2, i16 2
  %95 = load i16, ptr addrspace(1) %94
  %96 = getelementptr inbounds nuw i8, ptr addrspace(1) %copyload5, i16 2
  %97 = load i16, ptr addrspace(1) %96
  %98 = icmp ugt i16 %95, %97
  %99 = select i1 %98, ptr addrspace(1) %copyload5, ptr addrspace(1) %copyload2
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str9, i16 6))
  %100 = getelementptr i8, ptr addrspace(1) %99, i16 2
  %101 = load i16, ptr addrspace(1) %100
  call cc 1005 addrspace(1) void @N$PU2(i16 %101)
  br label %77

102:
  %103 = phi i16 [ %89, %85 ], [ %79, %82 ]
  %104 = load ptr, ptr %34
  %105 = getelementptr i8, ptr %104, i16 -4
  %106 = load i16, ptr %105
  %107 = icmp eq i16 %106, 0
  br i1 %107, label %108, label %109

108:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

109:
  %110 = addrspacecast ptr %7 to ptr addrspace(5)
  %111 = addrspacecast ptr %7 to ptr addrspace(1)
  %112 = addrspacecast ptr %104 to ptr addrspace(1)
  store i32 65537, ptr addrspace(5) %110
  %113 = getelementptr inbounds nuw i8, ptr addrspace(5) %110, i16 4
  store ptr addrspace(1) %112, ptr addrspace(5) %113
  call cc 1005 addrspace(1) void @N$PU2(i16 %103)
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str11, i16 6))
  call cc 1005 addrspace(1) void @N$PV(ptr addrspace(1) %111)
  call cc 1005 addrspace(1) void @N$PN()
  %114 = getelementptr i8, ptr %34, i16 -4
  %115 = load i16, ptr %114
  %116 = icmp eq i16 %115, 0
  br i1 %116, label %123, label %117

117:
  %118 = getelementptr i8, ptr %34, i16 4
  br label %b15

b12:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b15:
  %119 = phi ptr [ %118, %117 ], [ %141, %b20 ]
  %120 = phi i16 [ %115, %117 ], [ %140, %b20 ]
  %121 = load i16, ptr %119
  %122 = icmp ult i16 %121, 5
  br i1 %122, label %b18, label %b20

123:
  store i32 196611, ptr %6
  %124 = getelementptr inbounds nuw i8, ptr %6, i16 4
  store ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str15, i16 6) to ptr addrspace(1)), ptr %124, !tbaa !2
  %125 = addrspacecast ptr %6 to ptr addrspace(1)
  %126 = getelementptr i8, ptr %34, i16 -4
  %127 = load i16, ptr %126
  %128 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr nonnull %34, i16 1, i16 6)
  %129 = mul i16 %127, 6
  %130 = getelementptr inbounds i8, ptr %128, i16 %129
  %131 = call cc 1005 addrspace(1) ptr @N$VCPY(ptr addrspace(1) %125)
  store ptr %131, ptr %130
  %132 = getelementptr i8, ptr %130, i16 2
  store i16 1, ptr %132
  %133 = getelementptr i8, ptr %130, i16 4
  store i16 500, ptr %133
  %134 = getelementptr i8, ptr %128, i16 -4
  %135 = load i16, ptr %134
  call cc 1005 addrspace(1) void @N$PU2(i16 %135)
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str16, i16 6))
  call cc 1005 addrspace(1) void @N$PN()
  %136 = icmp eq ptr %50, null
  br i1 %136, label %b28, label %b23

b18:
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str12, i16 6))
  %137 = getelementptr i8, ptr %119, i16 -4
  %138 = load ptr, ptr %137
  call cc 1005 addrspace(1) void @N$PS(ptr %138)
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str13, i16 6))
  %139 = load i16, ptr %119
  call cc 1005 addrspace(1) void @N$PU2(i16 %139)
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str14, i16 6))
  call cc 1005 addrspace(1) void @N$PN()
  br label %b20

b20:
  %140 = add i16 %120, -1
  %141 = getelementptr i8, ptr %119, i16 6
  %exitcond3 = icmp eq i16 %140, 0
  br i1 %exitcond3, label %123, label %b15

b23:
  %142 = getelementptr i8, ptr %50, i16 -4
  %143 = load i16, ptr %142
  %144 = icmp eq i16 %143, 0
  br i1 %144, label %b28, label %145

145:
  br label %b26

b26:
  %146 = phi ptr [ %50, %145 ], [ %150, %b26 ]
  %147 = phi i16 [ %143, %145 ], [ %149, %b26 ]
  %148 = load ptr, ptr %146
  call cc 1005 addrspace(1) void @N$BDRP(ptr %148)
  %149 = add i16 %147, -1
  %150 = getelementptr i8, ptr %146, i16 6
  %exitcond2 = icmp eq i16 %149, 0
  br i1 %exitcond2, label %b28, label %b26

b27:
  call cc 1005 addrspace(1) void @N$BDRP(ptr nonnull %128)
  ret i16 0

b28:
  call cc 1005 addrspace(1) void @N$BDRP(ptr %50)
  %151 = getelementptr i8, ptr %128, i16 -4
  %152 = load i16, ptr %151
  %153 = icmp eq i16 %152, 0
  br i1 %153, label %b27, label %154

154:
  br label %b31

b31:
  %155 = phi ptr [ %128, %154 ], [ %159, %b31 ]
  %156 = phi i16 [ %152, %154 ], [ %158, %b31 ]
  %157 = load ptr, ptr %155
  call cc 1005 addrspace(1) void @N$BDRP(ptr %157)
  %158 = add i16 %156, -1
  %159 = getelementptr i8, ptr %155, i16 6
  %exitcond1 = icmp eq i16 %158, 0
  br i1 %exitcond1, label %b27, label %b31
}

declare cc 1005 ptr @N$BGRW(ptr, i16, i16) addrspace(1)

declare cc 1005 ptr @N$VCPY(ptr addrspace(1)) addrspace(1)

declare cc 1005 void @N$BDRP(ptr) addrspace(1)

declare cc 1005 i8 @N$VCMP(ptr addrspace(1), ptr addrspace(1)) addrspace(1) memory(read)

declare cc 1005 void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

declare void @llvm.memcpy.p0.p0.i16(ptr noalias nocapture writeonly, ptr noalias nocapture readonly, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PU2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PV(ptr addrspace(1)) addrspace(1)

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{i16 0, i16 10923}
!6 = !{i8 0, i8 2}
