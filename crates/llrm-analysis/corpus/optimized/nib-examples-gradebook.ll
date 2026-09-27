target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$str1 = internal constant [10 x i8] c"\08\00\03\00\03\00ada\00"
@$str2 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"
@$str3 = internal constant [9 x i8] c"\08\00\02\00\02\00: \00"
@$str4 = internal constant [21 x i8] c"\08\00\0E\00\0E\00 marks, first \00"
@$str5 = internal constant [14 x i8] c"\08\00\07\00\07\00recent \00"
@$str6 = internal constant [17 x i8] c"\08\00\0A\00\0A\00, average \00"
@$str7 = internal constant [12 x i8] c"\08\00\05\00\05\00best \00"
@$str8 = internal constant [21 x i8] c"\08\00\0E\00\0E\00week 2 day 2: \00"
@$str9 = internal constant [20 x i8] c"\08\00\0D\00\0D\00, week 1 has \00"

define internal void @curve(ptr addrspace(1) %0, i16 %1) addrspace(1) {
b1:
  %2 = load ptr, ptr addrspace(1) %0
  %3 = getelementptr i8, ptr %2, i16 -4
  %4 = load i16, ptr %3
  br label %b2

b2:
  %5 = phi i16 [ 0, %b1 ], [ %21, %b6 ]
  %6 = icmp ult i16 %5, %4
  br i1 %6, label %b3, label %b5

b3:
  %7 = load ptr, ptr addrspace(1) %0
  %8 = getelementptr i8, ptr %7, i16 -4
  %9 = load i16, ptr %8
  %10 = icmp ult i16 %5, %9
  br i1 %10, label %b6, label %b7

b5:
  %11 = load ptr, ptr addrspace(1) %0
  %12 = getelementptr i8, ptr %11, i16 -4
  %13 = load i16, ptr %12
  %14 = call addrspace(1) ptr @N$BGRW(ptr %11, i16 1, i16 2)
  store ptr %14, ptr addrspace(1) %0
  %15 = shl i16 %13, 1
  %16 = getelementptr i8, ptr %14, i16 %15
  store i16 5, ptr %16
  ret void

b6:
  %17 = shl i16 %5, 1
  %18 = getelementptr i8, ptr %7, i16 %17
  %19 = load i16, ptr %18
  %20 = add i16 %19, 5
  store i16 %20, ptr %18
  %21 = add i16 %5, 1
  br label %b2

b7:
  call addrspace(1) void @N$EBND()
  unreachable
}

define internal i16 @average(ptr addrspace(1) noalias readonly dereferenceable(8) %0) addrspace(1) willreturn {
b1:
  %1 = load i16, ptr addrspace(1) %0
  %2 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %3 = load ptr addrspace(1), ptr addrspace(1) %2
  br label %b2

b2:
  %4 = phi i16 [ 0, %b1 ], [ %10, %b3 ]
  %5 = phi i16 [ 0, %b1 ], [ %11, %b3 ]
  %6 = icmp ult i16 %5, %1
  br i1 %6, label %b3, label %b5

b3:
  %7 = shl i16 %5, 1
  %8 = getelementptr i8, ptr addrspace(1) %3, i16 %7
  %9 = load i16, ptr addrspace(1) %8
  %10 = add i16 %4, %9
  %11 = add i16 %5, 1
  br label %b2

b5:
  %12 = sdiv i16 %4, %1
  %13 = srem i16 %4, %1
  %14 = icmp ne i16 %13, 0
  %15 = sext i1 %14 to i8
  %16 = xor i16 %13, %1
  %17 = icmp slt i16 %16, 0
  %18 = sext i1 %17 to i8
  %19 = and i8 %15, %18
  %20 = sext i8 %19 to i16
  %21 = and i16 %20, 1
  %22 = sub i16 %12, %21
  ret i16 %22
}

