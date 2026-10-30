target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$var_heap.heads = internal global [26 x i8] zeroinitializer, align 2
@$var_heap.occupied = internal global [2 x i8] zeroinitializer, align 2
@$var_heap.end = internal global [2 x i8] zeroinitializer, align 2
@$var_format.width = internal global [1 x i8] zeroinitializer
@$var_format.radix = internal global [1 x i8] c"\0A"
@$var_format.fill = internal global [1 x i8] c" "
@$var_format.left = internal global [1 x i8] zeroinitializer
@$var_format.sink = internal global [2 x i8] zeroinitializer, align 2
@$var_format.scratch = internal global [48 x i8] zeroinitializer
@$str10 = internal constant [14 x i8] c"\08\00\07\00\07\00panic: \00"
@$str11 = internal constant [9 x i8] c"\08\00\02\00\02\00\0D\0A\00"
@$str12 = internal constant [35 x i8] c"\08\00\1C\00\1C\00division by zero or overflow\00"
@$str13 = internal constant [21 x i8] c"\08\00\0E\00\0E\00stack overflow\00"
@$str14 = internal constant [37 x i8] c"\08\00\1E\00\1E\00float outside the integer type\00"
@$str15 = internal constant [31 x i8] c"\08\00\18\00\18\00shift count out of range\00"
@$str16 = internal constant [26 x i8] c"\08\00\13\00\13\00index out of bounds\00"
@$str17 = internal constant [20 x i8] c"\08\00\0D\00\0D\00key not found\00"
@$str18 = internal constant [20 x i8] c"\08\00\0D\00\0D\00out of memory\00"
@$str19 = internal constant [28 x i8] c"\08\00\15\00\15\00pop from an empty vec\00"
@$str20 = internal constant [11 x i8] c"\08\00\04\00\04\00true\00"
@$str21 = internal constant [12 x i8] c"\08\00\05\00\05\00false\00"
@$str22 = internal constant [10 x i8] c"\08\00\03\00\03\00inf\00"
@$str23 = internal constant [10 x i8] c"\08\00\03\00\03\00nan\00"

define cc 1005 void @N$EDIV() addrspace(1) "llrm-fp-eval"="x87-extended" {
  %1 = tail call addrspace(1) i32 @LL$WRITE_FILE(i16 1, ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str10, i16 6) to ptr addrspace(1)), i16 7)
  %2 = tail call addrspace(1) i32 @LL$WRITE_FILE(i16 1, ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str12, i16 6) to ptr addrspace(1)), i16 28)
  %3 = tail call addrspace(1) i32 @LL$WRITE_FILE(i16 1, ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str11, i16 6) to ptr addrspace(1)), i16 2)
  tail call addrspace(1) void @LL$EXIT(i8 zeroext -1)
  ret void
}

define cc 1005 void @N$ESTK() addrspace(1) "llrm-fp-eval"="x87-extended" {
  %1 = tail call addrspace(1) i32 @LL$WRITE_FILE(i16 1, ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str10, i16 6) to ptr addrspace(1)), i16 7)
  %2 = tail call addrspace(1) i32 @LL$WRITE_FILE(i16 1, ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str13, i16 6) to ptr addrspace(1)), i16 14)
  %3 = tail call addrspace(1) i32 @LL$WRITE_FILE(i16 1, ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str11, i16 6) to ptr addrspace(1)), i16 2)
  tail call addrspace(1) void @LL$EXIT(i8 zeroext -1)
  ret void
}

define cc 1005 void @N$ECNV() addrspace(1) "llrm-fp-eval"="x87-extended" {
  %1 = tail call addrspace(1) i32 @LL$WRITE_FILE(i16 1, ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str10, i16 6) to ptr addrspace(1)), i16 7)
  %2 = tail call addrspace(1) i32 @LL$WRITE_FILE(i16 1, ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str14, i16 6) to ptr addrspace(1)), i16 30)
  %3 = tail call addrspace(1) i32 @LL$WRITE_FILE(i16 1, ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str11, i16 6) to ptr addrspace(1)), i16 2)
  tail call addrspace(1) void @LL$EXIT(i8 zeroext -1)
  ret void
}

define cc 1005 void @N$ESHF() addrspace(1) "llrm-fp-eval"="x87-extended" {
  %1 = tail call addrspace(1) i32 @LL$WRITE_FILE(i16 1, ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str10, i16 6) to ptr addrspace(1)), i16 7)
  %2 = tail call addrspace(1) i32 @LL$WRITE_FILE(i16 1, ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str15, i16 6) to ptr addrspace(1)), i16 24)
  %3 = tail call addrspace(1) i32 @LL$WRITE_FILE(i16 1, ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str11, i16 6) to ptr addrspace(1)), i16 2)
  tail call addrspace(1) void @LL$EXIT(i8 zeroext -1)
  ret void
}

define cc 1005 void @N$EBND() addrspace(1) "llrm-fp-eval"="x87-extended" {
  %1 = tail call addrspace(1) i32 @LL$WRITE_FILE(i16 1, ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str10, i16 6) to ptr addrspace(1)), i16 7)
  %2 = tail call addrspace(1) i32 @LL$WRITE_FILE(i16 1, ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str16, i16 6) to ptr addrspace(1)), i16 19)
  %3 = tail call addrspace(1) i32 @LL$WRITE_FILE(i16 1, ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str11, i16 6) to ptr addrspace(1)), i16 2)
  tail call addrspace(1) void @LL$EXIT(i8 zeroext -1)
  ret void
}

define cc 1005 void @N$EKEY() addrspace(1) "llrm-fp-eval"="x87-extended" {
  %1 = tail call addrspace(1) i32 @LL$WRITE_FILE(i16 1, ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str10, i16 6) to ptr addrspace(1)), i16 7)
  %2 = tail call addrspace(1) i32 @LL$WRITE_FILE(i16 1, ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str17, i16 6) to ptr addrspace(1)), i16 13)
  %3 = tail call addrspace(1) i32 @LL$WRITE_FILE(i16 1, ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str11, i16 6) to ptr addrspace(1)), i16 2)
  tail call addrspace(1) void @LL$EXIT(i8 zeroext -1)
  ret void
}

define internal cc 1005 ptr @heap.allocate(i16 %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
  %2 = icmp ugt i16 %0, -16
  br i1 %2, label %3, label %7

3:
  %4 = tail call addrspace(1) i32 @LL$WRITE_FILE(i16 1, ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str10, i16 6) to ptr addrspace(1)), i16 7)
  %5 = tail call addrspace(1) i32 @LL$WRITE_FILE(i16 1, ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str18, i16 6) to ptr addrspace(1)), i16 13)
  %6 = tail call addrspace(1) i32 @LL$WRITE_FILE(i16 1, ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str11, i16 6) to ptr addrspace(1)), i16 2)
  tail call addrspace(1) void @LL$EXIT(i8 zeroext -1)
  br label %7

7:
  %8 = add i16 %0, 5
  %9 = and i16 %8, -4
  %10 = tail call i16 @llvm.umax.i16(i16 %9, i16 8)
  %11 = lshr i16 %10, 4
  %12 = icmp eq i16 %11, 0
  br i1 %12, label %20, label %13

13:
  br label %14

14:
  %15 = phi i16 [ %17, %14 ], [ 0, %13 ]
  %16 = phi i16 [ %18, %14 ], [ %11, %13 ]
  %17 = add nuw nsw i16 %15, 1
  %18 = lshr i16 %16, 1
  %19 = icmp eq i16 %18, 0
  br i1 %19, label %20, label %14

20:
  %21 = phi i16 [ 0, %7 ], [ %17, %14 ]
  %22 = getelementptr inbounds [2 x i8], ptr @$var_heap.heads, i16 %21
  %23 = load ptr, ptr %22, !tbaa !2
  %24 = icmp eq ptr %23, null
  br i1 %24, label %31, label %25

25:
  br label %26

26:
  %27 = phi ptr [ %35, %33 ], [ %23, %25 ]
  %28 = load i16, ptr %27
  %29 = and i16 %28, -4
  %30 = icmp ult i16 %29, %10
  br i1 %30, label %33, label %150

31:
  %32 = icmp ult i16 %21, 16
  br i1 %32, label %37, label %45

33:
  %34 = getelementptr i8, ptr %27, i16 2
  %35 = load ptr, ptr %34
  %36 = icmp eq ptr %35, null
  br i1 %36, label %31, label %26

37:
  %38 = load i16, ptr @$var_heap.occupied, !tbaa !2
  %39 = shl i16 2, %21
  %40 = add i16 %39, -1
  %41 = xor i16 %40, -1
  %42 = and i16 %38, %41
  %43 = icmp eq i16 %42, 0
  br i1 %43, label %62, label %44

44:
  br label %46

45:
  tail call cc 1005 addrspace(1) void @N$ESHF()
  unreachable

46:
  %47 = phi ptr [ @$var_heap.heads, %44 ], [ %54, %46 ]
  %48 = phi i16 [ %51, %46 ], [ 0, %44 ]
  %49 = phi i16 [ %50, %46 ], [ %42, %44 ]
  %50 = lshr exact i16 %49, 1
  %51 = add i16 %48, 1
  %52 = and i16 %49, 2
  %53 = icmp eq i16 %52, 0
  %54 = getelementptr i8, ptr %47, i16 2
  br i1 %53, label %46, label %55

55:
  %56 = icmp ult i16 %51, 13
  br i1 %56, label %58, label %57

57:
  tail call cc 1005 addrspace(1) void @N$EBND()
  unreachable

58:
  %59 = getelementptr i8, ptr %47, i16 2
  %60 = load ptr, ptr %59, !tbaa !2
  %61 = icmp eq ptr %60, null
  br i1 %61, label %62, label %150

62:
  %63 = add i16 %10, 4
  %64 = tail call i16 @llvm.umax.i16(i16 %63, i16 1024)
  %65 = load ptr, ptr @$var_heap.end, !tbaa !2
  %66 = icmp eq ptr %65, null
  %67 = zext i1 %66 to i8
  %68 = select i1 %66, i16 2, i16 0
  %69 = or disjoint i16 %68, %64
  %70 = tail call addrspace(1) ptr @LL$MORE(i16 %69)
  %71 = icmp eq ptr %70, null
  br i1 %71, label %72, label %76

72:
  %73 = or disjoint i16 %68, %63
  %74 = tail call addrspace(1) ptr @LL$MORE(i16 %73)
  %75 = icmp eq ptr %74, null
  br i1 %75, label %99, label %76

76:
  %77 = phi ptr [ %74, %72 ], [ %70, %62 ]
  %78 = phi i16 [ %63, %72 ], [ %64, %62 ]
  %79 = icmp eq ptr %65, null
  %80 = xor i8 %67, 1
  %81 = icmp eq i8 %80, 0
  %82 = load ptr, ptr @$var_heap.end
  %83 = getelementptr i8, ptr %82, i16 2
  %84 = icmp ne ptr %77, %83
  %85 = xor i1 %81, true
  %86 = select i1 %85, i1 %84, i1 false
  %87 = select i1 %79, i1 true, i1 %86
  %88 = select i1 %87, ptr %77, ptr %82
  br i1 %87, label %89, label %90

89:
  store i16 2, ptr %77
  br label %90

90:
  %91 = add i16 %78, -4
  %92 = select i1 %86, i16 %91, i16 %78
  %93 = getelementptr i8, ptr %88, i16 %92
  store ptr %93, ptr @$var_heap.end, !tbaa !2
  store i16 3, ptr %93
  %94 = or i16 %92, 1
  %95 = load i16, ptr %88
  %96 = and i16 %95, 2
  %97 = or i16 %94, %96
  store i16 %97, ptr %88
  %98 = getelementptr i8, ptr %88, i16 2
  tail call cc 1005 addrspace(1) void @heap.release(ptr %98)
  br label %99

99:
  %100 = icmp eq i16 %11, 0
  br i1 %100, label %108, label %101

101:
  br label %102

102:
  %103 = phi i16 [ %105, %102 ], [ 0, %101 ]
  %104 = phi i16 [ %106, %102 ], [ %11, %101 ]
  %105 = add nuw nsw i16 %103, 1
  %106 = lshr i16 %104, 1
  %107 = icmp eq i16 %106, 0
  br i1 %107, label %108, label %102

108:
  %109 = phi i16 [ 0, %99 ], [ %105, %102 ]
  %110 = getelementptr inbounds [2 x i8], ptr @$var_heap.heads, i16 %109
  %111 = load ptr, ptr %110, !tbaa !2
  %112 = icmp eq ptr %111, null
  br i1 %112, label %119, label %113

113:
  br label %114

114:
  %115 = phi ptr [ %123, %121 ], [ %111, %113 ]
  %116 = load i16, ptr %115
  %117 = and i16 %116, -4
  %118 = icmp ult i16 %117, %10
  br i1 %118, label %121, label %150

119:
  %120 = icmp ult i16 %109, 16
  br i1 %120, label %125, label %133

121:
  %122 = getelementptr i8, ptr %115, i16 2
  %123 = load ptr, ptr %122
  %124 = icmp eq ptr %123, null
  br i1 %124, label %119, label %114

125:
  %126 = load i16, ptr @$var_heap.occupied, !tbaa !2
  %127 = shl i16 2, %109
  %128 = add i16 %127, -1
  %129 = xor i16 %128, -1
  %130 = and i16 %126, %129
  %131 = icmp eq i16 %130, 0
  br i1 %131, label %203, label %132

132:
  br label %134

133:
  tail call cc 1005 addrspace(1) void @N$ESHF()
  unreachable

134:
  %135 = phi ptr [ @$var_heap.heads, %132 ], [ %142, %134 ]
  %136 = phi i16 [ %139, %134 ], [ 0, %132 ]
  %137 = phi i16 [ %138, %134 ], [ %130, %132 ]
  %138 = lshr exact i16 %137, 1
  %139 = add i16 %136, 1
  %140 = and i16 %137, 2
  %141 = icmp eq i16 %140, 0
  %142 = getelementptr i8, ptr %135, i16 2
  br i1 %141, label %134, label %143

143:
  %144 = icmp ult i16 %139, 13
  br i1 %144, label %146, label %145

145:
  tail call cc 1005 addrspace(1) void @N$EBND()
  unreachable

146:
  %147 = getelementptr i8, ptr %135, i16 2
  %148 = load ptr, ptr %147, !tbaa !2
  %149 = icmp eq ptr %148, null
  br i1 %149, label %203, label %150

150:
  %151 = phi ptr [ %148, %146 ], [ %60, %58 ], [ %27, %26 ], [ %115, %114 ]
  %152 = getelementptr i8, ptr %151, i16 2
  %153 = load ptr, ptr %152
  %154 = getelementptr i8, ptr %151, i16 4
  %155 = load ptr, ptr %154
  %156 = icmp eq ptr %155, null
  br i1 %156, label %157, label %171

157:
  %158 = load i16, ptr %151
  %159 = lshr i16 %158, 4
  %160 = icmp eq i16 %159, 0
  br i1 %160, label %168, label %161

161:
  br label %162

162:
  %163 = phi i16 [ %165, %162 ], [ 0, %161 ]
  %164 = phi i16 [ %166, %162 ], [ %159, %161 ]
  %165 = add nuw nsw i16 %163, 1
  %166 = lshr i16 %164, 1
  %167 = icmp eq i16 %166, 0
  br i1 %167, label %168, label %162

168:
  %169 = phi i16 [ 0, %157 ], [ %165, %162 ]
  %170 = getelementptr inbounds [2 x i8], ptr @$var_heap.heads, i16 %169
  store ptr %153, ptr %170, !tbaa !2
  %magicptr = ptrtoint ptr %153 to i16
  %cond = icmp eq i16 %magicptr, 0
  br i1 %cond, label %175, label %180

171:
  %172 = getelementptr i8, ptr %155, i16 2
  store ptr %153, ptr %172
  br label %173

173:
  %174 = icmp eq ptr %153, null
  br i1 %174, label %182, label %180

175:
  %176 = load i16, ptr @$var_heap.occupied, !tbaa !2
  %177 = shl nuw i16 1, %169
  %178 = xor i16 %177, -1
  %179 = and i16 %176, %178
  store i16 %179, ptr @$var_heap.occupied, !tbaa !2
  br label %173

