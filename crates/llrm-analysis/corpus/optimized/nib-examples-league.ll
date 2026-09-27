target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$str1 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"
@$str2 = internal constant [13 x i8] c"\08\00\06\00\06\00Rovers\00"
@$str3 = internal constant [13 x i8] c"\08\00\06\00\06\00United\00"
@$str4 = internal constant [15 x i8] c"\08\00\08\00\08\00Athletic\00"
@$str5 = internal constant [16 x i8] c"\08\00\09\00\09\00 lead on \00"
@$str6 = internal constant [9 x i8] c"\08\00\02\00\02\00: \00"
@$str7 = internal constant [13 x i8] c"\08\00\06\00\06\00 from \00"

define internal i16 @Team.points(ptr addrspace(1) %0) addrspace(1) memory(argmem: read) willreturn {
b1:
  %1 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %2 = load i16, ptr addrspace(1) %1
  %3 = mul i16 %2, 3
  %4 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %5 = load i16, ptr addrspace(1) %4
  %6 = add i16 %3, %5
  ret i16 %6
}

define internal i16 @Team.played(ptr addrspace(1) %0) addrspace(1) memory(argmem: read) willreturn {
b1:
  %1 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %2 = load i16, ptr addrspace(1) %1
  %3 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %4 = load i16, ptr addrspace(1) %3
  %5 = add i16 %2, %4
  %6 = getelementptr i8, ptr addrspace(1) %0, i16 6
  %7 = load i16, ptr addrspace(1) %6
  %8 = add i16 %5, %7
  ret i16 %8
}

define internal void @Team.record(ptr addrspace(1) %0, i16 %1, i16 %2) addrspace(1) memory(argmem: readwrite) willreturn {
b1:
  %3 = icmp sgt i16 %1, %2
  br i1 %3, label %b2, label %b3

b2:
  %4 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %5 = load i16, ptr addrspace(1) %4
  %6 = add i16 %5, 1
  store i16 %6, ptr addrspace(1) %4
  br label %b4

b3:
  %7 = icmp eq i16 %1, %2
  br i1 %7, label %b5, label %b6

b4:
  ret void

b5:
  %8 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %9 = load i16, ptr addrspace(1) %8
  %10 = add i16 %9, 1
  store i16 %10, ptr addrspace(1) %8
  br label %b4

b6:
  %11 = getelementptr i8, ptr addrspace(1) %0, i16 6
  %12 = load i16, ptr addrspace(1) %11
  %13 = add i16 %12, 1
  store i16 %13, ptr addrspace(1) %11
  br label %b4
}

define internal i16 @main() addrspace(1) {
b1:
  %0 = alloca [4 x i8]
  %1 = alloca [8 x i8]
  %2 = alloca [8 x i8]
  %3 = alloca [4 x i8]
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 4, i1 false)
  %4 = getelementptr i8, ptr @$str1, i16 6
  %5 = call addrspace(1) ptr @N$BGRW(ptr %4, i16 3, i16 2)
  %6 = getelementptr i8, ptr %5, i16 0
  %7 = getelementptr i8, ptr @$str2, i16 6
  store ptr %7, ptr %6
  %8 = getelementptr i8, ptr %5, i16 2
  %9 = getelementptr i8, ptr @$str3, i16 6
  store ptr %9, ptr %8
  %10 = getelementptr i8, ptr %5, i16 4
  %11 = getelementptr i8, ptr @$str4, i16 6
  store ptr %11, ptr %10
  %12 = getelementptr i8, ptr %5, i16 -4
  %13 = load i16, ptr %12
  br label %b2

b2:
  %14 = phi ptr [ %4, %b1 ], [ %21, %b3 ]
  %15 = phi i16 [ 0, %b1 ], [ %29, %b3 ]
  %16 = icmp ult i16 %15, %13
  br i1 %16, label %b3, label %b5

b3:
  %17 = shl i16 %15, 1
  %18 = getelementptr i8, ptr %5, i16 %17
  %19 = getelementptr i8, ptr %14, i16 -4
  %20 = load i16, ptr %19
  %21 = call addrspace(1) ptr @N$BGRW(ptr %14, i16 1, i16 8)
  %22 = shl i16 %20, 3
  %23 = getelementptr i8, ptr %21, i16 %22
  %24 = load ptr, ptr %18
  %25 = call addrspace(1) ptr @N$BCLN(ptr %24, i16 1)
  store ptr %25, ptr %23
  %26 = getelementptr i8, ptr %23, i16 2
  store i16 0, ptr %26
  %27 = getelementptr i8, ptr %23, i16 4
  store i16 0, ptr %27
  %28 = getelementptr i8, ptr %23, i16 6
  store i16 0, ptr %28
  %29 = add i16 %15, 1
  br label %b2