define internal i16 @main() addrspace(1) {
b1:
  %0 = alloca [8 x i8]
  %1 = alloca [4 x i8]
  call void @llvm.memset.p0.i16(ptr %0, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 4, i1 false)
  %2 = getelementptr i8, ptr @$str1, i16 6
  %3 = getelementptr i8, ptr @$str2, i16 6
  store ptr %2, ptr %1, !tbaa !2
  %4 = getelementptr inbounds i8, ptr %1, i16 2
  store ptr %3, ptr %4, !tbaa !2
  %5 = getelementptr i8, ptr %3, i16 -4
  %6 = load i16, ptr %5
  %7 = call addrspace(1) ptr @N$BGRW(ptr %3, i16 1, i16 2)
  store ptr %7, ptr %4, !tbaa !2
  %8 = shl i16 %6, 1
  %9 = getelementptr i8, ptr %7, i16 %8
  store i16 71, ptr %9
  %10 = getelementptr i8, ptr %7, i16 -4
  %11 = load i16, ptr %10
  %12 = call addrspace(1) ptr @N$BGRW(ptr %7, i16 1, i16 2)
  store ptr %12, ptr %4, !tbaa !2
  %13 = shl i16 %11, 1
  %14 = getelementptr i8, ptr %12, i16 %13
  store i16 64, ptr %14
  %15 = getelementptr i8, ptr %12, i16 -4
  %16 = load i16, ptr %15
  %17 = call addrspace(1) ptr @N$BGRW(ptr %12, i16 1, i16 2)
  store ptr %17, ptr %4, !tbaa !2
  %18 = shl i16 %16, 1
  %19 = getelementptr i8, ptr %17, i16 %18
  store i16 80, ptr %19
  call addrspace(1) void @N$PS(ptr %2)
  %20 = getelementptr i8, ptr @$str3, i16 6
  call addrspace(1) void @N$PS(ptr %20)
  %21 = getelementptr i8, ptr %17, i16 -4
  %22 = load i16, ptr %21
  call addrspace(1) void @N$PU2(i16 %22)
  %23 = getelementptr i8, ptr @$str4, i16 6
  call addrspace(1) void @N$PS(ptr %23)
  %24 = load i16, ptr %21
  %25 = icmp ugt i16 %24, 0
  br i1 %25, label %b2, label %b3

b2:
  %26 = getelementptr i8, ptr %17, i16 0
  %27 = load i16, ptr %26
  call addrspace(1) void @N$PI2(i16 %27)
  call addrspace(1) void @N$PN()
  %28 = addrspacecast ptr %4 to ptr addrspace(1)
  %29 = load i16, ptr %21
  br label %30

30:
  %31 = phi i16 [ 0, %b2 ], [ %52, %47 ]
  %32 = icmp ult i16 %31, %29
  br i1 %32, label %33, label %36

33:
  %34 = load i16, ptr %21
  %35 = icmp ult i16 %31, %34
  br i1 %35, label %47, label %53

36:
  %37 = load ptr, ptr addrspace(1) %28
  %38 = getelementptr i8, ptr %37, i16 -4
  %39 = load i16, ptr %38
  %40 = call addrspace(1) ptr @N$BGRW(ptr %37, i16 1, i16 2)
  store ptr %40, ptr addrspace(1) %28
  %41 = shl i16 %39, 1
  %42 = getelementptr i8, ptr %40, i16 %41
  store i16 5, ptr %42
  %43 = getelementptr i8, ptr %40, i16 -4
  %44 = load i16, ptr %43
  %45 = addrspacecast ptr %40 to ptr addrspace(1)
  %46 = icmp uge i16 %44, 1
  br i1 %46, label %b4, label %b5

47:
  %48 = shl i16 %31, 1
  %49 = getelementptr i8, ptr %17, i16 %48
  %50 = load i16, ptr %49
  %51 = add i16 %50, 5
  store i16 %51, ptr %49
  %52 = add i16 %31, 1
  br label %30

53:
  call addrspace(1) void @N$EBND()
  unreachable

b3:
  call addrspace(1) void @N$EBND()
  unreachable

b4:
  %54 = getelementptr i8, ptr addrspace(1) %45, i16 2
  %55 = add i16 %44, -1
  store i16 %55, ptr %0, !tbaa !2
  %56 = getelementptr inbounds i8, ptr %0, i16 2
  store i16 %55, ptr %56, !tbaa !2
  %57 = getelementptr inbounds i8, ptr %0, i16 4
  store ptr addrspace(1) %54, ptr %57, !tbaa !2
  %58 = addrspacecast ptr %0 to ptr addrspace(1)
  br label %59

59:
  %60 = phi i16 [ 0, %b4 ], [ %67, %63 ]
  %61 = phi i16 [ 0, %b4 ], [ %68, %63 ]
  %62 = icmp ult i16 %61, %55
  br i1 %62, label %63, label %69

63:
  %64 = shl i16 %61, 1
  %65 = getelementptr i8, ptr addrspace(1) %54, i16 %64
  %66 = load i16, ptr addrspace(1) %65
  %67 = add i16 %60, %66
  %68 = add i16 %61, 1
  br label %59

69:
  %70 = load i16, ptr addrspace(1) %58
  %71 = sdiv i16 %60, %70
  %72 = srem i16 %60, %70
  %73 = icmp ne i16 %72, 0
  %74 = sext i1 %73 to i8
  %75 = xor i16 %72, %70
  %76 = icmp slt i16 %75, 0
  %77 = sext i1 %76 to i8
  %78 = and i8 %74, %77
  %79 = sext i8 %78 to i16
  %80 = and i16 %79, 1
  %81 = sub i16 %71, %80
  %82 = getelementptr i8, ptr @$str5, i16 6
  call addrspace(1) void @N$PS(ptr %82)
  call addrspace(1) void @N$PU2(i16 %55)
  %83 = getelementptr i8, ptr @$str6, i16 6
  call addrspace(1) void @N$PS(ptr %83)
  call addrspace(1) void @N$PI2(i16 %81)
  call addrspace(1) void @N$PN()
  %84 = load ptr, ptr %4, !tbaa !2
  %85 = getelementptr i8, ptr %84, i16 -4
  %86 = load i16, ptr %85
  %87 = addrspacecast ptr %84 to ptr addrspace(1)
  br label %b6

b5:
  call addrspace(1) void @N$EBND()
  unreachable

b6:
  %88 = phi i16 [ 0, %69 ], [ %117, %b12 ]
  %89 = phi i16 [ 0, %69 ], [ %118, %b12 ]
  %90 = icmp ult i16 %89, %86
  br i1 %90, label %b7, label %b9

b7:
  %91 = shl i16 %89, 1
  %92 = getelementptr i8, ptr addrspace(1) %87, i16 %91
  %93 = load i16, ptr addrspace(1) %92
  %94 = icmp sgt i16 %93, %88
  br i1 %94, label %b10, label %b12

b9:
  %95 = getelementptr i8, ptr @$str7, i16 6
  call addrspace(1) void @N$PS(ptr %95)
  call addrspace(1) void @N$PI2(i16 %88)
  call addrspace(1) void @N$PN()
  %96 = load i16, ptr %5
  %97 = call addrspace(1) ptr @N$BGRW(ptr %3, i16 1, i16 2)
  %98 = shl i16 %96, 1
  %99 = getelementptr i8, ptr %97, i16 %98
  %100 = call addrspace(1) ptr @N$BGRW(ptr %3, i16 3, i16 2)
  %101 = getelementptr i8, ptr %100, i16 0
  store i16 1, ptr %101
  %102 = getelementptr i8, ptr %100, i16 2
  store i16 2, ptr %102
  %103 = getelementptr i8, ptr %100, i16 4
  store i16 3, ptr %103
  store ptr %100, ptr %99
  %104 = getelementptr i8, ptr %97, i16 -4
  %105 = load i16, ptr %104
  %106 = call addrspace(1) ptr @N$BGRW(ptr %97, i16 1, i16 2)
  %107 = shl i16 %105, 1
  %108 = getelementptr i8, ptr %106, i16 %107
  %109 = call addrspace(1) ptr @N$BGRW(ptr %3, i16 2, i16 2)
  %110 = getelementptr i8, ptr %109, i16 0
  store i16 4, ptr %110
  %111 = getelementptr i8, ptr %109, i16 2
  store i16 5, ptr %111
  store ptr %109, ptr %108
  %112 = getelementptr i8, ptr @$str8, i16 6
  call addrspace(1) void @N$PS(ptr %112)
  %113 = getelementptr i8, ptr %106, i16 -4
  %114 = load i16, ptr %113
  %115 = icmp ugt i16 %114, 1
  br i1 %115, label %b13, label %b14

b10:
  %116 = load i16, ptr addrspace(1) %92
  br label %b12

b12:
  %117 = phi i16 [ %116, %b10 ], [ %88, %b7 ]
  %118 = add i16 %89, 1
  br label %b6

b13:
  %119 = getelementptr i8, ptr %106, i16 2
  %120 = load ptr, ptr %119
  %121 = getelementptr i8, ptr %120, i16 -4
  %122 = load i16, ptr %121
  %123 = icmp ugt i16 %122, 1
  br i1 %123, label %b15, label %b16

b14:
  call addrspace(1) void @N$EBND()
  unreachable

b15:
  %124 = getelementptr i8, ptr %120, i16 2
  %125 = load i16, ptr %124
  call addrspace(1) void @N$PI2(i16 %125)
  %126 = getelementptr i8, ptr @$str9, i16 6
  call addrspace(1) void @N$PS(ptr %126)
  %127 = load i16, ptr %113
  %128 = icmp ugt i16 %127, 0
  br i1 %128, label %b17, label %b18

b16:
  call addrspace(1) void @N$EBND()
  unreachable

b17:
  %129 = getelementptr i8, ptr %106, i16 0
  %130 = load ptr, ptr %129
  %131 = getelementptr i8, ptr %130, i16 -4
  %132 = load i16, ptr %131
  call addrspace(1) void @N$PU2(i16 %132)
  call addrspace(1) void @N$PN()
  %133 = icmp ne ptr %106, null
  br i1 %133, label %b20, label %b19

b18:
  call addrspace(1) void @N$EBND()
  unreachable

b19:
  call addrspace(1) void @N$BDRP(ptr %106)
  %134 = load ptr, ptr %4, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %134)
  %135 = load ptr, ptr %1, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %135)
  ret i16 0

b20:
  %136 = load i16, ptr %113
  br label %b21

b21:
  %137 = phi i16 [ 0, %b20 ], [ %142, %b23 ]
  %138 = icmp ult i16 %137, %136
  br i1 %138, label %b23, label %b19

b23:
  %139 = shl i16 %137, 1
  %140 = getelementptr i8, ptr %106, i16 %139
  %141 = load ptr, ptr %140
  call addrspace(1) void @N$BDRP(ptr %141)
  %142 = add i16 %137, 1
  br label %b21
}

declare ptr @N$BGRW(ptr, i16, i16) addrspace(1)

declare void @N$EBND() addrspace(1)

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare void @N$PS(ptr) addrspace(1)

declare void @N$PU2(i16) addrspace(1)

declare void @N$PI2(i16) addrspace(1)

declare void @N$PN() addrspace(1)

declare void @N$BDRP(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