180:
  %181 = getelementptr i8, ptr %153, i16 4
  store ptr %155, ptr %181
  br label %182

182:
  %183 = load i16, ptr %151
  %184 = and i16 %183, -4
  %185 = or disjoint i16 %184, 1
  %186 = and i16 %183, 2
  %187 = or disjoint i16 %185, %186
  store i16 %187, ptr %151
  %188 = getelementptr i8, ptr %151, i16 %184
  %189 = load i16, ptr %188
  %190 = or i16 %189, 2
  store i16 %190, ptr %188
  %191 = load i16, ptr %151
  %192 = and i16 %191, -4
  %193 = sub i16 %192, %10
  %194 = icmp ult i16 %193, 8
  br i1 %194, label %202, label %195

195:
  %196 = getelementptr i8, ptr %151, i16 %10
  %197 = or disjoint i16 %193, 3
  store i16 %197, ptr %196
  %198 = load i16, ptr %151
  %199 = and i16 %198, 3
  %200 = or disjoint i16 %199, %10
  store i16 %200, ptr %151
  %201 = getelementptr i8, ptr %196, i16 2
  tail call cc 1005 addrspace(1) void @heap.release(ptr %201)
  br label %202

202:
  ret ptr %152

203:
  %204 = tail call addrspace(1) i32 @LL$WRITE_FILE(i16 1, ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str10, i16 6) to ptr addrspace(1)), i16 7)
  %205 = tail call addrspace(1) i32 @LL$WRITE_FILE(i16 1, ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str18, i16 6) to ptr addrspace(1)), i16 13)
  %206 = tail call addrspace(1) i32 @LL$WRITE_FILE(i16 1, ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str11, i16 6) to ptr addrspace(1)), i16 2)
  tail call addrspace(1) void @LL$EXIT(i8 zeroext -1)
  unreachable
}

define internal cc 1005 void @heap.release(ptr %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" memory(readwrite, inaccessiblemem: none) nounwind nofree nosync norecurse {
b1:
  %1 = icmp eq ptr %0, null
  br i1 %1, label %common.ret, label %2

common.ret:
  ret void

2:
  %3 = getelementptr i8, ptr %0, i16 -2
  %4 = load i16, ptr %3
  %5 = and i16 %4, -4
  %6 = getelementptr i8, ptr %3, i16 %5
  %7 = load i16, ptr %6
  %8 = and i16 %7, 1
  %9 = icmp eq i16 %8, 0
  br i1 %9, label %10, label %49

10:
  %11 = getelementptr i8, ptr %0, i16 %5
  %12 = load ptr, ptr %11
  %13 = getelementptr i8, ptr %0, i16 %5
  %14 = getelementptr i8, ptr %13, i16 2
  %15 = load ptr, ptr %14
  %16 = icmp eq ptr %15, null
  br i1 %16, label %17, label %30

17:
  %18 = lshr i16 %7, 4
  %19 = icmp eq i16 %18, 0
  br i1 %19, label %27, label %20

20:
  br label %21

21:
  %22 = phi i16 [ %24, %21 ], [ 0, %20 ]
  %23 = phi i16 [ %25, %21 ], [ %18, %20 ]
  %24 = add nuw nsw i16 %22, 1
  %25 = lshr i16 %23, 1
  %26 = icmp eq i16 %25, 0
  br i1 %26, label %27, label %21

27:
  %28 = phi i16 [ 0, %17 ], [ %24, %21 ]
  %29 = getelementptr inbounds [2 x i8], ptr @$var_heap.heads, i16 %28
  store ptr %12, ptr %29, !tbaa !2
  %magicptr1 = ptrtoint ptr %12 to i16
  %cond1 = icmp eq i16 %magicptr1, 0
  br i1 %cond1, label %34, label %39

30:
  %31 = getelementptr i8, ptr %15, i16 2
  store ptr %12, ptr %31
  br label %32

32:
  %33 = icmp eq ptr %12, null
  br i1 %33, label %41, label %39

34:
  %35 = load i16, ptr @$var_heap.occupied, !tbaa !2
  %36 = shl nuw i16 1, %28
  %37 = xor i16 %36, -1
  %38 = and i16 %35, %37
  store i16 %38, ptr @$var_heap.occupied, !tbaa !2
  br label %32

39:
  %40 = getelementptr i8, ptr %12, i16 4
  store ptr %15, ptr %40
  br label %41

41:
  %42 = getelementptr i8, ptr %0, i16 %5
  %43 = getelementptr i8, ptr %42, i16 -2
  %44 = load i16, ptr %43
  %45 = and i16 %44, -4
  %46 = add i16 %45, %5
  %47 = getelementptr i8, ptr %0, i16 -2
  %48 = load i16, ptr %47
  br label %49

49:
  %50 = phi i16 [ %48, %41 ], [ %4, %2 ]
  %51 = phi i16 [ %46, %41 ], [ %5, %2 ]
  %52 = and i16 %50, 2
  %53 = icmp eq i16 %52, 0
  br i1 %53, label %54, label %98

54:
  %55 = getelementptr i8, ptr %0, i16 -4
  %56 = load i16, ptr %55
  %57 = sub i16 0, %56
  %58 = getelementptr i8, ptr %3, i16 %57
  %59 = getelementptr i8, ptr %0, i16 %57
  %60 = load ptr, ptr %59
  %61 = getelementptr i8, ptr %0, i16 %57
  %62 = getelementptr i8, ptr %61, i16 2
  %63 = load ptr, ptr %62
  %64 = icmp eq ptr %63, null
  br i1 %64, label %65, label %81

65:
  %66 = getelementptr i8, ptr %0, i16 %57
  %67 = getelementptr i8, ptr %66, i16 -2
  %68 = load i16, ptr %67
  %69 = lshr i16 %68, 4
  %70 = icmp eq i16 %69, 0
  br i1 %70, label %78, label %71

71:
  br label %72

72:
  %73 = phi i16 [ %75, %72 ], [ 0, %71 ]
  %74 = phi i16 [ %76, %72 ], [ %69, %71 ]
  %75 = add nuw nsw i16 %73, 1
  %76 = lshr i16 %74, 1
  %77 = icmp eq i16 %76, 0
  br i1 %77, label %78, label %72

78:
  %79 = phi i16 [ 0, %65 ], [ %75, %72 ]
  %80 = getelementptr inbounds [2 x i8], ptr @$var_heap.heads, i16 %79
  store ptr %60, ptr %80, !tbaa !2
  %magicptr = ptrtoint ptr %60 to i16
  %cond = icmp eq i16 %magicptr, 0
  br i1 %cond, label %85, label %90

81:
  %82 = getelementptr i8, ptr %63, i16 2
  store ptr %60, ptr %82
  br label %83

83:
  %84 = icmp eq ptr %60, null
  br i1 %84, label %92, label %90

85:
  %86 = load i16, ptr @$var_heap.occupied, !tbaa !2
  %87 = shl nuw i16 1, %79
  %88 = xor i16 %87, -1
  %89 = and i16 %86, %88
  store i16 %89, ptr @$var_heap.occupied, !tbaa !2
  br label %83

90:
  %91 = getelementptr i8, ptr %60, i16 4
  store ptr %63, ptr %91
  br label %92

92:
  %93 = add i16 %56, %51
  %94 = getelementptr i8, ptr %0, i16 %57
  %95 = getelementptr i8, ptr %94, i16 -2
  %96 = load i16, ptr %95
  %97 = and i16 %96, 2
  br label %98

98:
  %99 = phi i16 [ %97, %92 ], [ %52, %49 ]
  %100 = phi i16 [ %93, %92 ], [ %51, %49 ]
  %101 = phi ptr [ %58, %92 ], [ %3, %49 ]
  %102 = or i16 %99, %100
  store i16 %102, ptr %101
  %103 = getelementptr i8, ptr %101, i16 %100
  %104 = getelementptr i8, ptr %103, i16 -2
  store i16 %100, ptr %104
  %105 = load i16, ptr %103
  %106 = and i16 %105, -3
  store i16 %106, ptr %103
  %107 = lshr i16 %100, 4
  %108 = icmp eq i16 %107, 0
  br i1 %108, label %116, label %109

109:
  br label %110

110:
  %111 = phi i16 [ %113, %110 ], [ 0, %109 ]
  %112 = phi i16 [ %114, %110 ], [ %107, %109 ]
  %113 = add nuw nsw i16 %111, 1
  %114 = lshr i16 %112, 1
  %115 = icmp eq i16 %114, 0
  br i1 %115, label %116, label %110

116:
  %117 = phi i16 [ 0, %98 ], [ %113, %110 ]
  %118 = getelementptr inbounds [2 x i8], ptr @$var_heap.heads, i16 %117
  %119 = load ptr, ptr %118, !tbaa !2
  %120 = getelementptr i8, ptr %101, i16 2
  store ptr %119, ptr %120
  %121 = getelementptr i8, ptr %101, i16 4
  store ptr null, ptr %121
  %122 = icmp eq ptr %119, null
  br i1 %122, label %125, label %123

123:
  %124 = getelementptr i8, ptr %119, i16 4
  store ptr %101, ptr %124
  br label %125

125:
  %126 = mul i16 %117, 2
  %127 = getelementptr i8, ptr @$var_heap.heads, i16 %126
  store ptr %101, ptr %127, !tbaa !2
  %128 = load i16, ptr @$var_heap.occupied, !tbaa !2
  %129 = shl nuw i16 1, %117
  %130 = or i16 %128, %129
  store i16 %130, ptr @$var_heap.occupied, !tbaa !2
  br label %common.ret
}

define internal cc 1005 ptr @buffers.allocate(i16 %0, i16 %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
  %3 = icmp eq i16 %1, 0
  br i1 %3, label %11, label %4

4:
  %5 = udiv i16 -23, %1
  %6 = icmp ugt i16 %0, %5
  br i1 %6, label %7, label %11

7:
  %8 = tail call addrspace(1) i32 @LL$WRITE_FILE(i16 1, ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str10, i16 6) to ptr addrspace(1)), i16 7)
  %9 = tail call addrspace(1) i32 @LL$WRITE_FILE(i16 1, ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str18, i16 6) to ptr addrspace(1)), i16 13)
  %10 = tail call addrspace(1) i32 @LL$WRITE_FILE(i16 1, ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str11, i16 6) to ptr addrspace(1)), i16 2)
  tail call addrspace(1) void @LL$EXIT(i8 zeroext -1)
  br label %11

11:
  %12 = mul i16 %1, %0
  %13 = add i16 %12, 7
  %14 = tail call cc 1005 addrspace(1) ptr @heap.allocate(i16 %13)
  %15 = getelementptr i8, ptr %14, i16 6
  store i8 1, ptr %14
  %16 = getelementptr i8, ptr %14, i16 1
  store i8 0, ptr %16
  %17 = getelementptr i8, ptr %14, i16 2
  store i16 0, ptr %17
  %18 = getelementptr i8, ptr %14, i16 4
  store i16 %0, ptr %18
  store i8 0, ptr %15, align 2
  ret ptr %15
}

define cc 1005 ptr @N$BRES(ptr %0, i16 %1, i16 %2) addrspace(1) "llrm-fp-eval"="x87-extended" {
  %4 = getelementptr i8, ptr %0, i16 -4
  %5 = load i16, ptr %4
  %6 = getelementptr i8, ptr %0, i16 -2
  %7 = load i16, ptr %6
  %8 = tail call i16 @llvm.umax.i16(i16 %1, i16 %5)
  %9 = getelementptr i8, ptr %0, i16 -6
  %10 = load i8, ptr %9, align 2
  %11 = zext i8 %10 to i16
  %12 = and i16 %11, 9
  %13 = icmp eq i16 %12, 1
  %not = xor i1 %13, true
  %14 = icmp ult i16 %7, %8
  %or.cond = select i1 %not, i1 true, i1 %14
  br i1 %or.cond, label %b7, label %common.ret

common.ret:
  %common.op = phi ptr [ %0, %b17 ], [ %0, %3 ], [ %31, %45 ], [ %31, %46 ]
  ret ptr %common.op

b7:
  %15 = icmp sgt i16 %7, -1
  %16 = zext i1 %15 to i16
  %17 = shl nuw i16 %7, %16
  %18 = icmp ult i16 %8, %17
  br i1 %18, label %19, label %b13

19:
  %20 = icmp eq i16 %2, 0
  br i1 %20, label %b13.thread, label %21

21:
  %22 = udiv i16 -23, %2
  %23 = icmp ugt i16 %17, %22
  %24 = select i1 %23, i16 %8, i16 %17
  br label %b13

b13:
  %25 = phi i16 [ %8, %b7 ], [ %24, %21 ]
  %26 = icmp eq i16 %12, 1
  br i1 %26, label %143, label %29

b13.thread:
  %27 = icmp eq i16 %12, 1
  br i1 %27, label %53, label %29

b17:
  %28 = getelementptr i8, ptr %0, i16 -2
  store i16 %54, ptr %28
  br label %common.ret

29:
  %30 = phi i16 [ %17, %b13.thread ], [ %54, %71 ], [ %54, %139 ], [ %25, %b13 ], [ %25, %145 ]
  %31 = tail call cc 1005 addrspace(1) ptr @buffers.allocate(i16 %30, i16 %2)
  %32 = mul i16 %5, %2
  %33 = add i16 %32, 1
  %34 = icmp eq i16 %33, 0
  br i1 %34, label %46, label %35

35:
  %36 = mul i16 %5, %2
  %37 = add i16 %36, 1
  br label %38

38:
  %39 = phi i16 [ %43, %38 ], [ 0, %35 ]
  %40 = getelementptr i8, ptr %31, i16 %39
  %41 = getelementptr i8, ptr %0, i16 %39
  %42 = load i8, ptr %41
  store i8 %42, ptr %40
  %43 = add nuw i16 %39, 1
  %44 = icmp eq i16 %37, %43
  br i1 %44, label %46, label %38

45:
  tail call cc 1005 addrspace(1) void @heap.release(ptr %9)
  br label %common.ret

46:
  %47 = getelementptr i8, ptr %31, i16 -4
  store i16 %5, ptr %47
  %48 = getelementptr i8, ptr %0, i16 -6
  %49 = load i8, ptr %48, align 2
  %50 = zext i8 %49 to i16
  %51 = and i16 %50, 1
  %52 = icmp eq i16 %51, 0
  br i1 %52, label %common.ret, label %45

53:
  %54 = phi i16 [ %25, %143 ], [ %25, %145 ], [ %17, %b13.thread ]
  %55 = mul i16 %54, %2
  %56 = getelementptr i8, ptr %0, i16 -8
  %57 = add i16 %55, 22
  %58 = icmp ult i16 %57, 15
  br i1 %58, label %59, label %63

59:
  %60 = tail call addrspace(1) i32 @LL$WRITE_FILE(i16 1, ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str10, i16 6) to ptr addrspace(1)), i16 7)
  %61 = tail call addrspace(1) i32 @LL$WRITE_FILE(i16 1, ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str18, i16 6) to ptr addrspace(1)), i16 13)
  %62 = tail call addrspace(1) i32 @LL$WRITE_FILE(i16 1, ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str11, i16 6) to ptr addrspace(1)), i16 2)
  tail call addrspace(1) void @LL$EXIT(i8 zeroext -1)
  br label %63

63:
  %64 = add i16 %55, 12
  %65 = and i16 %64, -4
  %66 = tail call i16 @llvm.umax.i16(i16 %65, i16 8)
  %67 = getelementptr i8, ptr %0, i16 -8
  %68 = load i16, ptr %67
  %69 = and i16 %68, -4
  %70 = icmp ult i16 %69, %66
  br i1 %70, label %71, label %77

71:
  %72 = getelementptr i8, ptr %0, i16 %69
  %73 = getelementptr i8, ptr %72, i16 -8
  %74 = load i16, ptr %73
  %75 = and i16 %74, 1
  %76 = icmp eq i16 %75, 0
  br i1 %76, label %139, label %29

77:
  %78 = phi i16 [ %138, %123 ], [ %69, %63 ]
  %79 = sub i16 %78, %66
  %80 = icmp ult i16 %79, 8
  br i1 %80, label %b17, label %81

