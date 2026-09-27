target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$str1 = internal constant [11 x i8] c"\08\00\04\00\04\00lap \00"
@$str2 = internal constant [9 x i8] c"\08\00\02\00\02\00: \00"
@$str3 = internal constant [8 x i8] c"\08\00\01\00\01\00s\00"
@$str4 = internal constant [19 x i8] c"\08\00\0C\00\0C\00fastest lap \00"
@$str5 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"
@$str6 = internal constant [10 x i8] c"\08\00\03\00\03\00ada\00"
@$str7 = internal constant [10 x i8] c"\08\00\03\00\03\00bob\00"
@$str8 = internal constant [13 x i8] c"\08\00\06\00\06\00 made \00"
@$str9 = internal constant [11 x i8] c"\08\00\04\00\04\00 is \00"
@$str10 = internal constant [13 x i8] c"\08\00\06\00\06\00 short\00"
@$str11 = internal constant [15 x i8] c"\08\00\08\00\08\00s pace: \00"
@$str12 = internal constant [20 x i8] c"\08\00\0D\00\0D\00s for 12 laps\00"

define internal i16 @main() addrspace(1) {
b1:
  %0 = alloca [8 x i8]
  %1 = alloca [8 x i8]
  %2 = alloca [4 x i8]
  %3 = alloca [8 x i8]
  %4 = alloca [8 x i8]
  call void @llvm.memset.p0.i16(ptr %0, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 4, i1 false)
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %4, i8 0, i16 8, i1 false)
  %5 = getelementptr inbounds i16, ptr %4, i16 0
  store i16 62, ptr %5, !tbaa !2
  %6 = getelementptr inbounds i16, ptr %4, i16 1
  store i16 58, ptr %6, !tbaa !2
  %7 = getelementptr inbounds i16, ptr %4, i16 2
  store i16 60, ptr %7, !tbaa !2
  %8 = getelementptr inbounds i16, ptr %4, i16 3
  store i16 57, ptr %8, !tbaa !2
  %9 = addrspacecast ptr %4 to ptr addrspace(1)
  store i16 4, ptr %3, !tbaa !2
  %10 = getelementptr inbounds i8, ptr %3, i16 2
  store i16 4, ptr %10, !tbaa !2
  %11 = getelementptr inbounds i8, ptr %3, i16 4
  store ptr addrspace(1) %9, ptr %11, !tbaa !2
  %12 = addrspacecast ptr %3 to ptr addrspace(1)
  %13 = getelementptr i8, ptr addrspace(1) %12, i16 4
  %14 = getelementptr i8, ptr @$str1, i16 6
  %15 = getelementptr i8, ptr @$str2, i16 6
  %16 = getelementptr i8, ptr @$str3, i16 6
  br label %b3

b3:
  %17 = phi i16 [ 0, %b1 ], [ %41, %b12 ]
  %18 = phi i16 [ 0, %b1 ], [ %24, %b12 ]
  %19 = phi i16 [ 0, %b1 ], [ %42, %b12 ]
  %20 = icmp ult i16 %19, 4
  br i1 %20, label %b4, label %b6

b4:
  %21 = load ptr addrspace(1), ptr addrspace(1) %13, !tbaa !2
  %22 = shl i16 %19, 1
  %23 = getelementptr i8, ptr addrspace(1) %21, i16 %22
  call addrspace(1) void @N$PS(ptr %14)
  %24 = add i16 %18, 1
  call addrspace(1) void @N$PU2(i16 %24)
  call addrspace(1) void @N$PS(ptr %15)
  %25 = load i16, ptr addrspace(1) %23
  call addrspace(1) void @N$PI2(i16 %25)
  call addrspace(1) void @N$PS(ptr %16)
  call addrspace(1) void @N$PN()
  %26 = load i16, ptr addrspace(1) %23
  %27 = icmp ult i16 %17, 4
  br i1 %27, label %b8, label %b9