b5:
  %30 = icmp ne ptr %5, null
  br i1 %30, label %b7, label %b6

b6:
  call addrspace(1) void @N$BDRP(ptr %5)
  %31 = getelementptr i8, ptr %14, i16 -4
  %32 = load i16, ptr %31
  %33 = icmp ugt i16 %32, 0
  br i1 %33, label %b11, label %b12

b7:
  %34 = load i16, ptr %12
  br label %b8

b8:
  %35 = phi i16 [ 0, %b7 ], [ %40, %b10 ]
  %36 = icmp ult i16 %35, %34
  br i1 %36, label %b10, label %b6

b10:
  %37 = shl i16 %35, 1
  %38 = getelementptr i8, ptr %5, i16 %37
  %39 = load ptr, ptr %38
  call addrspace(1) void @N$BDRP(ptr %39)
  %40 = add i16 %35, 1
  br label %b8

b11:
  %41 = getelementptr i8, ptr %14, i16 0
  %42 = addrspacecast ptr %41 to ptr addrspace(1)
  %43 = getelementptr i8, ptr addrspace(1) %42, i16 2
  %44 = load i16, ptr addrspace(1) %43
  %45 = add i16 %44, 1
  store i16 %45, ptr addrspace(1) %43
  %46 = getelementptr i8, ptr addrspace(1) %42, i16 4
  %47 = load i16, ptr addrspace(1) %46
  %48 = add i16 %47, 1
  store i16 %48, ptr addrspace(1) %46
  %49 = icmp ugt i16 %32, 1
  br i1 %49, label %b15, label %b16

b12:
  call addrspace(1) void @N$EBND()
  unreachable

b15:
  %50 = getelementptr i8, ptr %14, i16 8
  %51 = addrspacecast ptr %50 to ptr addrspace(1)
  %52 = getelementptr i8, ptr addrspace(1) %51, i16 2
  %53 = load i16, ptr addrspace(1) %52
  %54 = add i16 %53, 1
  store i16 %54, ptr addrspace(1) %52
  %55 = getelementptr i8, ptr addrspace(1) %51, i16 6
  %56 = load i16, ptr addrspace(1) %55
  %57 = add i16 %56, 1
  store i16 %57, ptr addrspace(1) %55
  %58 = icmp ugt i16 %32, 2
  br i1 %58, label %b19, label %b20

b16:
  call addrspace(1) void @N$EBND()
  unreachable

b19:
  %59 = getelementptr i8, ptr %14, i16 16
  %60 = addrspacecast ptr %59 to ptr addrspace(1)
  %61 = getelementptr i8, ptr addrspace(1) %60, i16 4
  %62 = load i16, ptr addrspace(1) %61
  %63 = add i16 %62, 1
  store i16 %63, ptr addrspace(1) %61
  %64 = getelementptr i8, ptr addrspace(1) %60, i16 6
  %65 = load i16, ptr addrspace(1) %64
  %66 = add i16 %65, 1
  store i16 %66, ptr addrspace(1) %64
  br label %b23

b20:
  call addrspace(1) void @N$EBND()
  unreachable

b23:
  %67 = phi ptr [ %4, %b19 ], [ %74, %b24 ]
  %68 = phi i16 [ 0, %b19 ], [ %84, %b24 ]
  %69 = icmp ult i16 %68, %32
  br i1 %69, label %b24, label %b26

b24:
  %70 = shl i16 %68, 3
  %71 = getelementptr i8, ptr %14, i16 %70
  %72 = getelementptr i8, ptr %67, i16 -4
  %73 = load i16, ptr %72
  %74 = call addrspace(1) ptr @N$BGRW(ptr %67, i16 1, i16 2)
  %75 = shl i16 %73, 1
  %76 = getelementptr i8, ptr %74, i16 %75
  %77 = addrspacecast ptr %71 to ptr addrspace(1)
  %78 = getelementptr i8, ptr addrspace(1) %77, i16 2
  %79 = load i16, ptr addrspace(1) %78
  %80 = mul i16 %79, 3
  %81 = getelementptr i8, ptr addrspace(1) %77, i16 4
  %82 = load i16, ptr addrspace(1) %81
  %83 = add i16 %80, %82
  store i16 %83, ptr %76
  %84 = add i16 %68, 1
  br label %b23

b26:
  %85 = getelementptr i8, ptr %67, i16 -4
  %86 = load i16, ptr %85
  %87 = addrspacecast ptr %67 to ptr addrspace(1)
  call void @llvm.memset.p0.i16(ptr %0, i8 0, i16 4, i1 false)
  %88 = icmp ugt i16 %86, 0
  br i1 %88, label %89, label %92