81:
  %82 = getelementptr i8, ptr %56, i16 %66
  %83 = or disjoint i16 %79, 3
  %84 = getelementptr i8, ptr %0, i16 %66
  %85 = getelementptr i8, ptr %84, i16 -8
  store i16 %83, ptr %85
  %86 = getelementptr i8, ptr %0, i16 -8
  %87 = load i16, ptr %86
  %88 = and i16 %87, 3
  %89 = or disjoint i16 %88, %66
  store i16 %89, ptr %86
  %90 = getelementptr i8, ptr %82, i16 2
  tail call cc 1005 addrspace(1) void @heap.release(ptr %90)
  br label %b17

91:
  %92 = getelementptr i8, ptr %0, i16 %69
  %93 = getelementptr i8, ptr %92, i16 -6
  %94 = load ptr, ptr %93
  %95 = getelementptr i8, ptr %0, i16 %69
  %96 = getelementptr i8, ptr %95, i16 -4
  %97 = load ptr, ptr %96
  %98 = icmp eq ptr %97, null
  br i1 %98, label %99, label %112

99:
  %100 = lshr i16 %74, 4
  %101 = icmp eq i16 %100, 0
  br i1 %101, label %109, label %102

102:
  br label %103

103:
  %104 = phi i16 [ %106, %103 ], [ 0, %102 ]
  %105 = phi i16 [ %107, %103 ], [ %100, %102 ]
  %106 = add nuw nsw i16 %104, 1
  %107 = lshr i16 %105, 1
  %108 = icmp eq i16 %107, 0
  br i1 %108, label %109, label %103

109:
  %110 = phi i16 [ 0, %99 ], [ %106, %103 ]
  %111 = getelementptr inbounds [2 x i8], ptr @$var_heap.heads, i16 %110
  store ptr %94, ptr %111, !tbaa !2
  %magicptr = ptrtoint ptr %94 to i16
  %cond = icmp eq i16 %magicptr, 0
  br i1 %cond, label %116, label %121

112:
  %113 = getelementptr i8, ptr %97, i16 2
  store ptr %94, ptr %113
  br label %114

114:
  %115 = icmp eq ptr %94, null
  br i1 %115, label %123, label %121

116:
  %117 = load i16, ptr @$var_heap.occupied, !tbaa !2
  %118 = shl nuw i16 1, %110
  %119 = xor i16 %118, -1
  %120 = and i16 %117, %119
  store i16 %120, ptr @$var_heap.occupied, !tbaa !2
  br label %114

121:
  %122 = getelementptr i8, ptr %94, i16 4
  store ptr %97, ptr %122
  br label %123

123:
  %124 = getelementptr i8, ptr %0, i16 %69
  %125 = getelementptr i8, ptr %124, i16 -8
  %126 = load i16, ptr %125
  %127 = and i16 %126, -4
  %128 = add i16 %127, %69
  %129 = getelementptr i8, ptr %0, i16 -8
  %130 = load i16, ptr %129
  %131 = and i16 %130, 3
  %132 = or disjoint i16 %128, %131
  store i16 %132, ptr %129
  %133 = getelementptr i8, ptr %0, i16 %128
  %134 = getelementptr i8, ptr %133, i16 -8
  %135 = load i16, ptr %134
  %136 = or i16 %135, 2
  store i16 %136, ptr %134
  %137 = load i16, ptr %129
  %138 = and i16 %137, -4
  br label %77

139:
  %140 = and i16 %74, -4
  %141 = add i16 %140, %69
  %142 = icmp ult i16 %141, %66
  br i1 %142, label %29, label %91

143:
  %144 = icmp eq i16 %2, 0
  br i1 %144, label %53, label %145

145:
  %146 = udiv i16 -23, %2
  %147 = icmp ugt i16 %25, %146
  br i1 %147, label %29, label %53
}

define cc 1005 void @N$BDRP(ptr %0) addrspace(1) "llrm-fp-eval"="x87-extended" memory(readwrite, inaccessiblemem: none) nounwind nofree nosync norecurse {
b1:
  %1 = icmp eq ptr %0, null
  br i1 %1, label %b4, label %2

b2:
  tail call cc 1005 addrspace(1) void @heap.release(ptr %3)
  br label %b4

b4:
  ret void

2:
  %3 = getelementptr i8, ptr %0, i16 -6
  %4 = load i8, ptr %3
  %5 = zext i8 %4 to i16
  %6 = and i16 %5, 1
  %7 = icmp eq i16 %6, 0
  br i1 %7, label %b4, label %b2
}

define cc 1005 ptr @N$BGRW(ptr %0, i16 %1, i16 %2) addrspace(1) "llrm-fp-eval"="x87-extended" {
  %4 = getelementptr i8, ptr %0, i16 -4
  %5 = load i16, ptr %4
  %6 = add i16 %5, %1
  %7 = tail call cc 1005 addrspace(1) ptr @N$BRES(ptr %0, i16 %6, i16 %2)
  %8 = getelementptr i8, ptr %7, i16 -4
  store i16 %6, ptr %8
  ret ptr %7
}

define cc 1005 i16 @N$BSHR(ptr nocapture %0, i16 %1) addrspace(1) "llrm-fp-eval"="x87-extended" {
  %3 = getelementptr i8, ptr %0, i16 -4
  %4 = load i16, ptr %3
  %5 = icmp ult i16 %4, %1
  br i1 %5, label %6, label %b4

6:
  %7 = tail call addrspace(1) i32 @LL$WRITE_FILE(i16 1, ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str10, i16 6) to ptr addrspace(1)), i16 7)
  %8 = tail call addrspace(1) i32 @LL$WRITE_FILE(i16 1, ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str19, i16 6) to ptr addrspace(1)), i16 21)
  %9 = tail call addrspace(1) i32 @LL$WRITE_FILE(i16 1, ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str11, i16 6) to ptr addrspace(1)), i16 2)
  tail call addrspace(1) void @LL$EXIT(i8 zeroext -1)
  br label %b4

b4:
  %10 = sub i16 %4, %1
  %11 = getelementptr i8, ptr %0, i16 -4
  store i16 %10, ptr %11
  ret i16 %10
}

define cc 1005 ptr @N$BCLN(ptr readonly nocapture %0, i16 %1) addrspace(1) "llrm-fp-eval"="x87-extended" {
  %3 = getelementptr i8, ptr %0, i16 -4
  %4 = load i16, ptr %3
  %5 = tail call cc 1005 addrspace(1) ptr @buffers.allocate(i16 %4, i16 %1)
  %6 = mul i16 %4, %1
  %7 = add i16 %6, 1
  %8 = icmp eq i16 %7, 0
  br i1 %8, label %19, label %9

9:
  %10 = mul i16 %4, %1
  %11 = add i16 %10, 1
  br label %12

12:
  %13 = phi i16 [ %17, %12 ], [ 0, %9 ]
  %14 = getelementptr i8, ptr %5, i16 %13
  %15 = getelementptr i8, ptr %0, i16 %13
  %16 = load i8, ptr %15
  store i8 %16, ptr %14
  %17 = add nuw i16 %13, 1
  %18 = icmp eq i16 %11, %17
  br i1 %18, label %19, label %12

19:
  %20 = getelementptr i8, ptr %5, i16 -4
  store i16 %4, ptr %20
  ret ptr %5
}

define cc 1005 ptr @N$DRES(ptr %0, i16 %1) addrspace(1) "llrm-fp-eval"="x87-extended" {
  %3 = getelementptr i8, ptr %0, i16 -4
  %4 = load i16, ptr %3
  %5 = getelementptr i8, ptr %0, i16 -2
  %6 = load i16, ptr %5
  %7 = shl i16 %6, 2
  %8 = add i16 %7, 4
  %9 = mul i16 %4, 3
  %10 = icmp ugt i16 %8, %9
  br i1 %10, label %b4, label %common.ret

common.ret:
  %common.op = phi ptr [ %0, %2 ], [ %15, %30 ], [ %15, %29 ]
  ret ptr %common.op

b4:
  %11 = icmp ult i16 %4, 8
  %12 = shl i16 %4, 1
  %13 = select i1 %11, i16 8, i16 %12
  %14 = add i16 %13, -1
  %15 = tail call cc 1005 addrspace(1) ptr @buffers.allocate(i16 %13, i16 %1)
  %16 = mul i16 %13, %1
  %17 = icmp eq i16 %16, 0
  br i1 %17, label %b11, label %18

18:
  tail call void @llvm.memset.p0.i16(ptr align 1 %15, i8 0, i16 %16, i1 false)
  br label %b11

b11:
  %19 = icmp eq i16 %4, 0
  br i1 %19, label %30, label %20

20:
  br label %b13

b13:
  %21 = phi ptr [ %28, %b14 ], [ %0, %20 ]
  %22 = phi i16 [ 0, %20 ], [ %27, %b14 ]
  %23 = mul i16 %22, %1
  %24 = getelementptr i8, ptr %0, i16 %23
  %25 = load i16, ptr %24
  %26 = icmp eq i16 %25, 0
  br i1 %26, label %b14, label %b18

b14:
  %27 = add nuw i16 %22, 1
  %exitcond1 = icmp eq i16 %27, %4
  %28 = getelementptr i8, ptr %21, i16 %1
  br i1 %exitcond1, label %30, label %b13

29:
  tail call cc 1005 addrspace(1) void @heap.release(ptr %33)
  br label %common.ret

30:
  %31 = getelementptr i8, ptr %15, i16 -4
  store i16 %13, ptr %31
  %32 = getelementptr i8, ptr %15, i16 -2
  store i16 %6, ptr %32
  %33 = getelementptr i8, ptr %0, i16 -6
  %34 = load i8, ptr %33, align 2
  %35 = zext i8 %34 to i16
  %36 = and i16 %35, 1
  %37 = icmp eq i16 %36, 0
  br i1 %37, label %common.ret, label %29

b18:
  %38 = and i16 %25, %14
  %39 = mul i16 %38, %1
  %40 = getelementptr i8, ptr %15, i16 %39
  %41 = load i16, ptr %40
  %42 = icmp eq i16 %41, 0
  br i1 %42, label %51, label %43

43:
  br label %b20

b20:
  %44 = phi i16 [ %46, %b20 ], [ %38, %43 ]
  %45 = add i16 %44, 1
  %46 = and i16 %45, %14
  %47 = mul i16 %46, %1
  %48 = getelementptr i8, ptr %15, i16 %47
  %49 = load i16, ptr %48
  %50 = icmp eq i16 %49, 0
  br i1 %50, label %51, label %b20

51:
  %52 = phi i16 [ %39, %b18 ], [ %47, %b20 ]
  %53 = icmp eq i16 %1, 0
  br i1 %53, label %b14, label %54

54:
  %55 = getelementptr i8, ptr %15, i16 %52
  br label %56

56:
  %57 = phi i16 [ %61, %56 ], [ 0, %54 ]
  %58 = getelementptr i8, ptr %55, i16 %57
  %59 = getelementptr i8, ptr %21, i16 %57
  %60 = load i8, ptr %59
  store i8 %60, ptr %58
  %61 = add nuw i16 %57, 1
  %exitcond = icmp eq i16 %1, %61
  br i1 %exitcond, label %b14, label %56
}

define cc 1005 ptr @N$TAPP(ptr %0, ptr readonly nocapture %1) addrspace(1) "llrm-fp-eval"="x87-extended" {
  %3 = getelementptr i8, ptr %1, i16 -4
  %4 = load i16, ptr %3
  %5 = getelementptr i8, ptr %0, i16 -4
  %6 = load i16, ptr %5
  %7 = add i16 %6, %4
  %8 = tail call cc 1005 addrspace(1) ptr @N$BRES(ptr %0, i16 %7, i16 1)
  %9 = icmp eq i16 %4, 0
  br i1 %9, label %18, label %10

10:
  %11 = getelementptr i8, ptr %8, i16 %6
  br label %12

12:
  %13 = phi i16 [ %17, %12 ], [ 0, %10 ]
  %14 = getelementptr i8, ptr %11, i16 %13
  %15 = getelementptr i8, ptr %1, i16 %13
  %16 = load i8, ptr %15
  store i8 %16, ptr %14
  %17 = add nuw i16 %13, 1
  %exitcond = icmp eq i16 %4, %17
  br i1 %exitcond, label %18, label %12

18:
  %19 = getelementptr i8, ptr %8, i16 -4
  store i16 %7, ptr %19
  %20 = getelementptr i8, ptr %8, i16 %7
  store i8 0, ptr %20
  ret ptr %8
}

define cc 1005 ptr @N$TCAT(ptr readonly nocapture %0, ptr nocapture readonly %1) addrspace(1) "llrm-fp-eval"="x87-extended" {
  %3 = getelementptr i8, ptr %0, i16 -4
  %4 = load i16, ptr %3
  %5 = getelementptr i8, ptr %1, i16 -4
  %6 = load i16, ptr %5
  %7 = add i16 %6, %4
  %8 = icmp ugt i16 %7, -23
  br i1 %8, label %9, label %13

9:
  %10 = tail call addrspace(1) i32 @LL$WRITE_FILE(i16 1, ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str10, i16 6) to ptr addrspace(1)), i16 7)
  %11 = tail call addrspace(1) i32 @LL$WRITE_FILE(i16 1, ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str18, i16 6) to ptr addrspace(1)), i16 13)
  %12 = tail call addrspace(1) i32 @LL$WRITE_FILE(i16 1, ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str11, i16 6) to ptr addrspace(1)), i16 2)
  tail call addrspace(1) void @LL$EXIT(i8 zeroext -1)
  br label %13

13:
  %14 = add i16 %7, 7
  %15 = tail call cc 1005 addrspace(1) ptr @heap.allocate(i16 %14)
  %16 = getelementptr i8, ptr %15, i16 6
  store i8 1, ptr %15
  %17 = getelementptr i8, ptr %15, i16 1
  store i8 0, ptr %17
  %18 = getelementptr i8, ptr %15, i16 2
  store i16 0, ptr %18
  %19 = getelementptr i8, ptr %15, i16 4
  store i16 %7, ptr %19
  store i8 0, ptr %16, align 2
  %20 = icmp eq i16 %4, 0
  br i1 %20, label %29, label %21

21:
  br label %22

22:
  %23 = phi i16 [ %28, %22 ], [ 0, %21 ]
  %24 = getelementptr i8, ptr %0, i16 %23
  %25 = load i8, ptr %24
  %26 = getelementptr i8, ptr %15, i16 %23
  %27 = getelementptr i8, ptr %26, i16 6
  store i8 %25, ptr %27
  %28 = add nuw i16 %23, 1
  %exitcond1 = icmp eq i16 %4, %28
  br i1 %exitcond1, label %29, label %22

29:
  %30 = getelementptr i8, ptr %15, i16 2
  store i16 %4, ptr %30
  %31 = getelementptr i8, ptr %1, i16 -4
  %32 = load i16, ptr %31
  %33 = add i16 %32, %4
  %34 = tail call cc 1005 addrspace(1) ptr @N$BRES(ptr %16, i16 %33, i16 1)
  %35 = icmp eq i16 %32, 0
  br i1 %35, label %44, label %36

36:
  %37 = getelementptr i8, ptr %34, i16 %4
  br label %38

38:
  %39 = phi i16 [ %43, %38 ], [ 0, %36 ]
  %40 = getelementptr i8, ptr %37, i16 %39
  %41 = getelementptr i8, ptr %1, i16 %39
  %42 = load i8, ptr %41
  store i8 %42, ptr %40
  %43 = add nuw i16 %39, 1
  %exitcond = icmp eq i16 %32, %43
  br i1 %exitcond, label %44, label %38

44:
  %45 = getelementptr i8, ptr %34, i16 -4
  store i16 %33, ptr %45
  %46 = getelementptr i8, ptr %34, i16 %33
  store i8 0, ptr %46
  ret ptr %34
}