b6:
  %28 = getelementptr i8, ptr @$str4, i16 6
  call addrspace(1) void @N$PS(ptr %28)
  %29 = add i16 %17, 1
  call addrspace(1) void @N$PU2(i16 %29)
  call addrspace(1) void @N$PN()
  %30 = getelementptr i8, ptr @$str5, i16 6
  %31 = call addrspace(1) ptr @N$BGRW(ptr %30, i16 2, i16 4)
  %32 = getelementptr i8, ptr %31, i16 0
  %33 = getelementptr i8, ptr @$str6, i16 6
  store ptr %33, ptr %32
  %34 = getelementptr i8, ptr %32, i16 2
  store i16 12, ptr %34
  %35 = getelementptr i8, ptr %31, i16 4
  %36 = getelementptr i8, ptr @$str7, i16 6
  store ptr %36, ptr %35
  %37 = getelementptr i8, ptr %35, i16 2
  store i16 9, ptr %37
  br label %b13

b8:
  %38 = getelementptr inbounds i16, ptr %4, i16 %17
  %39 = load i16, ptr %38, !tbaa !2
  %40 = icmp slt i16 %26, %39
  br i1 %40, label %b12, label %b11

b9:
  call addrspace(1) void @N$EBND()
  unreachable

b11:
  br label %b12

b12:
  %41 = phi i16 [ %18, %b8 ], [ %17, %b11 ]
  %42 = add i16 %19, 1
  br label %b3

b13:
  %43 = phi i16 [ 0, %b6 ], [ %46, %b14 ]
  %44 = icmp slt i16 %43, 2
  br i1 %44, label %b14, label %b16

b14:
  %45 = getelementptr inbounds i16, ptr %2, i16 %43
  store i16 10, ptr %45, !tbaa !2
  %46 = add i16 %43, 1
  br label %b13

b16:
  %47 = getelementptr i8, ptr %31, i16 -4
  %48 = load i16, ptr %47
  %49 = addrspacecast ptr %31 to ptr addrspace(1)
  store i16 %48, ptr %1, !tbaa !2
  %50 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 %48, ptr %50, !tbaa !2
  %51 = getelementptr inbounds i8, ptr %1, i16 4
  store ptr addrspace(1) %49, ptr %51, !tbaa !2
  %52 = addrspacecast ptr %1 to ptr addrspace(1)
  %53 = addrspacecast ptr %2 to ptr addrspace(1)
  store i16 2, ptr %0, !tbaa !2
  %54 = getelementptr inbounds i8, ptr %0, i16 2
  store i16 2, ptr %54, !tbaa !2
  %55 = getelementptr inbounds i8, ptr %0, i16 4
  store ptr addrspace(1) %53, ptr %55, !tbaa !2
  %56 = addrspacecast ptr %0 to ptr addrspace(1)
  %57 = getelementptr i8, ptr addrspace(1) %52, i16 4
  %58 = getelementptr i8, ptr addrspace(1) %56, i16 4
  %59 = getelementptr i8, ptr @$str9, i16 6
  %60 = getelementptr i8, ptr @$str10, i16 6
  %61 = getelementptr i8, ptr @$str8, i16 6
  br label %b18

b17:
  %62 = getelementptr i8, ptr @$str11, i16 6
  %63 = getelementptr i8, ptr @$str12, i16 6
  br label %b31

b18:
  %64 = phi i16 [ 0, %b16 ], [ %75, %b23 ]
  %65 = load i16, ptr addrspace(1) %52
  %66 = icmp ult i16 %64, %65
  %67 = sext i1 %66 to i8
  br i1 %66, label %b21, label %b22

b19:
  %68 = load i16, ptr addrspace(1) %52, !tbaa !2
  %69 = icmp ult i16 %64, %68
  br i1 %69, label %b24, label %b25

b21:
  %70 = load i16, ptr addrspace(1) %56
  %71 = icmp ult i16 %64, %70
  %72 = sext i1 %71 to i8
  br label %b22