89:
  %90 = getelementptr i8, ptr addrspace(1) %87, i16 0
  %91 = load i16, ptr addrspace(1) %90
  br label %93

92:
  call addrspace(1) void @N$EBND()
  unreachable

93:
  %94 = phi i16 [ 0, %89 ], [ %117, %116 ]
  %95 = phi i16 [ %91, %89 ], [ %118, %116 ]
  %96 = phi i16 [ 0, %89 ], [ %119, %116 ]
  %97 = phi i16 [ 0, %89 ], [ %120, %116 ]
  %98 = icmp ult i16 %97, %86
  br i1 %98, label %99, label %104

99:
  %100 = shl i16 %97, 1
  %101 = getelementptr i8, ptr addrspace(1) %87, i16 %100
  %102 = load i16, ptr addrspace(1) %101
  %103 = icmp sgt i16 %102, %95
  br i1 %103, label %114, label %116

104:
  store i16 %94, ptr %0
  %105 = getelementptr inbounds i8, ptr %0, i16 2
  store i16 %95, ptr %105
  %106 = addrspacecast ptr %0 to ptr addrspace(1)
  %107 = load i32, ptr addrspace(1) %106
  %108 = addrspacecast ptr %3 to ptr addrspace(1)
  store i32 %107, ptr addrspace(1) %108, !tbaa !2
  %109 = load i16, ptr %3, !tbaa !2
  %110 = getelementptr inbounds i8, ptr %3, i16 2
  %111 = load i16, ptr %110, !tbaa !2
  %112 = load i16, ptr %31
  %113 = icmp ult i16 %109, %112
  br i1 %113, label %b27, label %b28

114:
  %115 = load i16, ptr addrspace(1) %101
  br label %116

116:
  %117 = phi i16 [ %96, %114 ], [ %94, %99 ]
  %118 = phi i16 [ %115, %114 ], [ %95, %99 ]
  %119 = add i16 %96, 1
  %120 = add i16 %97, 1
  br label %93

b27:
  %121 = shl i16 %109, 3
  %122 = getelementptr i8, ptr %14, i16 %121
  %123 = load ptr, ptr %122
  call addrspace(1) void @N$PS(ptr %123)
  %124 = getelementptr i8, ptr @$str5, i16 6
  call addrspace(1) void @N$PS(ptr %124)
  call addrspace(1) void @N$PI2(i16 %111)
  call addrspace(1) void @N$PN()
  %125 = load i16, ptr %85
  store i16 %125, ptr %2, !tbaa !2
  %126 = getelementptr inbounds i8, ptr %2, i16 2
  store i16 %125, ptr %126, !tbaa !2
  %127 = getelementptr inbounds i8, ptr %2, i16 4
  store ptr addrspace(1) %87, ptr %127, !tbaa !2
  %128 = addrspacecast ptr %2 to ptr addrspace(1)
  %129 = getelementptr i8, ptr addrspace(1) %128, i16 4
  %130 = getelementptr inbounds i8, ptr %1, i16 2
  %131 = getelementptr inbounds i8, ptr %1, i16 4
  %132 = addrspacecast ptr %1 to ptr addrspace(1)
  %133 = getelementptr i8, ptr @$str6, i16 6
  %134 = getelementptr i8, ptr @$str7, i16 6
  br label %b30

b28:
  call addrspace(1) void @N$EBND()
  unreachable

b29:
  call addrspace(1) void @N$BDRP(ptr %67)
  call addrspace(1) void @N$BDRP(ptr null)
  %135 = icmp ne ptr %14, null
  br i1 %135, label %b41, label %b40

b30:
  %136 = phi i16 [ 0, %b27 ], [ %146, %b36 ]
  %137 = phi i16 [ 0, %b27 ], [ %147, %b36 ]
  %138 = icmp ult i16 %137, %125
  br i1 %138, label %b31, label %b29

b31:
  %139 = load ptr addrspace(1), ptr addrspace(1) %129, !tbaa !2
  %140 = shl i16 %137, 1
  %141 = getelementptr i8, ptr addrspace(1) %139, i16 %140
  %142 = load i16, ptr addrspace(1) %141
  %143 = icmp sge i16 %142, 2
  br i1 %143, label %b34, label %b36

b34:
  %144 = load i16, ptr %31
  %145 = icmp ult i16 %136, %144
  br i1 %145, label %b38, label %b39

b36:
  %146 = add i16 %136, 1
  %147 = add i16 %137, 1
  br label %b30