define cc 1005 ptr @N$VCPY(ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
  %2 = load i16, ptr addrspace(1) %0
  %3 = icmp ugt i16 %2, -23
  br i1 %3, label %4, label %8

4:
  %5 = tail call addrspace(1) i32 @LL$WRITE_FILE(i16 1, ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str10, i16 6) to ptr addrspace(1)), i16 7)
  %6 = tail call addrspace(1) i32 @LL$WRITE_FILE(i16 1, ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str18, i16 6) to ptr addrspace(1)), i16 13)
  %7 = tail call addrspace(1) i32 @LL$WRITE_FILE(i16 1, ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str11, i16 6) to ptr addrspace(1)), i16 2)
  tail call addrspace(1) void @LL$EXIT(i8 zeroext -1)
  br label %8

8:
  %9 = add i16 %2, 7
  %10 = tail call cc 1005 addrspace(1) ptr @heap.allocate(i16 %9)
  %11 = getelementptr i8, ptr %10, i16 6
  store i8 1, ptr %10
  %12 = getelementptr i8, ptr %10, i16 1
  store i8 0, ptr %12
  %13 = getelementptr i8, ptr %10, i16 2
  store i16 0, ptr %13
  %14 = getelementptr i8, ptr %10, i16 4
  store i16 %2, ptr %14
  store i8 0, ptr %11, align 2
  %15 = getelementptr inbounds nuw i8, ptr addrspace(1) %0, i16 4
  %16 = load ptr addrspace(1), ptr addrspace(1) %15
  %17 = icmp eq i16 %2, 0
  br i1 %17, label %26, label %18

18:
  br label %19

19:
  %20 = phi i16 [ %25, %19 ], [ 0, %18 ]
  %21 = getelementptr i8, ptr addrspace(1) %16, i16 %20
  %22 = load i8, ptr addrspace(1) %21
  %23 = getelementptr i8, ptr %10, i16 %20
  %24 = getelementptr i8, ptr %23, i16 6
  store i8 %22, ptr %24
  %25 = add nuw i16 %20, 1
  %exitcond = icmp eq i16 %2, %25
  br i1 %exitcond, label %26, label %19

26:
  %27 = getelementptr i8, ptr %10, i16 %2
  %28 = getelementptr i8, ptr %27, i16 6
  store i8 0, ptr %28
  %29 = getelementptr i8, ptr %10, i16 2
  store i16 %2, ptr %29
  ret ptr %11
}

define cc 1005 range(i8 -1, 2) i8 @N$VCMP(ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %0, ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %1) addrspace(1) "llrm-fp-eval"="x87-extended" memory(read, inaccessiblemem: none) nounwind nofree nosync norecurse {
b1:
  %2 = load i16, ptr addrspace(1) %0
  %3 = load i16, ptr addrspace(1) %1
  %4 = tail call i16 @llvm.umin.i16(i16 %2, i16 %3)
  %5 = getelementptr inbounds nuw i8, ptr addrspace(1) %0, i16 4
  %6 = load ptr addrspace(1), ptr addrspace(1) %5
  %7 = getelementptr inbounds nuw i8, ptr addrspace(1) %1, i16 4
  %8 = load ptr addrspace(1), ptr addrspace(1) %7
  %9 = icmp eq i16 %4, 0
  br i1 %9, label %b8, label %10

10:
  %11 = call i16 @llvm.umin.i16(i16 %3, i16 %2)
  br label %b6

b5:
  %12 = add nuw i16 %13, 1
  %exitcond = icmp eq i16 %11, %12
  br i1 %exitcond, label %b8, label %b6

b6:
  %13 = phi i16 [ %12, %b5 ], [ 0, %10 ]
  %14 = getelementptr i8, ptr addrspace(1) %6, i16 %13
  %15 = load i8, ptr addrspace(1) %14
  %16 = getelementptr i8, ptr addrspace(1) %8, i16 %13
  %17 = load i8, ptr addrspace(1) %16
  %18 = icmp eq i8 %15, %17
  br i1 %18, label %b5, label %b9

b8:
  %19 = icmp ult i16 %2, %3
  %20 = icmp eq i16 %2, %3
  %21 = select i1 %19, i8 -1, i8 1
  %22 = select i1 %20, i8 0, i8 %21
  br label %common.ret

b9:
  %23 = icmp ult i8 %15, %17
  %24 = select i1 %23, i8 -1, i8 1
  br label %common.ret

common.ret:
  %common.op = phi i8 [ %24, %b9 ], [ %22, %b8 ]
  ret i8 %common.op
}

define internal cc 1006 void @format.field(ptr addrspace(1) %0, i16 %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
  %3 = load i8, ptr @$var_format.width, !tbaa !2
  %4 = zext i8 %3 to i16
  %5 = zext i8 %3 to i16
  %6 = zext i8 %3 to i16
  %7 = icmp ugt i16 %6, %1
  %8 = sub nsw i16 %6, %1
  %9 = select i1 %7, i16 %8, i16 0
  store i8 0, ptr @$var_format.width, !tbaa !2
  store i8 10, ptr @$var_format.radix, !tbaa !2
  %10 = load i8, ptr @$var_format.left, !tbaa !2, !range !18
  %11 = icmp ne i8 %10, 0
  %12 = icmp eq i16 %9, 0
  %or.cond1 = select i1 %11, i1 true, i1 %12
  %13 = load ptr, ptr @$var_format.sink, !tbaa !2
  br i1 %or.cond1, label %36, label %14

14:
  %15 = call i16 @llvm.umax.i16(i16 %1, i16 %5)
  %16 = sub i16 %15, %1
  br label %17

17:
  %18 = phi i16 [ %16, %14 ], [ %35, %33 ]
  %19 = phi ptr [ %34, %33 ], [ %13, %14 ]
  %20 = icmp eq ptr %19, null
  br i1 %20, label %21, label %24

21:
  %22 = tail call addrspace(1) i32 @LL$WRITE_FILE(i16 1, ptr addrspace(1) addrspacecast (ptr @$var_format.fill to ptr addrspace(1)), i16 1)
  %23 = load ptr, ptr @$var_format.sink, !tbaa !2
  br label %33

24:
  %25 = getelementptr i8, ptr %19, i16 -4
  %26 = load i16, ptr %25
  %27 = add i16 %26, 1
  %28 = tail call cc 1005 addrspace(1) ptr @N$BRES(ptr nonnull %19, i16 %27, i16 1)
  %29 = getelementptr i8, ptr %28, i16 %26
  %30 = load i8, ptr @$var_format.fill
  store i8 %30, ptr %29
  %31 = getelementptr i8, ptr %28, i16 -4
  store i16 %27, ptr %31
  %32 = getelementptr i8, ptr %28, i16 %27
  store i8 0, ptr %32
  store ptr %28, ptr @$var_format.sink, !tbaa !2
  br label %33

33:
  %34 = phi ptr [ %23, %21 ], [ %28, %24 ]
  %35 = add i16 %18, -1
  %exitcond11 = icmp eq i16 %35, 0
  br i1 %exitcond11, label %36, label %17

36:
  %37 = phi ptr [ %13, %2 ], [ %34, %33 ]
  %38 = icmp eq ptr %37, null
  br i1 %38, label %39, label %41

39:
  %40 = tail call addrspace(1) i32 @LL$WRITE_FILE(i16 1, ptr addrspace(1) %0, i16 %1)
  br label %58

41:
  %42 = getelementptr i8, ptr %37, i16 -4
  %43 = load i16, ptr %42
  %44 = add i16 %43, %1
  %45 = tail call cc 1005 addrspace(1) ptr @N$BRES(ptr nonnull %37, i16 %44, i16 1)
  %46 = icmp eq i16 %1, 0
  br i1 %46, label %55, label %47

47:
  %48 = getelementptr i8, ptr %45, i16 %43
  br label %49

49:
  %50 = phi i16 [ %54, %49 ], [ 0, %47 ]
  %51 = getelementptr i8, ptr %48, i16 %50
  %52 = getelementptr i8, ptr addrspace(1) %0, i16 %50
  %53 = load i8, ptr addrspace(1) %52
  store i8 %53, ptr %51
  %54 = add nuw i16 %50, 1
  %exitcond = icmp eq i16 %1, %54
  br i1 %exitcond, label %55, label %49

55:
  %56 = getelementptr i8, ptr %45, i16 -4
  store i16 %44, ptr %56
  %57 = getelementptr i8, ptr %45, i16 %44
  store i8 0, ptr %57
  store ptr %45, ptr @$var_format.sink, !tbaa !2
  br label %58

58:
  %59 = icmp eq i16 %9, 0
  %60 = load i8, ptr @$var_format.left, !tbaa !2, !range !19
  %61 = icmp eq i8 %60, 0
  %or.cond = select i1 %61, i1 true, i1 %59
  br i1 %or.cond, label %85, label %62

62:
  %63 = load ptr, ptr @$var_format.sink, !tbaa !2
  %64 = call i16 @llvm.umax.i16(i16 %1, i16 %4)
  %65 = sub i16 %64, %1
  br label %66

66:
  %67 = phi i16 [ %84, %82 ], [ %65, %62 ]
  %68 = phi ptr [ %63, %62 ], [ %83, %82 ]
  %69 = icmp eq ptr %68, null
  br i1 %69, label %70, label %73

70:
  %71 = tail call addrspace(1) i32 @LL$WRITE_FILE(i16 1, ptr addrspace(1) addrspacecast (ptr @$var_format.fill to ptr addrspace(1)), i16 1)
  %72 = load ptr, ptr @$var_format.sink, !tbaa !2
  br label %82

73:
  %74 = getelementptr i8, ptr %68, i16 -4
  %75 = load i16, ptr %74
  %76 = add i16 %75, 1
  %77 = tail call cc 1005 addrspace(1) ptr @N$BRES(ptr nonnull %68, i16 %76, i16 1)
  %78 = getelementptr i8, ptr %77, i16 %75
  %79 = load i8, ptr @$var_format.fill
  store i8 %79, ptr %78
  %80 = getelementptr i8, ptr %77, i16 -4
  store i16 %76, ptr %80
  %81 = getelementptr i8, ptr %77, i16 %76
  store i8 0, ptr %81
  store ptr %77, ptr @$var_format.sink, !tbaa !2
  br label %82

82:
  %83 = phi ptr [ %72, %70 ], [ %77, %73 ]
  %84 = add i16 %67, -1
  %exitcond1 = icmp eq i16 %84, 0
  br i1 %exitcond1, label %85, label %66

85:
  store i8 32, ptr @$var_format.fill, !tbaa !2
  store i8 0, ptr @$var_format.left, !tbaa !2
  ret void
}

define internal cc 1005 void @format.number(ptr %0, i16 %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = load i8, ptr @$var_format.fill, !tbaa !2
  %3 = icmp eq i8 %2, 48
  br i1 %3, label %b7, label %b4

4:
  %5 = load ptr, ptr @$var_format.sink, !tbaa !2
  %6 = icmp eq ptr %5, null
  br i1 %6, label %7, label %10

7:
  %8 = addrspacecast ptr %0 to ptr addrspace(1)
  %9 = tail call addrspace(1) i32 @LL$WRITE_FILE(i16 1, ptr addrspace(1) %8, i16 1)
  br label %19

10:
  %11 = getelementptr i8, ptr %5, i16 -4
  %12 = load i16, ptr %11
  %13 = add i16 %12, 1
  %14 = tail call cc 1005 addrspace(1) ptr @N$BRES(ptr nonnull %5, i16 %13, i16 1)
  %15 = getelementptr i8, ptr %14, i16 %12
  %16 = load i8, ptr %0
  store i8 %16, ptr %15
  %17 = getelementptr i8, ptr %14, i16 -4
  store i16 %13, ptr %17
  %18 = getelementptr i8, ptr %14, i16 %13
  store i8 0, ptr %18
  store ptr %14, ptr @$var_format.sink, !tbaa !2
  br label %19

19:
  %20 = getelementptr i8, ptr %0, i16 1
  %21 = add i16 %1, -1
  %22 = load i8, ptr @$var_format.width, !tbaa !2
  %23 = icmp eq i8 %22, 0
  br i1 %23, label %b4, label %b8

b4:
  %24 = phi i16 [ %21, %b8 ], [ %21, %19 ], [ %1, %b5 ], [ %1, %b7 ], [ %1, %b1 ]
  %25 = phi ptr [ %20, %b8 ], [ %20, %19 ], [ %0, %b5 ], [ %0, %b7 ], [ %0, %b1 ]
  %26 = addrspacecast ptr %25 to ptr addrspace(1)
  tail call cc 1006 addrspace(1) void @format.field(ptr addrspace(1) %26, i16 %24)
  ret void

b5:
  %27 = load i8, ptr %0
  %28 = icmp eq i8 %27, 45
  br i1 %28, label %4, label %b4

b7:
  %29 = load i8, ptr @$var_format.left, !tbaa !2, !range !20
  %30 = icmp eq i8 %29, 0
  %31 = icmp ne i16 %1, 0
  %or.cond = select i1 %30, i1 %31, i1 false
  br i1 %or.cond, label %b5, label %b4

b8:
  %32 = add i8 %22, -1
  store i8 %32, ptr @$var_format.width, !tbaa !2
  br label %b4
}

define cc 1005 void @N$PI1(i8 %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
  %2 = sext i8 %0 to i32
  %3 = tail call i32 @llvm.abs.i32(i32 %2, i1 true)
  %4 = load i8, ptr @$var_format.radix, !tbaa !2
  %5 = zext i8 %4 to i32
  br label %6

6:
  %7 = phi i16 [ %19, %6 ], [ 0, %1 ]
  %8 = phi i32 [ %3, %1 ], [ %17, %6 ]
  %9 = urem i32 %8, %5
  %10 = getelementptr i8, ptr @$var_format.scratch, i16 %7
  %11 = getelementptr i8, ptr %10, i16 15
  %12 = icmp samesign ult i32 %9, 10
  %13 = trunc nuw i32 %9 to i8
  %14 = or disjoint i8 %13, 48
  %15 = add nuw i8 %13, 87
  %16 = select i1 %12, i8 %14, i8 %15
  store i8 %16, ptr %11
  %17 = udiv i32 %8, %5
  %18 = icmp eq i32 %17, 0
  %19 = add i16 %7, -1
  br i1 %18, label %20, label %6

20:
  %21 = icmp sgt i8 %0, -1
  %22 = sub i16 0, %19
  br i1 %21, label %28, label %23

23:
  %24 = sub i16 0, %19
  %25 = add i16 %24, 1
  %26 = getelementptr i8, ptr @$var_format.scratch, i16 %19
  %27 = getelementptr i8, ptr %26, i16 15
  store i8 45, ptr %27
  br label %28

28:
  %29 = phi i16 [ %25, %23 ], [ %22, %20 ]
  %30 = sub i16 0, %29
  %31 = getelementptr i8, ptr getelementptr inbounds nuw (i8, ptr @$var_format.scratch, i16 16), i16 %30
  tail call cc 1005 addrspace(1) void @format.number(ptr %31, i16 %29)
  ret void
}

define cc 1005 void @N$PU1(i8 %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
  %2 = zext i8 %0 to i32
  %3 = load i8, ptr @$var_format.radix, !tbaa !2
  %4 = zext i8 %3 to i32
  br label %5

5:
  %6 = phi i16 [ %18, %5 ], [ 0, %1 ]
  %7 = phi i32 [ %2, %1 ], [ %16, %5 ]
  %8 = urem i32 %7, %4
  %9 = getelementptr i8, ptr @$var_format.scratch, i16 %6
  %10 = getelementptr i8, ptr %9, i16 15
  %11 = icmp samesign ult i32 %8, 10
  %12 = trunc nuw i32 %8 to i8
  %13 = or disjoint i8 %12, 48
  %14 = add i8 %12, 87
  %15 = select i1 %11, i8 %13, i8 %14
  store i8 %15, ptr %10
  %16 = udiv i32 %7, %4
  %17 = icmp eq i32 %16, 0
  %18 = add i16 %6, -1
  br i1 %17, label %19, label %5

19:
  %20 = getelementptr i8, ptr @$var_format.scratch, i16 16
  %21 = getelementptr i8, ptr %20, i16 %18
  %22 = sub i16 0, %18
  tail call cc 1005 addrspace(1) void @format.number(ptr %21, i16 %22)
  ret void
}