b22:
  %73 = phi i8 [ %67, %b18 ], [ %72, %b21 ]
  %74 = icmp ne i8 %73, 0
  br i1 %74, label %b19, label %b17

b23:
  %75 = add i16 %64, 1
  br label %b18

b24:
  %76 = load ptr addrspace(1), ptr addrspace(1) %57, !tbaa !2
  %77 = shl i16 %64, 2
  %78 = getelementptr i8, ptr addrspace(1) %76, i16 %77
  %79 = load i16, ptr addrspace(1) %56, !tbaa !2
  %80 = icmp ult i16 %64, %79
  br i1 %80, label %b26, label %b27

b25:
  call addrspace(1) void @N$EBND()
  unreachable

b26:
  %81 = load ptr addrspace(1), ptr addrspace(1) %58, !tbaa !2
  %82 = shl i16 %64, 1
  %83 = getelementptr i8, ptr addrspace(1) %81, i16 %82
  %84 = getelementptr i8, ptr addrspace(1) %78, i16 2
  %85 = load i16, ptr addrspace(1) %84
  %86 = load i16, ptr addrspace(1) %83
  %87 = icmp sge i16 %85, %86
  br i1 %87, label %b28, label %b29

b27:
  call addrspace(1) void @N$EBND()
  unreachable

b28:
  %88 = load ptr, ptr addrspace(1) %78
  call addrspace(1) void @N$PS(ptr %88)
  call addrspace(1) void @N$PS(ptr %61)
  %89 = load i16, ptr addrspace(1) %83
  call addrspace(1) void @N$PI2(i16 %89)
  call addrspace(1) void @N$PN()
  br label %b23

b29:
  %90 = load ptr, ptr addrspace(1) %78
  call addrspace(1) void @N$PS(ptr %90)
  call addrspace(1) void @N$PS(ptr %59)
  %91 = load i16, ptr addrspace(1) %83
  %92 = load i16, ptr addrspace(1) %84
  %93 = sub i16 %91, %92
  call addrspace(1) void @N$PI2(i16 %93)
  call addrspace(1) void @N$PS(ptr %60)
  call addrspace(1) void @N$PN()
  br label %b23

b31:
  %94 = phi i16 [ 58, %b17 ], [ %97, %b32 ]
  %95 = icmp slt i16 %94, 61
  br i1 %95, label %b32, label %b34

b32:
  call addrspace(1) void @N$PI2(i16 %94)
  call addrspace(1) void @N$PS(ptr %62)
  %96 = mul i16 %94, 12
  call addrspace(1) void @N$PI2(i16 %96)
  call addrspace(1) void @N$PS(ptr %63)
  call addrspace(1) void @N$PN()
  %97 = add i16 %94, 1
  br label %b31

b34:
  %98 = icmp ne ptr %31, null
  br i1 %98, label %b36, label %b35

b35:
  call addrspace(1) void @N$BDRP(ptr %31)
  ret i16 0

b36:
  %99 = load i16, ptr %47
  br label %b37

b37:
  %100 = phi i16 [ 0, %b36 ], [ %105, %b39 ]
  %101 = icmp ult i16 %100, %99
  br i1 %101, label %b39, label %b35

b39:
  %102 = shl i16 %100, 2
  %103 = getelementptr i8, ptr %31, i16 %102
  %104 = load ptr, ptr %103
  call addrspace(1) void @N$BDRP(ptr %104)
  %105 = add i16 %100, 1
  br label %b37
}

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare void @N$PS(ptr) addrspace(1)

declare void @N$PU2(i16) addrspace(1)

declare void @N$PN() addrspace(1)

declare ptr @N$BGRW(ptr, i16, i16) addrspace(1)

declare void @N$PI2(i16) addrspace(1)

declare void @N$EBND() addrspace(1)

declare void @N$BDRP(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