b38:
  %148 = shl i16 %136, 3
  %149 = getelementptr i8, ptr %14, i16 %148
  %150 = load ptr, ptr %149
  %151 = getelementptr i8, ptr %149, i16 2
  %152 = load i16, ptr %151
  %153 = getelementptr i8, ptr %149, i16 4
  %154 = load i16, ptr %153
  %155 = getelementptr i8, ptr %149, i16 6
  %156 = load i16, ptr %155
  %157 = call addrspace(1) ptr @N$BCLN(ptr %150, i16 1)
  %158 = getelementptr i8, ptr %157, i16 -4
  %159 = load i16, ptr %158
  %160 = addrspacecast ptr %157 to ptr addrspace(1)
  store i16 %159, ptr %1, !tbaa !2
  store i16 %159, ptr %130, !tbaa !2
  store ptr addrspace(1) %160, ptr %131, !tbaa !2
  %161 = mul i16 %152, 3
  %162 = add i16 %161, %154
  %163 = add i16 %152, %154
  %164 = add i16 %163, %156
  call addrspace(1) void @N$PV(ptr addrspace(1) %132)
  call addrspace(1) void @N$PS(ptr %133)
  call addrspace(1) void @N$PI2(i16 %162)
  call addrspace(1) void @N$PS(ptr %134)
  call addrspace(1) void @N$PI2(i16 %164)
  call addrspace(1) void @N$PN()
  call addrspace(1) void @N$BDRP(ptr %157)
  br label %b36

b39:
  call addrspace(1) void @N$EBND()
  unreachable

b40:
  call addrspace(1) void @N$BDRP(ptr %14)
  ret i16 0

b41:
  %165 = load i16, ptr %31
  br label %b42

b42:
  %166 = phi i16 [ 0, %b41 ], [ %171, %b44 ]
  %167 = icmp ult i16 %166, %165
  br i1 %167, label %b44, label %b40

b44:
  %168 = shl i16 %166, 3
  %169 = getelementptr i8, ptr %14, i16 %168
  %170 = load ptr, ptr %169
  call addrspace(1) void @N$BDRP(ptr %170)
  %171 = add i16 %166, 1
  br label %b42
}

define internal i32 @"best[i16]"(ptr addrspace(1) noalias readonly dereferenceable(8) %0) addrspace(1) {
b1:
  %1 = alloca [4 x i8]
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 4, i1 false)
  %2 = load i16, ptr addrspace(1) %0
  %3 = icmp ugt i16 %2, 0
  br i1 %3, label %b2, label %b3

b2:
  %4 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %5 = load ptr addrspace(1), ptr addrspace(1) %4
  %6 = getelementptr i8, ptr addrspace(1) %5, i16 0
  %7 = load i16, ptr addrspace(1) %6
  br label %b4

b3:
  call addrspace(1) void @N$EBND()
  unreachable

b4:
  %8 = phi i16 [ 0, %b2 ], [ %21, %b10 ]
  %9 = phi i16 [ %7, %b2 ], [ %22, %b10 ]
  %10 = phi i16 [ 0, %b2 ], [ %23, %b10 ]
  %11 = phi i16 [ 0, %b2 ], [ %24, %b10 ]
  %12 = icmp ult i16 %11, %2
  br i1 %12, label %b5, label %b7

b5:
  %13 = shl i16 %11, 1
  %14 = getelementptr i8, ptr addrspace(1) %5, i16 %13
  %15 = load i16, ptr addrspace(1) %14
  %16 = icmp sgt i16 %15, %9
  br i1 %16, label %b8, label %b10

b7:
  store i16 %8, ptr %1, !tbaa !2
  %17 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 %9, ptr %17, !tbaa !2
  %18 = addrspacecast ptr %1 to ptr addrspace(1)
  %19 = load i32, ptr addrspace(1) %18, !tbaa !2
  ret i32 %19

b8:
  %20 = load i16, ptr addrspace(1) %14
  br label %b10

b10:
  %21 = phi i16 [ %10, %b8 ], [ %8, %b5 ]
  %22 = phi i16 [ %20, %b8 ], [ %9, %b5 ]
  %23 = add i16 %10, 1
  %24 = add i16 %11, 1
  br label %b4
}

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare ptr @N$BGRW(ptr, i16, i16) addrspace(1)

declare ptr @N$BCLN(ptr, i16) addrspace(1)

declare void @N$BDRP(ptr) addrspace(1)

declare void @N$EBND() addrspace(1)

declare void @N$PS(ptr) addrspace(1)

declare void @N$PI2(i16) addrspace(1)

declare void @N$PN() addrspace(1)

declare void @N$PV(ptr addrspace(1)) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