define cc 1005 void @N$PI2(i16 %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
  %2 = sext i16 %0 to i32
  %3 = tail call i32 @llvm.abs.i32(i32 %2, i1 true)
  %4 = load i8, ptr @$var_format.radix, !tbaa !2
  %5 = zext i8 %4 to i32
  br label %6

6:
  %7 = phi i16 [ %19, %6 ], [ 0, %1 ]
  %8 = phi i32 [ %3, %1 ], [ %17, %6 ]
  %9 = urem i32 %8, %5
  %10 = getelementptr i8, ptr @$var_format.scratch, i16 %7
  %11 = getelementptr i8, ptr %10, i16 15
  %12 = icmp samesign ult i32 %9, 10
  %13 = trunc nuw i32 %9 to i8
  %14 = or disjoint i8 %13, 48
  %15 = add i8 %13, 87
  %16 = select i1 %12, i8 %14, i8 %15
  store i8 %16, ptr %11
  %17 = udiv i32 %8, %5
  %18 = icmp eq i32 %17, 0
  %19 = add i16 %7, -1
  br i1 %18, label %20, label %6

20:
  %21 = icmp sgt i16 %0, -1
  %22 = sub i16 0, %19
  br i1 %21, label %28, label %23

23:
  %24 = sub i16 0, %19
  %25 = add i16 %24, 1
  %26 = getelementptr i8, ptr @$var_format.scratch, i16 %19
  %27 = getelementptr i8, ptr %26, i16 15
  store i8 45, ptr %27
  br label %28

28:
  %29 = phi i16 [ %25, %23 ], [ %22, %20 ]
  %30 = sub i16 0, %29
  %31 = getelementptr i8, ptr getelementptr inbounds nuw (i8, ptr @$var_format.scratch, i16 16), i16 %30
  tail call cc 1005 addrspace(1) void @format.number(ptr %31, i16 %29)
  ret void
}

define cc 1005 void @N$PU2(i16 %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
  %2 = zext i16 %0 to i32
  %3 = load i8, ptr @$var_format.radix, !tbaa !2
  %4 = zext i8 %3 to i32
  br label %5

5:
  %6 = phi i16 [ %18, %5 ], [ 0, %1 ]
  %7 = phi i32 [ %2, %1 ], [ %16, %5 ]
  %8 = urem i32 %7, %4
  %9 = getelementptr i8, ptr @$var_format.scratch, i16 %6
  %10 = getelementptr i8, ptr %9, i16 15
  %11 = icmp samesign ult i32 %8, 10
  %12 = trunc nuw i32 %8 to i8
  %13 = or disjoint i8 %12, 48
  %14 = add i8 %12, 87
  %15 = select i1 %11, i8 %13, i8 %14
  store i8 %15, ptr %10
  %16 = udiv i32 %7, %4
  %17 = icmp eq i32 %16, 0
  %18 = add i16 %6, -1
  br i1 %17, label %19, label %5

19:
  %20 = getelementptr i8, ptr @$var_format.scratch, i16 16
  %21 = getelementptr i8, ptr %20, i16 %18
  %22 = sub i16 0, %18
  tail call cc 1005 addrspace(1) void @format.number(ptr %21, i16 %22)
  ret void
}

define cc 1005 void @N$PI4(i32 %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
  %2 = tail call i32 @llvm.abs.i32(i32 %0, i1 false)
  %3 = load i8, ptr @$var_format.radix, !tbaa !2
  %4 = zext i8 %3 to i32
  br label %5

5:
  %6 = phi i16 [ %18, %5 ], [ 0, %1 ]
  %7 = phi i32 [ %2, %1 ], [ %16, %5 ]
  %8 = urem i32 %7, %4
  %9 = getelementptr i8, ptr @$var_format.scratch, i16 %6
  %10 = getelementptr i8, ptr %9, i16 15
  %11 = icmp samesign ult i32 %8, 10
  %12 = trunc nuw i32 %8 to i8
  %13 = or disjoint i8 %12, 48
  %14 = add i8 %12, 87
  %15 = select i1 %11, i8 %13, i8 %14
  store i8 %15, ptr %10
  %16 = udiv i32 %7, %4
  %17 = icmp eq i32 %16, 0
  %18 = add i16 %6, -1
  br i1 %17, label %19, label %5

19:
  %20 = icmp sgt i32 %0, -1
  %21 = sub i16 0, %18
  br i1 %20, label %27, label %22

22:
  %23 = sub i16 0, %18
  %24 = add i16 %23, 1
  %25 = getelementptr i8, ptr @$var_format.scratch, i16 %18
  %26 = getelementptr i8, ptr %25, i16 15
  store i8 45, ptr %26
  br label %27

27:
  %28 = phi i16 [ %24, %22 ], [ %21, %19 ]
  %29 = sub i16 0, %28
  %30 = getelementptr i8, ptr getelementptr inbounds nuw (i8, ptr @$var_format.scratch, i16 16), i16 %29
  tail call cc 1005 addrspace(1) void @format.number(ptr %30, i16 %28)
  ret void
}

define cc 1005 void @N$PU4(i32 %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
  %2 = load i8, ptr @$var_format.radix, !tbaa !2
  %3 = zext i8 %2 to i32
  br label %4

4:
  %5 = phi i16 [ %17, %4 ], [ 0, %1 ]
  %6 = phi i32 [ %0, %1 ], [ %15, %4 ]
  %7 = urem i32 %6, %3
  %8 = getelementptr i8, ptr @$var_format.scratch, i16 %5
  %9 = getelementptr i8, ptr %8, i16 15
  %10 = icmp samesign ult i32 %7, 10
  %11 = trunc nuw i32 %7 to i8
  %12 = or disjoint i8 %11, 48
  %13 = add i8 %11, 87
  %14 = select i1 %10, i8 %12, i8 %13
  store i8 %14, ptr %9
  %15 = udiv i32 %6, %3
  %16 = icmp eq i32 %15, 0
  %17 = add i16 %5, -1
  br i1 %16, label %18, label %4

18:
  %19 = getelementptr i8, ptr @$var_format.scratch, i16 16
  %20 = getelementptr i8, ptr %19, i16 %17
  %21 = sub i16 0, %17
  tail call cc 1005 addrspace(1) void @format.number(ptr %20, i16 %21)
  ret void
}

define cc 1005 void @N$PQ4(i32 %0, i8 %1) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %2 = icmp slt i32 %0, 0
  br i1 %2, label %3, label %5

3:
  store i8 45, ptr @$var_format.scratch, align 16
  %4 = sub i32 0, %0
  br label %5

5:
  %6 = phi i32 [ %4, %3 ], [ %0, %b1 ]
  %7 = phi i16 [ 1, %3 ], [ 0, %b1 ]
  %8 = zext i8 %1 to i32
  %9 = lshr i32 %6, %8
  %10 = getelementptr i8, ptr @$var_format.scratch, i16 15
  br label %11

11:
  %12 = phi ptr [ %21, %11 ], [ %10, %5 ]
  %13 = phi i16 [ %20, %11 ], [ -1, %5 ]
  %14 = phi i32 [ %9, %5 ], [ %18, %11 ]
  %15 = urem i32 %14, 10
  %16 = trunc nuw nsw i32 %15 to i8
  %17 = or disjoint i8 %16, 48
  store i8 %17, ptr %12
  %18 = udiv i32 %14, 10
  %19 = icmp eq i32 %18, 0
  %20 = add i16 %13, 1
  %21 = getelementptr i8, ptr %12, i16 -1
  br i1 %19, label %22, label %11

22:
  %23 = icmp eq i16 %20, -1
  br i1 %23, label %39, label %24

24:
  %25 = sub i16 0, %20
  %26 = sub i16 -1, %20
  %27 = getelementptr i8, ptr @$var_format.scratch, i16 %7
  br label %28

28:
  %29 = phi ptr [ %27, %24 ], [ %37, %28 ]
  %30 = phi i16 [ %26, %24 ], [ %36, %28 ]
  %31 = phi i16 [ %25, %24 ], [ %35, %28 ]
  %32 = getelementptr i8, ptr @$var_format.scratch, i16 %30
  %33 = getelementptr i8, ptr %32, i16 16
  %34 = load i8, ptr %33
  store i8 %34, ptr %29
  %35 = add i16 %31, 1
  %36 = add i16 %30, 1
  %37 = getelementptr i8, ptr %29, i16 1
  %38 = icmp eq i16 %35, 1
  br i1 %38, label %39, label %28

39:
  %40 = add i16 %7, %20
  %41 = getelementptr i8, ptr @$var_format.scratch, i16 %40
  %42 = getelementptr i8, ptr %41, i16 1
  store i8 46, ptr %42
  %43 = add i16 %7, %20
  %44 = add i16 %43, 2
  %45 = shl nuw i32 1, %8
  %46 = add i32 %45, -1
  %47 = and i32 %6, %46
  %48 = icmp eq i32 %47, 0
  br i1 %48, label %b7.thread, label %54

b7.thread:
  %49 = add i16 %7, %20
  %50 = getelementptr i8, ptr @$var_format.scratch, i16 %49
  %51 = getelementptr i8, ptr %50, i16 2
  store i8 48, ptr %51
  %52 = add i16 %7, %20
  %53 = add i16 %52, 3
  br label %113

54:
  %55 = udiv i32 %45, 10
  %56 = urem i32 %45, 10
  %57 = trunc nuw nsw i32 %56 to i16
  %58 = add nuw nsw i16 %57, 9
  %59 = udiv i16 %58, 10
  %60 = zext nneg i16 %59 to i32
  %61 = add nuw nsw i32 %55, %60
  %62 = shl nuw nsw i32 %55, 1
  %63 = shl nuw nsw i16 %57, 1
  %64 = add nuw nsw i16 %63, 9
  %65 = udiv i16 %64, 10
  %66 = zext nneg i16 %65 to i32
  %67 = add nuw nsw i32 %62, %66
  %68 = mul nuw nsw i32 %55, 3
  %69 = mul nuw nsw i16 %57, 3
  %70 = add nuw nsw i16 %69, 9
  %71 = udiv i16 %70, 10
  %72 = zext nneg i16 %71 to i32
  %73 = add nuw nsw i32 %68, %72
  %74 = shl nuw nsw i32 %55, 2
  %75 = shl nuw nsw i16 %57, 2
  %76 = add nuw nsw i16 %75, 9
  %77 = udiv i16 %76, 10
  %78 = zext nneg i16 %77 to i32
  %79 = add nuw nsw i32 %74, %78
  %80 = mul nuw nsw i32 %55, 5
  %81 = mul nuw nsw i16 %57, 5
  %82 = add nuw nsw i16 %81, 9
  %83 = udiv i16 %82, 10
  %84 = zext nneg i16 %83 to i32
  %85 = add nuw nsw i32 %80, %84
  %86 = mul nuw nsw i32 %55, 6
  %87 = mul nuw nsw i16 %57, 6
  %88 = add nuw nsw i16 %87, 9
  %89 = udiv i16 %88, 10
  %90 = zext nneg i16 %89 to i32
  %91 = add nuw nsw i32 %86, %90
  %92 = mul nuw nsw i32 %55, 7
  %93 = mul nuw nsw i16 %57, 7
  %94 = add nuw nsw i16 %93, 9
  %95 = udiv i16 %94, 10
  %96 = zext nneg i16 %95 to i32
  %97 = add nuw nsw i32 %92, %96
  %98 = shl nuw nsw i32 %55, 3
  %99 = shl nuw nsw i16 %57, 3
  %100 = add nuw nsw i16 %99, 9
  %101 = udiv i16 %100, 10
  %102 = zext nneg i16 %101 to i32
  %103 = add nuw nsw i32 %98, %102
  %104 = mul nuw nsw i32 %55, 9
  %105 = mul nuw nsw i16 %57, 9
  %106 = add nuw nsw i16 %105, 9
  %107 = udiv i16 %106, 10
  %108 = zext nneg i16 %107 to i32
  %109 = add nuw nsw i32 %104, %108
  br label %b9

b9:
  %110 = phi i16 [ %44, %54 ], [ %143, %131 ]
  %111 = phi i32 [ %47, %54 ], [ %139, %131 ]
  %112 = icmp ult i32 %111, %61
  br i1 %112, label %131, label %b11

113:
  %114 = phi i16 [ %53, %b7.thread ], [ %143, %131 ]
  tail call cc 1005 addrspace(1) void @format.number(ptr @$var_format.scratch, i16 %114)
  ret void

b11:
  %115 = icmp ult i32 %111, %67
  br i1 %115, label %131, label %116

116:
  %117 = icmp ult i32 %111, %73
  br i1 %117, label %131, label %118

118:
  %119 = icmp ult i32 %111, %79
  br i1 %119, label %131, label %120

120:
  %121 = icmp ult i32 %111, %85
  br i1 %121, label %131, label %122

122:
  %123 = icmp ult i32 %111, %91
  br i1 %123, label %131, label %124

124:
  %125 = icmp ult i32 %111, %97
  br i1 %125, label %131, label %126

126:
  %127 = icmp ult i32 %111, %103
  br i1 %127, label %131, label %128

128:
  %129 = icmp ult i32 %111, %109
  %130 = select i1 %129, i16 8, i16 9
  br label %131

131:
  %132 = phi i16 [ 0, %b9 ], [ 1, %b11 ], [ 2, %116 ], [ 3, %118 ], [ 4, %120 ], [ 5, %122 ], [ 6, %124 ], [ 7, %126 ], [ %130, %128 ]
  %133 = zext nneg i16 %132 to i32
  %134 = mul nuw nsw i32 %55, %133
  %135 = sub i32 %111, %134
  %136 = mul i32 %135, 10
  %137 = mul nuw nsw i16 %132, %57
  %138 = zext nneg i16 %137 to i32
  %139 = sub i32 %136, %138
  %140 = getelementptr i8, ptr @$var_format.scratch, i16 %110
  %141 = trunc nuw nsw i16 %132 to i8
  %142 = or disjoint i8 %141, 48
  store i8 %142, ptr %140
  %143 = add i16 %110, 1
  %144 = icmp eq i32 %139, 0
  br i1 %144, label %113, label %b9
}

define cc 1005 void @N$PQ2(i16 %0, i8 %1) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %2 = sext i16 %0 to i32
  tail call cc 1005 addrspace(1) void @N$PQ4(i32 %2, i8 zeroext %1)
  ret void
}

define cc 1005 void @N$PB(i8 %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
  %2 = icmp eq i8 %0, 0
  %3 = select i1 %2, ptr getelementptr inbounds nuw (i8, ptr @$str21, i16 6), ptr getelementptr inbounds nuw (i8, ptr @$str20, i16 6)
  %4 = select i1 %2, ptr getelementptr (i8, ptr @$str21, i16 2), ptr getelementptr (i8, ptr @$str20, i16 2)
  %5 = load i16, ptr %4
  %6 = addrspacecast ptr %3 to ptr addrspace(1)
  tail call cc 1006 addrspace(1) void @format.field(ptr addrspace(1) %6, i16 %5)
  ret void
}

define cc 1005 void @N$PC(i8 %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
  store i8 %0, ptr @$var_format.scratch, align 16
  tail call cc 1006 addrspace(1) void @format.field(ptr addrspace(1) addrspacecast (ptr @$var_format.scratch to ptr addrspace(1)), i16 1)
  ret void
}

define cc 1005 void @N$PS(ptr %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
  %2 = addrspacecast ptr %0 to ptr addrspace(1)
  %3 = getelementptr i8, ptr %0, i16 -4
  %4 = load i16, ptr %3
  tail call cc 1006 addrspace(1) void @format.field(ptr addrspace(1) %2, i16 %4)
  ret void
}

define cc 1005 void @N$PV(ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %1 = getelementptr inbounds nuw i8, ptr addrspace(1) %0, i16 4
  %2 = load ptr addrspace(1), ptr addrspace(1) %1
  %3 = load i16, ptr addrspace(1) %0
  tail call cc 1006 addrspace(1) void @format.field(ptr addrspace(1) %2, i16 %3)
  ret void
}

define cc 1005 void @N$PN() addrspace(1) "llrm-fp-eval"="x87-extended" {
  %1 = load ptr, ptr @$var_format.sink, !tbaa !2
  %2 = icmp eq ptr %1, null
  br i1 %2, label %3, label %5

3:
  %4 = tail call addrspace(1) i32 @LL$WRITE_FILE(i16 1, ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str11, i16 6) to ptr addrspace(1)), i16 2)
  br label %14

5:
  %6 = getelementptr i8, ptr %1, i16 -4
  %7 = load i16, ptr %6
  %8 = add i16 %7, 2
  %9 = tail call cc 1005 addrspace(1) ptr @N$BRES(ptr nonnull %1, i16 %8, i16 1)
  %10 = getelementptr i8, ptr %9, i16 %7
  store i8 13, ptr %10
  %11 = getelementptr i8, ptr %10, i16 1
  store i8 10, ptr %11
  %12 = getelementptr i8, ptr %9, i16 -4
  store i16 %8, ptr %12
  %13 = getelementptr i8, ptr %9, i16 %8
  store i8 0, ptr %13
  store ptr %9, ptr @$var_format.sink, !tbaa !2
  br label %14

14:
  ret void
}

define cc 1005 void @N$PFLD(i8 %0, i8 %1, i8 %2, i8 %3) addrspace(1) "llrm-fp-eval"="x87-extended" memory(write, argmem: none, inaccessiblemem: none) willreturn nounwind nofree nosync norecurse {
b1:
  store i8 %0, ptr @$var_format.width, !tbaa !2
  store i8 %1, ptr @$var_format.radix, !tbaa !2
  store i8 %2, ptr @$var_format.fill, !tbaa !2
  %4 = icmp ne i8 %3, 0
  %5 = zext i1 %4 to i8
  store i8 %5, ptr @$var_format.left, !tbaa !2
  ret void
}

define cc 1005 void @N$PBEG() addrspace(1) "llrm-fp-eval"="x87-extended" {
  %1 = tail call cc 1005 addrspace(1) ptr @heap.allocate(i16 23)
  %2 = getelementptr i8, ptr %1, i16 6
  store i8 1, ptr %1
  %3 = getelementptr i8, ptr %1, i16 1
  store i8 0, ptr %3
  %4 = getelementptr i8, ptr %1, i16 2
  store i16 0, ptr %4
  %5 = getelementptr i8, ptr %1, i16 4
  store i16 16, ptr %5
  store i8 0, ptr %2, align 2
  store ptr %2, ptr @$var_format.sink, !tbaa !2
  ret void
}

define cc 1005 ptr @N$PEND() addrspace(1) "llrm-fp-eval"="x87-extended" memory(readwrite, argmem: none, inaccessiblemem: none) willreturn nounwind nofree nosync norecurse {
b1:
  %0 = load ptr, ptr @$var_format.sink, !tbaa !2
  store ptr null, ptr @$var_format.sink, !tbaa !2
  ret ptr %0
}

define internal cc 1006 void @floats.print_float(ptr addrspace(5) nocapture readonly %0, i16 range(i16 2, 5) %1, i16 range(i16 4, 8) %2, i16 range(i16 255, 2048) %3, i16 range(i16 150, 1076) %4) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
  %6 = tail call cc 1005 addrspace(1) ptr @heap.allocate(i16 752)
  %7 = getelementptr i8, ptr %6, i16 720
  %8 = add nsw i16 %1, -1
  %9 = shl nuw nsw i16 %8, 1
  %10 = getelementptr i8, ptr addrspace(5) %0, i16 %9
  %11 = load i16, ptr addrspace(5) %10
  %12 = lshr i16 %11, %2
  %13 = and i16 %12, %3
  %14 = shl nuw nsw i16 1, %2
  %15 = add nsw i16 %14, -1
  %16 = and i16 %11, %15
  %17 = icmp eq i16 %16, 0
  %18 = zext i1 %17 to i8
  %19 = add nsw i16 %1, -1
  br label %b3

b3:
  %20 = phi i16 [ %31, %b7 ], [ 0, %5 ]
  %21 = phi i16 [ %30, %b7 ], [ %19, %5 ]
  %22 = phi i8 [ %18, %5 ], [ %29, %b7 ]
  %23 = icmp eq i8 %22, 0
  br i1 %23, label %b7, label %b6

b5:
  %24 = icmp sgt i16 %11, -1
  br i1 %24, label %b10, label %b11

b6:
  %25 = getelementptr i8, ptr addrspace(5) %0, i16 %20
  %26 = load i16, ptr addrspace(5) %25
  %27 = icmp eq i16 %26, 0
  %28 = zext i1 %27 to i8
  br label %b7

b7:
  %29 = phi i8 [ %28, %b6 ], [ 0, %b3 ]
  %30 = add i16 %21, -1
  %31 = add i16 %20, 2
  %exitcond17 = icmp eq i16 %30, 0
  br i1 %exitcond17, label %b5, label %b3

32:
  %33 = getelementptr i8, ptr %6, i16 720
  store i8 45, ptr %33
  br label %b10

b10:
  %34 = phi i16 [ 1, %32 ], [ 0, %b11 ], [ 0, %b5 ]
  %35 = icmp eq i16 %13, %3
  br i1 %35, label %38, label %b14

b11:
  %36 = icmp ne i16 %13, %3
  %37 = icmp ne i8 %29, 0
  %or.cond = select i1 %36, i1 true, i1 %37
  br i1 %or.cond, label %32, label %b10

38:
  %39 = icmp eq i8 %29, 0
  %40 = select i1 %39, ptr getelementptr inbounds nuw (i8, ptr @$str23, i16 6), ptr getelementptr inbounds nuw (i8, ptr @$str22, i16 6)
  %41 = select i1 %39, ptr getelementptr (i8, ptr @$str23, i16 2), ptr getelementptr (i8, ptr @$str22, i16 2)
  %42 = load i16, ptr %41
  %43 = icmp eq i16 %42, 0
  br i1 %43, label %b15, label %44

44:
  br label %45

45:
  %46 = phi ptr [ %40, %44 ], [ %54, %45 ]
  %47 = phi i16 [ %42, %44 ], [ %53, %45 ]
  %48 = phi i16 [ %52, %45 ], [ %34, %44 ]
  %49 = load i8, ptr %46
  %50 = getelementptr i8, ptr %6, i16 %48
  %51 = getelementptr i8, ptr %50, i16 720
  store i8 %49, ptr %51
  %52 = add i16 %48, 1
  %53 = add i16 %47, -1
  %54 = getelementptr i8, ptr %46, i16 1
  %exitcond15 = icmp eq i16 %53, 0
  br i1 %exitcond15, label %b15, label %45

b14:
  %55 = icmp eq i16 %13, 0
  %56 = icmp ne i8 %29, 0
  %or.cond1 = select i1 %55, i1 %56, i1 false
  br i1 %or.cond1, label %64, label %72

57:
  %58 = add nuw nsw i16 %34, 2
  %59 = add i16 %671, %58
  %60 = sub i16 %59, %353
  br label %b15

61:
  %62 = sub i16 %691, %707
  br label %b15

b15:
  %63 = phi i16 [ %697, %694 ], [ %758, %745 ], [ %71, %64 ], [ %60, %57 ], [ %62, %61 ], [ %34, %38 ], [ %52, %45 ], [ %661, %660 ]
  tail call cc 1005 addrspace(1) void @format.number(ptr %7, i16 %63)
  tail call cc 1005 addrspace(1) void @heap.release(ptr %6)
  ret void

64:
  %65 = getelementptr i8, ptr %6, i16 %34
  %66 = getelementptr i8, ptr %65, i16 720
  store i8 48, ptr %66
  %67 = getelementptr i8, ptr %6, i16 %34
  %68 = getelementptr i8, ptr %67, i16 721
  store i8 46, ptr %68
  %69 = getelementptr i8, ptr %6, i16 %34
  %70 = getelementptr i8, ptr %69, i16 722
  store i8 48, ptr %70
  %71 = add nuw nsw i16 %34, 3
  br label %b15

72:
  tail call void @llvm.memset.p0.i16(ptr align 2 %6, i8 0, i16 144, i1 false)
  %73 = add nsw i16 %1, -1
  br label %b24

b24:
  %74 = phi i16 [ %80, %b24 ], [ 0, %72 ]
  %75 = phi i16 [ %79, %b24 ], [ %73, %72 ]
  %76 = getelementptr i8, ptr %6, i16 %74
  %77 = getelementptr i8, ptr addrspace(5) %0, i16 %74
  %78 = load i16, ptr addrspace(5) %77
  store i16 %78, ptr %76
  %79 = add i16 %75, -1
  %80 = add nuw i16 %74, 2
  %exitcond16 = icmp eq i16 %79, 0
  br i1 %exitcond16, label %81, label %b24

81:
  %82 = icmp eq i16 %13, 0
  %83 = getelementptr i8, ptr %6, i16 %9
  %84 = select i1 %82, i16 0, i16 %14
  %85 = or i16 %84, %16
  store i16 %85, ptr %83
  %86 = select i1 %82, i16 1, i16 %13
  %87 = sub nsw i16 %86, %4
  %88 = icmp ugt i16 %13, 1
  %89 = select i1 %88, i8 %29, i8 0
  %90 = load i16, ptr addrspace(5) %0
  %91 = trunc i16 %90 to i8
  %92 = and i8 %91, 1
  %93 = xor i8 %92, 1
  %94 = getelementptr i8, ptr %6, i16 288
  %95 = getelementptr i8, ptr %6, i16 432
  %96 = getelementptr i8, ptr %6, i16 576
  %97 = icmp eq i8 %89, 0
  %98 = xor i1 %97, true
  %99 = zext i1 %98 to i16
  %100 = icmp sgt i16 %87, -1
  br i1 %100, label %101, label %196

101:
  %102 = icmp eq i8 %89, 0
  %103 = add nuw nsw i16 %87, 1
  %104 = add nuw nsw i16 %103, %99
  %105 = lshr i16 %104, 4
  %106 = and i16 %104, 15
  %107 = call i16 @llvm.umax.i16(i16 %13, i16 1)
  %108 = add i1 %102, true
  %109 = zext i1 %108 to i16
  %110 = add i16 %107, %109
  %111 = add i16 %110, 1
  %112 = sub i16 %111, %4
  %113 = lshr i16 %112, 4
  %114 = mul nsw i16 %113, -2
  %115 = getelementptr i8, ptr %6, i16 142
  br label %116

116:
  %117 = phi i16 [ %131, %127 ], [ 71, %101 ]
  %118 = phi ptr [ %130, %127 ], [ %115, %101 ]
  %119 = phi i16 [ %129, %127 ], [ -72, %101 ]
  %120 = icmp samesign ult i16 %117, %105
  br i1 %120, label %127, label %124

121:
  %122 = icmp eq i16 %106, 0
  br i1 %122, label %149, label %123

123:
  br label %133

124:
  %125 = getelementptr i8, ptr %118, i16 %114
  %126 = load i16, ptr %125
  br label %127

127:
  %128 = phi i16 [ %126, %124 ], [ 0, %116 ]
  store i16 %128, ptr %118
  %129 = add nsw i16 %119, 1
  %130 = getelementptr i8, ptr %118, i16 -2
  %131 = add nsw i16 %117, -1
  %132 = icmp eq i16 %129, 0
  br i1 %132, label %121, label %116

133:
  %134 = phi i16 [ 73, %123 ], [ %144, %133 ]
  %135 = shl nuw nsw i16 %134, 1
  %136 = getelementptr i8, ptr %6, i16 %135
  %137 = getelementptr i8, ptr %136, i16 -4
  %138 = load i16, ptr %137
  %139 = shl nuw nsw i16 %134, 1
  %140 = getelementptr i8, ptr %6, i16 %139
  %141 = getelementptr i8, ptr %140, i16 -6
  %142 = load i16, ptr %141
  %143 = tail call i16 @llvm.fshl.i16(i16 %138, i16 %142, i16 %106)
  store i16 %143, ptr %137
  %144 = add nsw i16 %134, -1
  %145 = icmp samesign ugt i16 %144, 2
  br i1 %145, label %133, label %146

146:
  %147 = load i16, ptr %6
  %148 = shl i16 %147, %106
  store i16 %148, ptr %6
  br label %149

149:
  %150 = shl nuw nsw i16 2, %99
  %151 = getelementptr i8, ptr %6, i16 146
  tail call void @llvm.memset.p0.i16(ptr align 2 %151, i8 0, i16 142, i1 false)
  %152 = getelementptr i8, ptr %6, i16 144
  store i16 %150, ptr %152
  %153 = getelementptr i8, ptr %6, i16 434
  tail call void @llvm.memset.p0.i16(ptr align 2 %153, i8 0, i16 142, i1 false)
  %154 = getelementptr i8, ptr %6, i16 432
  store i16 1, ptr %154
  %155 = lshr i16 %87, 4
  %156 = and i16 %87, 15
  %157 = call i16 @llvm.umax.i16(i16 %13, i16 1)
  %158 = sub i16 %157, %4
  %159 = lshr i16 %158, 4
  %160 = shl nuw nsw i16 %159, 1
  %161 = sub i16 0, %160
  %162 = getelementptr i8, ptr %6, i16 574
  %163 = getelementptr i8, ptr %162, i16 %161
  br label %164

164:
  %165 = phi i16 [ %180, %175 ], [ 71, %149 ]
  %166 = phi i16 [ %179, %175 ], [ 0, %149 ]
  %167 = icmp samesign ult i16 %165, %155
  br i1 %167, label %175, label %172

168:
  %169 = icmp eq i16 %156, 0
  br i1 %169, label %269, label %170

170:
  %171 = getelementptr i8, ptr %6, i16 574
  br label %182

172:
  %173 = getelementptr i8, ptr %163, i16 %166
  %174 = load i16, ptr %173
  br label %175

175:
  %176 = phi i16 [ %174, %172 ], [ 0, %164 ]
  %177 = getelementptr i8, ptr %6, i16 %166
  %178 = getelementptr i8, ptr %177, i16 574
  store i16 %176, ptr %178
  %179 = add nsw i16 %166, -2
  %180 = add nsw i16 %165, -1
  %181 = icmp eq i16 %179, -144
  br i1 %181, label %168, label %164

182:
  %183 = phi ptr [ %171, %170 ], [ %190, %182 ]
  %184 = phi i16 [ 73, %170 ], [ %189, %182 ]
  %185 = load i16, ptr %183
  %186 = getelementptr i8, ptr %183, i16 -2
  %187 = load i16, ptr %186
  %188 = tail call i16 @llvm.fshl.i16(i16 %185, i16 %187, i16 %156)
  store i16 %188, ptr %183
  %189 = add nsw i16 %184, -1
  %190 = getelementptr i8, ptr %183, i16 -2
  %191 = icmp samesign ugt i16 %189, 2
  br i1 %191, label %182, label %192

192:
  %193 = getelementptr i8, ptr %6, i16 432
  %194 = load i16, ptr %193
  %195 = shl i16 %194, %156
  br label %267

196:
  %197 = icmp eq i8 %89, 0
  %198 = select i1 %197, i16 1, i16 2
  tail call void @llvm.memmove.p0.p0.i16(ptr nonnull %6, ptr nonnull %6, i16 144, i1 false), !llrm.backward !36
  br label %199

199:
  %200 = phi i16 [ %210, %199 ], [ 73, %196 ]
  %201 = shl nuw nsw i16 %200, 1
  %202 = getelementptr i8, ptr %6, i16 %201
  %203 = getelementptr i8, ptr %202, i16 -4
  %204 = load i16, ptr %203
  %205 = shl nuw nsw i16 %200, 1
  %206 = getelementptr i8, ptr %6, i16 %205
  %207 = getelementptr i8, ptr %206, i16 -6
  %208 = load i16, ptr %207
  %209 = tail call i16 @llvm.fshl.i16(i16 %204, i16 %208, i16 %198)
  store i16 %209, ptr %203
  %210 = add nsw i16 %200, -1
  %211 = icmp samesign ugt i16 %210, 2
  br i1 %211, label %199, label %212

212:
  %213 = icmp eq i8 %89, 0
  %214 = load i16, ptr %6
  %215 = shl i16 %214, %198
  store i16 %215, ptr %6
  %216 = getelementptr i8, ptr %6, i16 146
  tail call void @llvm.memset.p0.i16(ptr align 2 %216, i8 0, i16 142, i1 false)
  %217 = getelementptr i8, ptr %6, i16 144
  store i16 1, ptr %217
  %218 = sub nsw i16 %99, %87
  %219 = add nuw nsw i16 %218, 1
  %220 = lshr i16 %219, 4
  %221 = and i16 %219, 15
  %222 = add i1 %213, true
  %223 = zext i1 %222 to i16
  %224 = add i16 %4, %223
  %225 = add i16 %224, 1
  %226 = call i16 @llvm.umax.i16(i16 %13, i16 1)
  %227 = sub i16 %225, %226
  %228 = lshr i16 %227, 4
  %229 = mul nsw i16 %228, -2
  %230 = getelementptr i8, ptr %6, i16 286
  br label %231

231:
  %232 = phi i16 [ %246, %242 ], [ 71, %212 ]
  %233 = phi ptr [ %245, %242 ], [ %230, %212 ]
  %234 = phi i16 [ %244, %242 ], [ -72, %212 ]
  %235 = icmp samesign ult i16 %232, %220
  br i1 %235, label %242, label %239

236:
  %237 = icmp eq i16 %221, 0
  br i1 %237, label %265, label %238

238:
  br label %248

239:
  %240 = getelementptr i8, ptr %233, i16 %229
  %241 = load i16, ptr %240
  br label %242

242:
  %243 = phi i16 [ %241, %239 ], [ 0, %231 ]
  store i16 %243, ptr %233
  %244 = add nsw i16 %234, 1
  %245 = getelementptr i8, ptr %233, i16 -2
  %246 = add nsw i16 %232, -1
  %247 = icmp eq i16 %244, 0
  br i1 %247, label %236, label %231

248:
  %249 = phi i16 [ 73, %238 ], [ %259, %248 ]
  %250 = shl nuw nsw i16 %249, 1
  %251 = getelementptr i8, ptr %6, i16 %250
  %252 = getelementptr i8, ptr %251, i16 140
  %253 = load i16, ptr %252
  %254 = shl nuw nsw i16 %249, 1
  %255 = getelementptr i8, ptr %6, i16 %254
  %256 = getelementptr i8, ptr %255, i16 138
  %257 = load i16, ptr %256
  %258 = tail call i16 @llvm.fshl.i16(i16 %253, i16 %257, i16 %221)
  store i16 %258, ptr %252
  %259 = add nsw i16 %249, -1
  %260 = icmp samesign ugt i16 %259, 2
  br i1 %260, label %248, label %261

261:
  %262 = getelementptr i8, ptr %6, i16 144
  %263 = load i16, ptr %262
  %264 = shl i16 %263, %221
  store i16 %264, ptr %262
  br label %265

265:
  %266 = getelementptr i8, ptr %6, i16 434
  tail call void @llvm.memset.p0.i16(ptr align 2 %266, i8 0, i16 142, i1 false)
  br label %267

267:
  %sink = phi i16 [ 1, %265 ], [ %195, %192 ]
  %268 = getelementptr i8, ptr %6, i16 432
  store i16 %sink, ptr %268
  br label %269

269:
  %270 = icmp eq i8 %89, 0
  tail call void @llvm.memcpy.p0.p0.i16(ptr align 2 %94, ptr align 2 %95, i16 144, i1 false), !llrm.forward !35
  br i1 %270, label %287, label %271

271:
  %272 = getelementptr i8, ptr %6, i16 430
  br label %273

273:
  %274 = phi ptr [ %272, %271 ], [ %281, %273 ]
  %275 = phi i16 [ 73, %271 ], [ %280, %273 ]
  %276 = load i16, ptr %274
  %277 = getelementptr i8, ptr %274, i16 -2
  %278 = load i16, ptr %277
  %279 = tail call i16 @llvm.fshl.i16(i16 %276, i16 %278, i16 1)
  store i16 %279, ptr %274
  %280 = add nsw i16 %275, -1
  %281 = getelementptr i8, ptr %274, i16 -2
  %282 = icmp samesign ugt i16 %280, 2
  br i1 %282, label %273, label %283

283:
  %284 = getelementptr i8, ptr %6, i16 288
  %285 = load i16, ptr %284
  %286 = shl i16 %285, 1
  store i16 %286, ptr %284
  br label %287

287:
  %288 = icmp eq i8 %92, 0
  %289 = xor i1 %288, true
  %290 = getelementptr i8, ptr %6, i16 722
  %291 = getelementptr i8, ptr %290, i16 %34
  br label %292

292:
  %293 = phi i16 [ %348, %345 ], [ 1, %287 ]
  %294 = phi ptr [ %347, %345 ], [ %291, %287 ]
  %295 = phi i16 [ 0, %287 ], [ %346, %345 ]
  tail call void @llvm.memcpy.p0.p0.i16(ptr %96, ptr nonnull %6, i16 144, i1 false)
  br label %296

296:
  %297 = phi i16 [ %311, %296 ], [ -144, %292 ]
  %298 = phi i32 [ 0, %292 ], [ %310, %296 ]
  %299 = getelementptr i8, ptr %6, i16 %297
  %300 = getelementptr i8, ptr %299, i16 720
  %301 = load i16, ptr %300
  %302 = zext i16 %301 to i32
  %303 = getelementptr i8, ptr %6, i16 %297
  %304 = getelementptr i8, ptr %303, i16 432
  %305 = load i16, ptr %304
  %306 = zext i16 %305 to i32
  %307 = add nuw nsw i32 %298, %302
  %308 = add nuw nsw i32 %307, %306
  %309 = trunc i32 %308 to i16
  store i16 %309, ptr %300
  %310 = lshr i32 %308, 16
  %311 = add nsw i16 %297, 2
  %exitcond14 = icmp eq i16 %311, 0
  br i1 %exitcond14, label %315, label %296

312:
  %313 = add nsw i16 %317, -2
  %314 = icmp eq i16 %313, 574
  br i1 %314, label %327, label %316

315:
  br label %316

316:
  %317 = phi i16 [ 718, %315 ], [ %313, %312 ]
  %318 = getelementptr i8, ptr %6, i16 %317
  %319 = load i16, ptr %318
  %320 = getelementptr i8, ptr %6, i16 %317
  %321 = getelementptr i8, ptr %320, i16 -432
  %322 = load i16, ptr %321
  %323 = icmp eq i16 %319, %322
  br i1 %323, label %312, label %324

324:
  %325 = icmp ult i16 %319, %322
  %326 = select i1 %325, i8 -1, i8 1
  br label %327

327:
  %common.op5 = phi i8 [ %326, %324 ], [ 0, %312 ]
  %328 = icmp sgt i8 %common.op5, 0
  %329 = icmp eq i8 %common.op5, 0
  %330 = and i1 %329, %289
  %331 = select i1 %328, i1 true, i1 %330
  br i1 %331, label %332, label %349

332:
  br label %333

333:
  %334 = phi i16 [ -144, %332 ], [ %344, %333 ]
  %335 = phi i32 [ %343, %333 ], [ 0, %332 ]
  %336 = getelementptr i8, ptr %6, i16 %334
  %337 = getelementptr i8, ptr %336, i16 288
  %338 = load i16, ptr %337
  %339 = zext i16 %338 to i32
  %340 = mul nuw nsw i32 %339, 10
  %341 = add nuw nsw i32 %340, %335
  %342 = trunc i32 %341 to i16
  store i16 %342, ptr %337
  %343 = lshr i32 %341, 16
  %344 = add nsw i16 %334, 2
  %exitcond13 = icmp eq i16 %344, 0
  br i1 %exitcond13, label %345, label %333

345:
  %346 = add i16 %295, 1
  %347 = getelementptr i8, ptr %294, i16 -1
  %348 = add i16 %293, -1
  br label %292

349:
  br label %350

350:
  %351 = phi i16 [ %293, %349 ], [ %448, %445 ]
  %352 = phi ptr [ %294, %349 ], [ %447, %445 ]
  %353 = phi i16 [ %446, %445 ], [ %295, %349 ]
  tail call void @llvm.memcpy.p0.p0.i16(ptr %96, ptr nonnull %6, i16 144, i1 false)
  br label %354

354:
  %355 = phi i16 [ %369, %354 ], [ -144, %350 ]
  %356 = phi i32 [ 0, %350 ], [ %368, %354 ]
  %357 = getelementptr i8, ptr %6, i16 %355
  %358 = getelementptr i8, ptr %357, i16 720
  %359 = load i16, ptr %358
  %360 = zext i16 %359 to i32
  %361 = getelementptr i8, ptr %6, i16 %355
  %362 = getelementptr i8, ptr %361, i16 432
  %363 = load i16, ptr %362
  %364 = zext i16 %363 to i32
  %365 = add nuw nsw i32 %356, %360
  %366 = add nuw nsw i32 %365, %364
  %367 = trunc i32 %366 to i16
  store i16 %367, ptr %358
  %368 = lshr i32 %366, 16
  %369 = add nsw i16 %355, 2
  %exitcond11 = icmp eq i16 %369, 0
  br i1 %exitcond11, label %370, label %354

370:
  br label %371

371:
  %372 = phi i16 [ -144, %370 ], [ %382, %371 ]
  %373 = phi i32 [ %381, %371 ], [ 0, %370 ]
  %374 = getelementptr i8, ptr %6, i16 %372
  %375 = getelementptr i8, ptr %374, i16 720
  %376 = load i16, ptr %375
  %377 = zext i16 %376 to i32
  %378 = mul nuw nsw i32 %377, 10
  %379 = add nuw nsw i32 %378, %373
  %380 = trunc i32 %379 to i16
  store i16 %380, ptr %375
  %381 = lshr i32 %379, 16
  %382 = add nsw i16 %372, 2
  %exitcond10 = icmp eq i16 %382, 0
  br i1 %exitcond10, label %386, label %371

383:
  %384 = add nsw i16 %388, -2
  %385 = icmp eq i16 %384, 574
  br i1 %385, label %398, label %387

386:
  br label %387

387:
  %388 = phi i16 [ 718, %386 ], [ %384, %383 ]
  %389 = getelementptr i8, ptr %6, i16 %388
  %390 = load i16, ptr %389
  %391 = getelementptr i8, ptr %6, i16 %388
  %392 = getelementptr i8, ptr %391, i16 -432
  %393 = load i16, ptr %392
  %394 = icmp eq i16 %390, %393
  br i1 %394, label %383, label %395

395:
  %396 = icmp ult i16 %390, %393
  %397 = select i1 %396, i8 -1, i8 1
  br label %398

398:
  %common.op4 = phi i8 [ %397, %395 ], [ 0, %383 ]
  %399 = icmp sgt i8 %common.op4, 0
  %400 = icmp eq i8 %common.op4, 0
  %401 = and i1 %400, %289
  %402 = select i1 %399, i1 true, i1 %401
  br i1 %402, label %404, label %403

403:
  br label %407

404:
  %405 = icmp eq i8 %93, 0
  %406 = xor i1 %405, true
  br label %449

407:
  %408 = phi i16 [ -144, %403 ], [ %418, %407 ]
  %409 = phi i32 [ %417, %407 ], [ 0, %403 ]
  %410 = getelementptr i8, ptr %6, i16 %408
  %411 = getelementptr i8, ptr %410, i16 144
  %412 = load i16, ptr %411
  %413 = zext i16 %412 to i32
  %414 = mul nuw nsw i32 %413, 10
  %415 = add nuw nsw i32 %414, %409
  %416 = trunc i32 %415 to i16
  store i16 %416, ptr %411
  %417 = lshr i32 %415, 16
  %418 = add nsw i16 %408, 2
  %exitcond9 = icmp eq i16 %418, 0
  br i1 %exitcond9, label %419, label %407

419:
  br label %420

420:
  %421 = phi i16 [ -144, %419 ], [ %431, %420 ]
  %422 = phi i32 [ %430, %420 ], [ 0, %419 ]
  %423 = getelementptr i8, ptr %6, i16 %421
  %424 = getelementptr i8, ptr %423, i16 432
  %425 = load i16, ptr %424
  %426 = zext i16 %425 to i32
  %427 = mul nuw nsw i32 %426, 10
  %428 = add nuw nsw i32 %427, %422
  %429 = trunc i32 %428 to i16
  store i16 %429, ptr %424
  %430 = lshr i32 %428, 16
  %431 = add nsw i16 %421, 2
  %exitcond8 = icmp eq i16 %431, 0
  br i1 %exitcond8, label %432, label %420

432:
  br label %433

433:
  %434 = phi i16 [ -144, %432 ], [ %444, %433 ]
  %435 = phi i32 [ %443, %433 ], [ 0, %432 ]
  %436 = getelementptr i8, ptr %6, i16 %434
  %437 = getelementptr i8, ptr %436, i16 576
  %438 = load i16, ptr %437
  %439 = zext i16 %438 to i32
  %440 = mul nuw nsw i32 %439, 10
  %441 = add nuw nsw i32 %440, %435
  %442 = trunc i32 %441 to i16
  store i16 %442, ptr %437
  %443 = lshr i32 %441, 16
  %444 = add nsw i16 %434, 2
  %exitcond7 = icmp eq i16 %444, 0
  br i1 %exitcond7, label %445, label %433

445:
  %446 = add i16 %353, -1
  %447 = getelementptr i8, ptr %352, i16 1
  %448 = add i16 %351, 1
  br label %350

449:
  %450 = phi i16 [ %591, %586 ], [ %351, %404 ]
  %451 = phi i16 [ 0, %404 ], [ %590, %586 ]
  br label %452

452:
  %453 = phi i16 [ %463, %452 ], [ -144, %449 ]
  %454 = phi i32 [ 0, %449 ], [ %462, %452 ]
  %455 = getelementptr i8, ptr %6, i16 %453
  %456 = getelementptr i8, ptr %455, i16 144
  %457 = load i16, ptr %456
  %458 = zext i16 %457 to i32
  %459 = mul nuw nsw i32 %458, 10
  %460 = add nuw nsw i32 %459, %454
  %461 = trunc i32 %460 to i16
  store i16 %461, ptr %456
  %462 = lshr i32 %460, 16
  %463 = add nsw i16 %453, 2
  %exitcond6 = icmp eq i16 %463, 0
  br i1 %exitcond6, label %464, label %452

464:
  br label %465

465:
  %466 = phi i16 [ -144, %464 ], [ %476, %465 ]
  %467 = phi i32 [ %475, %465 ], [ 0, %464 ]
  %468 = getelementptr i8, ptr %6, i16 %466
  %469 = getelementptr i8, ptr %468, i16 432
  %470 = load i16, ptr %469
  %471 = zext i16 %470 to i32
  %472 = mul nuw nsw i32 %471, 10
  %473 = add nuw nsw i32 %472, %467
  %474 = trunc i32 %473 to i16
  store i16 %474, ptr %469
  %475 = lshr i32 %473, 16
  %476 = add nsw i16 %466, 2
  %exitcond5 = icmp eq i16 %476, 0
  br i1 %exitcond5, label %477, label %465

477:
  br label %478

478:
  %479 = phi i16 [ -144, %477 ], [ %489, %478 ]
  %480 = phi i32 [ %488, %478 ], [ 0, %477 ]
  %481 = getelementptr i8, ptr %6, i16 %479
  %482 = getelementptr i8, ptr %481, i16 576
  %483 = load i16, ptr %482
  %484 = zext i16 %483 to i32
  %485 = mul nuw nsw i32 %484, 10
  %486 = add nuw nsw i32 %485, %480
  %487 = trunc i32 %486 to i16
  store i16 %487, ptr %482
  %488 = lshr i32 %486, 16
  %489 = add nsw i16 %479, 2
  %exitcond41 = icmp eq i16 %489, 0
  br i1 %exitcond41, label %490, label %478

490:
  br label %491

491:
  %492 = phi i8 [ %527, %526 ], [ 0, %490 ]
  br label %496

493:
  %494 = add nsw i16 %497, -2
  %495 = icmp eq i16 %494, 0
  br i1 %495, label %508, label %496

496:
  %497 = phi i16 [ %494, %493 ], [ 144, %491 ]
  %498 = getelementptr i8, ptr %6, i16 %497
  %499 = getelementptr i8, ptr %498, i16 -2
  %500 = load i16, ptr %499
  %501 = getelementptr i8, ptr %6, i16 %497
  %502 = getelementptr i8, ptr %501, i16 142
  %503 = load i16, ptr %502
  %504 = icmp eq i16 %500, %503
  br i1 %504, label %493, label %505

505:
  %506 = icmp ult i16 %500, %503
  br i1 %506, label %507, label %508

507:
  br label %531

508:
  br label %509

509:
  %510 = phi i16 [ -144, %508 ], [ %525, %509 ]
  %511 = phi i32 [ %524, %509 ], [ 0, %508 ]
  %512 = getelementptr i8, ptr %6, i16 %510
  %513 = getelementptr i8, ptr %512, i16 144
  %514 = load i16, ptr %513
  %515 = zext i16 %514 to i32
  %516 = getelementptr i8, ptr %6, i16 %510
  %517 = getelementptr i8, ptr %516, i16 288
  %518 = load i16, ptr %517
  %519 = zext i16 %518 to i32
  %520 = add nuw nsw i32 %511, %519
  %521 = sub nsw i32 %515, %520
  %522 = trunc i32 %521 to i16
  store i16 %522, ptr %513
  %523 = lshr i32 %521, 16
  %524 = and i32 %523, 1
  %525 = add nsw i16 %510, 2
  %exitcond3 = icmp eq i16 %525, 0
  br i1 %exitcond3, label %526, label %509

526:
  %527 = add i8 %492, 1
  br label %491

528:
  %529 = add nsw i16 %532, -2
  %530 = icmp eq i16 %529, 430
  br i1 %530, label %542, label %531

531:
  %532 = phi i16 [ 574, %507 ], [ %529, %528 ]
  %533 = getelementptr i8, ptr %6, i16 %532
  %534 = getelementptr i8, ptr %533, i16 -432
  %535 = load i16, ptr %534
  %536 = getelementptr i8, ptr %6, i16 %532
  %537 = load i16, ptr %536
  %538 = icmp eq i16 %535, %537
  br i1 %538, label %528, label %539

539:
  %540 = icmp ult i16 %535, %537
  %541 = select i1 %540, i8 -1, i8 1
  br label %542

542:
  %common.op2 = phi i8 [ %541, %539 ], [ 0, %528 ]
  %543 = icmp sgt i8 %common.op2, 0
  %544 = icmp eq i8 %common.op2, 0
  %545 = and i1 %544, %289
  %546 = select i1 %543, i1 true, i1 %545
  %547 = zext i1 %546 to i8
  %548 = xor i8 %547, 1
  tail call void @llvm.memcpy.p0.p0.i16(ptr %96, ptr nonnull %6, i16 144, i1 false)
  br label %549

549:
  %550 = phi i16 [ %564, %549 ], [ -144, %542 ]
  %551 = phi i32 [ 0, %542 ], [ %563, %549 ]
  %552 = getelementptr i8, ptr %6, i16 %550
  %553 = getelementptr i8, ptr %552, i16 720
  %554 = load i16, ptr %553
  %555 = zext i16 %554 to i32
  %556 = getelementptr i8, ptr %6, i16 %550
  %557 = getelementptr i8, ptr %556, i16 432
  %558 = load i16, ptr %557
  %559 = zext i16 %558 to i32
  %560 = add nuw nsw i32 %551, %555
  %561 = add nuw nsw i32 %560, %559
  %562 = trunc i32 %561 to i16
  store i16 %562, ptr %553
  %563 = lshr i32 %561, 16
  %564 = add nsw i16 %550, 2
  %exitcond12 = icmp eq i16 %564, 0
  br i1 %exitcond12, label %568, label %549

565:
  %566 = add nsw i16 %570, -2
  %567 = icmp eq i16 %566, 574
  br i1 %567, label %580, label %569

568:
  br label %569

569:
  %570 = phi i16 [ 718, %568 ], [ %566, %565 ]
  %571 = getelementptr i8, ptr %6, i16 %570
  %572 = load i16, ptr %571
  %573 = getelementptr i8, ptr %6, i16 %570
  %574 = getelementptr i8, ptr %573, i16 -432
  %575 = load i16, ptr %574
  %576 = icmp eq i16 %572, %575
  br i1 %576, label %565, label %577

577:
  %578 = icmp ult i16 %572, %575
  %579 = select i1 %578, i8 -1, i8 1
  br label %580

580:
  %common.op1 = phi i8 [ %579, %577 ], [ 0, %565 ]
  %581 = icmp sgt i8 %common.op1, 0
  %582 = icmp eq i8 %common.op1, 0
  %583 = and i1 %582, %406
  %584 = select i1 %581, i1 true, i1 %583
  %585 = icmp ne i8 %548, 0
  %or.cond3 = select i1 %585, i1 true, i1 %584
  br i1 %or.cond3, label %592, label %586

586:
  %587 = add i8 %492, 48
  %588 = getelementptr i8, ptr %6, i16 %451
  %589 = getelementptr i8, ptr %588, i16 736
  store i8 %587, ptr %589
  %590 = add i16 %451, 1
  %591 = add i16 %450, 1
  br label %449

592:
  %593 = icmp ne i8 %548, 0
  %or.cond11 = select i1 %593, i1 %584, i1 false
  br i1 %or.cond11, label %594, label %626

594:
  tail call void @llvm.memcpy.p0.p0.i16(ptr %96, ptr nonnull %6, i16 144, i1 false)
  %595 = getelementptr i8, ptr %6, i16 718
  br label %596

596:
  %597 = phi ptr [ %604, %596 ], [ %595, %594 ]
  %598 = phi i16 [ %603, %596 ], [ 73, %594 ]
  %599 = load i16, ptr %597
  %600 = getelementptr i8, ptr %597, i16 -2
  %601 = load i16, ptr %600
  %602 = tail call i16 @llvm.fshl.i16(i16 %599, i16 %601, i16 1)
  store i16 %602, ptr %597
  %603 = add nsw i16 %598, -1
  %604 = getelementptr i8, ptr %597, i16 -2
  %605 = icmp samesign ugt i16 %603, 2
  br i1 %605, label %596, label %606

606:
  %607 = getelementptr i8, ptr %6, i16 576
  %608 = load i16, ptr %607
  %609 = shl i16 %608, 1
  store i16 %609, ptr %607
  br label %613

610:
  %611 = add nsw i16 %614, -2
  %612 = icmp eq i16 %611, -144
  br i1 %612, label %624, label %613

613:
  %614 = phi i16 [ %611, %610 ], [ 0, %606 ]
  %615 = getelementptr i8, ptr %6, i16 %614
  %616 = getelementptr i8, ptr %615, i16 718
  %617 = load i16, ptr %616
  %618 = getelementptr i8, ptr %6, i16 %614
  %619 = getelementptr i8, ptr %618, i16 286
  %620 = load i16, ptr %619
  %621 = icmp eq i16 %617, %620
  br i1 %621, label %610, label %622

622:
  %623 = icmp ult i16 %617, %620
  br i1 %623, label %629, label %624

624:
  %625 = add i8 %492, 1
  br label %629

626:
  %627 = zext i1 %584 to i8
  %628 = add i8 %492, %627
  br label %629

629:
  %630 = phi i8 [ %628, %626 ], [ %625, %624 ], [ %492, %622 ]
  %631 = add i8 %630, 48
  %632 = getelementptr i8, ptr %6, i16 %451
  %633 = getelementptr i8, ptr %632, i16 736
  store i8 %631, ptr %633
  %634 = add i16 %451, 1
  %635 = add i16 %353, -1
  %636 = icmp sgt i16 %635, -5
  %637 = icmp slt i16 %635, 16
  %or.cond2 = and i1 %636, %637
  br i1 %or.cond2, label %638, label %641

638:
  %639 = icmp slt i16 %635, 0
  br i1 %639, label %648, label %640

640:
  br label %674

641:
  %642 = getelementptr i8, ptr %6, i16 736
  %643 = load i8, ptr %642
  %644 = getelementptr i8, ptr %6, i16 %34
  %645 = getelementptr i8, ptr %644, i16 720
  store i8 %643, ptr %645
  %646 = add nuw nsw i16 %34, 1
  %647 = icmp ugt i16 %634, 1
  br i1 %647, label %711, label %717

648:
  %649 = getelementptr i8, ptr %6, i16 %34
  %650 = getelementptr i8, ptr %649, i16 720
  store i8 48, ptr %650
  %651 = add nuw nsw i16 %34, 1
  %652 = getelementptr i8, ptr %6, i16 %651
  %653 = getelementptr i8, ptr %652, i16 720
  store i8 46, ptr %653
  %654 = or disjoint i16 %34, 2
  %655 = xor i16 %635, -1
  %656 = icmp eq i16 %655, 0
  br i1 %656, label %660, label %657

657:
  %658 = getelementptr i8, ptr %7, i16 %654
  tail call void @llvm.memset.p0.i16(ptr align 1 %658, i8 48, i16 %655, i1 false)
  %659 = sub nsw i16 %651, %635
  br label %660

660:
  %661 = phi i16 [ %659, %657 ], [ %654, %648 ]
  %662 = icmp eq i16 %634, 0
  br i1 %662, label %b15, label %663

663:
  %664 = add i16 %451, 1
  br label %665

665:
  %666 = phi ptr [ %352, %663 ], [ %672, %665 ]
  %667 = phi i16 [ %671, %665 ], [ 0, %663 ]
  %668 = getelementptr i8, ptr %6, i16 %667
  %669 = getelementptr i8, ptr %668, i16 736
  %670 = load i8, ptr %669
  store i8 %670, ptr %666
  %671 = add nuw i16 %667, 1
  %672 = getelementptr i8, ptr %666, i16 1
  %673 = icmp eq i16 %664, %671
  br i1 %673, label %57, label %665

674:
  %675 = phi i16 [ %691, %687 ], [ %34, %640 ]
  %676 = phi i16 [ %692, %687 ], [ 0, %640 ]
  %677 = icmp ult i16 %676, %634
  br i1 %677, label %683, label %687

678:
  %679 = getelementptr i8, ptr %6, i16 %691
  %680 = getelementptr i8, ptr %679, i16 720
  store i8 46, ptr %680
  %681 = icmp ugt i16 %634, %353
  br i1 %681, label %682, label %694

682:
  br label %698

683:
  %684 = getelementptr i8, ptr %6, i16 %676
  %685 = getelementptr i8, ptr %684, i16 736
  %686 = load i8, ptr %685
  br label %687

687:
  %688 = phi i8 [ %686, %683 ], [ 48, %674 ]
  %689 = getelementptr i8, ptr %6, i16 %675
  %690 = getelementptr i8, ptr %689, i16 720
  store i8 %688, ptr %690
  %691 = add i16 %675, 1
  %692 = add nuw i16 %676, 1
  %693 = icmp eq i16 %353, %692
  br i1 %693, label %678, label %674

694:
  %695 = getelementptr i8, ptr %6, i16 %691
  %696 = getelementptr i8, ptr %695, i16 721
  store i8 48, ptr %696
  %697 = add i16 %691, 2
  br label %b15

698:
  %699 = phi i16 [ %450, %682 ], [ %709, %698 ]
  %700 = phi ptr [ %6, %682 ], [ %708, %698 ]
  %701 = phi i16 [ -1, %682 ], [ %707, %698 ]
  %702 = getelementptr i8, ptr %700, i16 %353
  %703 = getelementptr i8, ptr %702, i16 736
  %704 = load i8, ptr %703
  %705 = getelementptr i8, ptr %700, i16 %691
  %706 = getelementptr i8, ptr %705, i16 721
  store i8 %704, ptr %706
  %707 = add i16 %701, -1
  %708 = getelementptr i8, ptr %700, i16 1
  %709 = add i16 %699, -1
  %710 = icmp eq i16 %709, 0
  br i1 %710, label %61, label %698

711:
  %712 = getelementptr i8, ptr %6, i16 %646
  %713 = getelementptr i8, ptr %712, i16 720
  store i8 46, ptr %713
  %714 = getelementptr i8, ptr %6, i16 722
  br label %728

715:
  %716 = sub i16 %34, %734
  br label %717

717:
  %718 = phi i16 [ %716, %715 ], [ %646, %641 ]
  %719 = getelementptr i8, ptr %6, i16 %718
  %720 = getelementptr i8, ptr %719, i16 720
  store i8 101, ptr %720
  %721 = icmp slt i16 %635, 0
  %722 = select i1 %721, i8 45, i8 43
  %723 = getelementptr i8, ptr %6, i16 %718
  %724 = getelementptr i8, ptr %723, i16 721
  store i8 %722, ptr %724
  %725 = add i16 %718, 2
  %726 = tail call i16 @llvm.abs.i16(i16 %635, i1 false)
  %727 = icmp ugt i16 %726, 99
  br i1 %727, label %738, label %745

728:
  %729 = phi ptr [ %735, %728 ], [ %714, %711 ]
  %730 = phi i16 [ %734, %728 ], [ -2, %711 ]
  %731 = getelementptr i8, ptr %729, i16 15
  %732 = load i8, ptr %731
  %733 = getelementptr i8, ptr %729, i16 %34
  store i8 %732, ptr %733
  %734 = add i16 %730, -1
  %735 = getelementptr i8, ptr %729, i16 1
  %736 = add i16 %451, %734
  %737 = icmp eq i16 %736, -2
  br i1 %737, label %715, label %728

738:
  %739 = udiv i16 %726, 100
  %740 = trunc i16 %739 to i8
  %741 = add i8 %740, 48
  %742 = getelementptr i8, ptr %6, i16 %725
  %743 = getelementptr i8, ptr %742, i16 720
  store i8 %741, ptr %743
  %744 = add i16 %718, 3
  br label %745

745:
  %746 = phi i16 [ %744, %738 ], [ %725, %717 ]
  %747 = udiv i16 %726, 10
  %748 = urem i16 %747, 10
  %749 = trunc nuw nsw i16 %748 to i8
  %750 = or disjoint i8 %749, 48
  %751 = getelementptr i8, ptr %6, i16 %746
  %752 = getelementptr i8, ptr %751, i16 720
  store i8 %750, ptr %752
  %753 = urem i16 %726, 10
  %754 = trunc nuw nsw i16 %753 to i8
  %755 = or disjoint i8 %754, 48
  %756 = getelementptr i8, ptr %6, i16 %746
  %757 = getelementptr i8, ptr %756, i16 721
  store i8 %755, ptr %757
  %758 = add i16 %746, 2
  br label %b15
}

define cc 1005 void @N$PR8(double %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca double
  store double %0, ptr %1, !tbaa !2
  %2 = addrspacecast ptr %1 to ptr addrspace(5)
  call cc 1006 addrspace(1) void @floats.print_float(ptr addrspace(5) %2, i16 4, i16 4, i16 2047, i16 1075)
  ret void
}

define cc 1005 void @N$PR4(float %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca float
  store float %0, ptr %1, !tbaa !2
  %2 = addrspacecast ptr %1 to ptr addrspace(5)
  call cc 1006 addrspace(1) void @floats.print_float(ptr addrspace(5) %2, i16 2, i16 7, i16 255, i16 150)
  ret void
}

declare i32 @LL$WRITE_FILE(i16, ptr addrspace(1), i16) addrspace(1)

declare void @LL$EXIT(i8) addrspace(1)

declare ptr @LL$MORE(i16) addrspace(1)

declare i16 @llvm.fshl.i16(i16, i16, i16) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare i16 @llvm.umax.i16(i16, i16) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare i16 @llvm.umin.i16(i16, i16) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare i32 @llvm.abs.i32(i32, i1 immarg) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare void @llvm.memmove.p0.p0.i16(ptr nocapture writeonly, ptr nocapture readonly, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)

declare void @llvm.memcpy.p0.p0.i16(ptr noalias nocapture writeonly, ptr noalias nocapture readonly, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)

declare i16 @llvm.abs.i16(i16, i1 immarg) nocallback nofree nosync nounwind speculatable willreturn memory(none)

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
!25 = !{i8 0, i8 2}
!26 = !{i8 0, i8 2}
!27 = !{i8 0, i8 2}
!28 = !{i8 0, i8 2}
!29 = !{i8 0, i8 2}
!30 = !{i8 0, i8 2}
!31 = !{i8 0, i8 2}
!32 = !{i8 0, i8 2}
!33 = !{i8 0, i8 2}
!34 = !{i8 0, i8 2}
!35 = !{}
!36 = !{}
