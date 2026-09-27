target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$str1 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"
@$str2 = internal constant [20 x i8] c"\08\00\0D\00\0D\00Ada, pilot, 7\00"
@$str3 = internal constant [26 x i8] c"\08\00\13\00\13\00Grace,  admiral , 9\00"
@$str4 = internal constant [19 x i8] c"\08\00\0C\00\0C\00Linus ,  , 3\00"
@$str5 = internal constant [18 x i8] c"\08\00\0B\00\0B\00| (no role)\00"
@$str6 = internal constant [9 x i8] c"\08\00\02\00\02\00| \00"
@$str7 = internal constant [17 x i8] c"\08\00\0A\00\0A\00 at level \00"
@$str8 = internal constant [12 x i8] c"\08\00\05\00\05\00Grace\00"
@$str9 = internal constant [8 x i8] c"\08\00\01\00\01\00 \00"

define internal void @trimmed(ptr addrspace(1) %0, ptr addrspace(1) noalias readonly dereferenceable(8) %1) addrspace(1) {
b1:
  %2 = load i16, ptr addrspace(1) %1
  %3 = getelementptr i8, ptr addrspace(1) %1, i16 4
  %4 = load ptr addrspace(1), ptr addrspace(1) %3
  br label %b2

b2:
  %5 = phi i16 [ 0, %b1 ], [ %8, %b3 ]
  %6 = icmp ult i16 %5, %2
  %7 = sext i1 %6 to i8
  br i1 %6, label %b5, label %b6

b3:
  %8 = add i16 %5, 1
  br label %b2

b4:
  br label %b9

b5:
  %9 = getelementptr i8, ptr addrspace(1) %4, i16 %5
  %10 = load i8, ptr addrspace(1) %9
  %11 = icmp eq i8 %10, 32
  %12 = sext i1 %11 to i8
  br label %b6

b6:
  %13 = phi i8 [ %7, %b2 ], [ %12, %b5 ]
  %14 = icmp ne i8 %13, 0
  br i1 %14, label %b3, label %b4

b9:
  %15 = phi i16 [ %2, %b4 ], [ %18, %b10 ]
  %16 = icmp ugt i16 %15, %5
  %17 = sext i1 %16 to i8
  br i1 %16, label %b12, label %b13

b10:
  %18 = add i16 %15, -1
  br label %b9

b11:
  %19 = icmp ule i16 %15, %2
  br i1 %19, label %b16, label %b17

b12:
  %20 = add i16 %15, -1
  %21 = icmp ult i16 %20, %2
  br i1 %21, label %b14, label %b15

b13:
  %22 = phi i8 [ %17, %b9 ], [ %27, %b14 ]
  %23 = icmp ne i8 %22, 0
  br i1 %23, label %b10, label %b11

b14:
  %24 = getelementptr i8, ptr addrspace(1) %4, i16 %20
  %25 = load i8, ptr addrspace(1) %24
  %26 = icmp eq i8 %25, 32
  %27 = sext i1 %26 to i8
  br label %b13

b15:
  call addrspace(1) void @N$EBND()
  unreachable

b16:
  %28 = icmp ule i16 %5, %15
  br i1 %28, label %b18, label %b19

b17:
  call addrspace(1) void @N$EBND()
  unreachable

b18:
  %29 = getelementptr i8, ptr addrspace(1) %4, i16 %5
  %30 = sub i16 %15, %5
  store i16 %30, ptr addrspace(1) %0
  %31 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %30, ptr addrspace(1) %31
  %32 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store ptr addrspace(1) %29, ptr addrspace(1) %32
  ret void

b19:
  call addrspace(1) void @N$EBND()
  unreachable
}

define internal void @field(ptr addrspace(1) %0, ptr addrspace(1) noalias readonly dereferenceable(8) %1, i16 %2) addrspace(1) {
b1:
  %3 = alloca [8 x i8]
  %4 = alloca [8 x i8]
  %5 = alloca [8 x i8]
  %6 = alloca [8 x i8]
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %4, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %5, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %6, i8 0, i16 8, i1 false)
  %7 = load i16, ptr addrspace(1) %1
  %8 = getelementptr i8, ptr addrspace(1) %1, i16 4
  %9 = load ptr addrspace(1), ptr addrspace(1) %8
  br label %b2

b2:
  %10 = phi i16 [ 0, %b1 ], [ %19, %b10 ]
  %11 = phi i16 [ 0, %b1 ], [ %20, %b10 ]
  %12 = phi i16 [ 0, %b1 ], [ %21, %b10 ]
  %13 = icmp ult i16 %12, %7
  br i1 %13, label %b3, label %b5

b3:
  %14 = getelementptr i8, ptr addrspace(1) %9, i16 %12
  %15 = load i8, ptr addrspace(1) %14
  %16 = icmp eq i8 %15, 44
  br i1 %16, label %b8, label %b10

b5:
  %17 = icmp eq i16 %10, %2
  br i1 %17, label %b18, label %b20

b8:
  %18 = icmp eq i16 %10, %2
  br i1 %18, label %b11, label %b12

b10:
  %19 = phi i16 [ %10, %b3 ], [ %24, %b12 ]
  %20 = phi i16 [ %11, %b3 ], [ %25, %b12 ]
  %21 = add i16 %12, 1
  br label %b2

b11:
  %22 = addrspacecast ptr %6 to ptr addrspace(1)
  %23 = icmp ule i16 %12, %7
  br i1 %23, label %b14, label %b15

b12:
  %24 = add i16 %10, 1
  %25 = add i16 %12, 1
  br label %b10

b14:
  %26 = icmp ule i16 %11, %12
  br i1 %26, label %b16, label %b17

b15:
  call addrspace(1) void @N$EBND()
  unreachable

b16:
  %27 = getelementptr i8, ptr addrspace(1) %9, i16 %11
  %28 = sub i16 %12, %11
  store i16 %28, ptr %5, !tbaa !2
  %29 = getelementptr inbounds i8, ptr %5, i16 2
  store i16 %28, ptr %29, !tbaa !2
  %30 = getelementptr inbounds i8, ptr %5, i16 4
  store ptr addrspace(1) %27, ptr %30, !tbaa !2
  %31 = addrspacecast ptr %5 to ptr addrspace(1)
  call addrspace(1) void @trimmed(ptr addrspace(1) %22, ptr addrspace(1) %31)
  %32 = load i16, ptr addrspace(1) %22, !tbaa !2
  store i16 %32, ptr addrspace(1) %0
  %33 = getelementptr i8, ptr addrspace(1) %22, i16 2
  %34 = load i16, ptr addrspace(1) %33, !tbaa !2
  %35 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %34, ptr addrspace(1) %35
  %36 = getelementptr i8, ptr addrspace(1) %22, i16 4
  %37 = load ptr addrspace(1), ptr addrspace(1) %36, !tbaa !2
  %38 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store ptr addrspace(1) %37, ptr addrspace(1) %38
  ret void

b17:
  call addrspace(1) void @N$EBND()
  unreachable

b18:
  %39 = addrspacecast ptr %4 to ptr addrspace(1)
  %40 = icmp ule i16 %11, %7
  br i1 %40, label %b23, label %b24

b20:
  store i16 0, ptr addrspace(1) %0
  %41 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 0, ptr addrspace(1) %41
  %42 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store ptr addrspace(1) %9, ptr addrspace(1) %42
  ret void

b23:
  %43 = getelementptr i8, ptr addrspace(1) %9, i16 %11
  %44 = sub i16 %7, %11
  store i16 %44, ptr %3, !tbaa !2
  %45 = getelementptr inbounds i8, ptr %3, i16 2
  store i16 %44, ptr %45, !tbaa !2
  %46 = getelementptr inbounds i8, ptr %3, i16 4
  store ptr addrspace(1) %43, ptr %46, !tbaa !2
  %47 = addrspacecast ptr %3 to ptr addrspace(1)
  call addrspace(1) void @trimmed(ptr addrspace(1) %39, ptr addrspace(1) %47)
  %48 = load i16, ptr addrspace(1) %39, !tbaa !2
  store i16 %48, ptr addrspace(1) %0
  %49 = getelementptr i8, ptr addrspace(1) %39, i16 2
  %50 = load i16, ptr addrspace(1) %49, !tbaa !2
  %51 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %50, ptr addrspace(1) %51
  %52 = getelementptr i8, ptr addrspace(1) %39, i16 4
  %53 = load ptr addrspace(1), ptr addrspace(1) %52, !tbaa !2
  %54 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store ptr addrspace(1) %53, ptr addrspace(1) %54
  ret void

b24:
  call addrspace(1) void @N$EBND()
  unreachable
}

define internal i16 @main() addrspace(1) {
b1:
  %0 = alloca [8 x i8]
  %1 = alloca [8 x i8]
  %2 = alloca [8 x i8]
  %3 = alloca [8 x i8]
  %4 = alloca [8 x i8]
  %5 = alloca [8 x i8]
  %6 = alloca [8 x i8]
  %7 = alloca [8 x i8]
  %8 = alloca [8 x i8]
  call void @llvm.memset.p0.i16(ptr %0, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %4, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %5, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %6, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %7, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %8, i8 0, i16 8, i1 false)
  %9 = getelementptr i8, ptr @$str1, i16 6
  %10 = call addrspace(1) ptr @N$BGRW(ptr %9, i16 3, i16 2)
  %11 = getelementptr i8, ptr %10, i16 0
  %12 = getelementptr i8, ptr @$str2, i16 6
  store ptr %12, ptr %11
  %13 = getelementptr i8, ptr %10, i16 2
  %14 = getelementptr i8, ptr @$str3, i16 6
  store ptr %14, ptr %13
  %15 = getelementptr i8, ptr %10, i16 4
  %16 = getelementptr i8, ptr @$str4, i16 6
  store ptr %16, ptr %15
  %17 = getelementptr i8, ptr %10, i16 -4
  %18 = load i16, ptr %17
  %19 = addrspacecast ptr %8 to ptr addrspace(1)
  %20 = getelementptr inbounds i8, ptr %7, i16 2
  %21 = getelementptr inbounds i8, ptr %7, i16 4
  %22 = addrspacecast ptr %7 to ptr addrspace(1)
  %23 = addrspacecast ptr %6 to ptr addrspace(1)
  %24 = getelementptr inbounds i8, ptr %5, i16 2
  %25 = getelementptr inbounds i8, ptr %5, i16 4
  %26 = addrspacecast ptr %5 to ptr addrspace(1)
  %27 = addrspacecast ptr %4 to ptr addrspace(1)
  %28 = getelementptr inbounds i8, ptr %3, i16 2
  %29 = getelementptr inbounds i8, ptr %3, i16 4
  %30 = addrspacecast ptr %3 to ptr addrspace(1)
  %31 = getelementptr i8, ptr @$str6, i16 6
  %32 = getelementptr i8, ptr @$str7, i16 6
  %33 = getelementptr i8, ptr @$str5, i16 6
  br label %b2

b2:
  %34 = phi i16 [ 0, %b1 ], [ %55, %b8 ]
  %35 = icmp ult i16 %34, %18
  br i1 %35, label %b3, label %b5

b3:
  %36 = shl i16 %34, 1
  %37 = getelementptr i8, ptr %10, i16 %36
  %38 = load ptr, ptr %37
  %39 = getelementptr i8, ptr %38, i16 -4
  %40 = load i16, ptr %39
  %41 = addrspacecast ptr %38 to ptr addrspace(1)
  store i16 %40, ptr %7, !tbaa !2
  store i16 %40, ptr %20, !tbaa !2
  store ptr addrspace(1) %41, ptr %21, !tbaa !2
  call addrspace(1) void @field(ptr addrspace(1) %19, ptr addrspace(1) %22, i16 0)
  %42 = load ptr, ptr %37
  %43 = getelementptr i8, ptr %42, i16 -4
  %44 = load i16, ptr %43
  %45 = addrspacecast ptr %42 to ptr addrspace(1)
  store i16 %44, ptr %5, !tbaa !2
  store i16 %44, ptr %24, !tbaa !2
  store ptr addrspace(1) %45, ptr %25, !tbaa !2
  call addrspace(1) void @field(ptr addrspace(1) %23, ptr addrspace(1) %26, i16 1)
  %46 = load i16, ptr addrspace(1) %23
  %47 = icmp eq i16 %46, 0
  br i1 %47, label %b6, label %b7

b5:
  %48 = addrspacecast ptr %2 to ptr addrspace(1)
  %49 = load i16, ptr %17
  %50 = icmp ugt i16 %49, 1
  br i1 %50, label %b9, label %b10

b6:
  call addrspace(1) void @N$PFLD(i8 6, i8 10, i8 32, i8 1)
  call addrspace(1) void @N$PV(ptr addrspace(1) %19)
  call addrspace(1) void @N$PS(ptr %33)
  call addrspace(1) void @N$PN()
  br label %b8

b7:
  %51 = load ptr, ptr %37
  %52 = getelementptr i8, ptr %51, i16 -4
  %53 = load i16, ptr %52
  %54 = addrspacecast ptr %51 to ptr addrspace(1)
  store i16 %53, ptr %3, !tbaa !2
  store i16 %53, ptr %28, !tbaa !2
  store ptr addrspace(1) %54, ptr %29, !tbaa !2
  call addrspace(1) void @field(ptr addrspace(1) %27, ptr addrspace(1) %30, i16 2)
  call addrspace(1) void @N$PFLD(i8 6, i8 10, i8 32, i8 1)
  call addrspace(1) void @N$PV(ptr addrspace(1) %19)
  call addrspace(1) void @N$PS(ptr %31)
  call addrspace(1) void @N$PV(ptr addrspace(1) %23)
  call addrspace(1) void @N$PS(ptr %32)
  call addrspace(1) void @N$PV(ptr addrspace(1) %27)
  call addrspace(1) void @N$PN()
  br label %b8

b8:
  %55 = add i16 %34, 1
  br label %b2

b9:
  %56 = load ptr, ptr %13
  %57 = getelementptr i8, ptr %56, i16 -4
  %58 = load i16, ptr %57
  %59 = addrspacecast ptr %56 to ptr addrspace(1)
  store i16 %58, ptr %1, !tbaa !2
  %60 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 %58, ptr %60, !tbaa !2
  %61 = getelementptr inbounds i8, ptr %1, i16 4
  store ptr addrspace(1) %59, ptr %61, !tbaa !2
  %62 = addrspacecast ptr %1 to ptr addrspace(1)
  call addrspace(1) void @field(ptr addrspace(1) %48, ptr addrspace(1) %62, i16 0)
  %63 = getelementptr i8, ptr @$str8, i16 6
  %64 = getelementptr i8, ptr %63, i16 -4
  %65 = load i16, ptr %64
  %66 = addrspacecast ptr %63 to ptr addrspace(1)
  store i16 %65, ptr %0, !tbaa !2
  %67 = getelementptr inbounds i8, ptr %0, i16 2
  store i16 %65, ptr %67, !tbaa !2
  %68 = getelementptr inbounds i8, ptr %0, i16 4
  store ptr addrspace(1) %66, ptr %68, !tbaa !2
  %69 = addrspacecast ptr %0 to ptr addrspace(1)
  %70 = call addrspace(1) i8 @N$VCMP(ptr addrspace(1) %48, ptr addrspace(1) %69)
  %71 = icmp eq i8 %70, 0
  %72 = sext i1 %71 to i8
  %73 = call addrspace(1) ptr @N$VCPY(ptr addrspace(1) %48)
  call addrspace(1) void @N$PB(i8 %72)
  %74 = getelementptr i8, ptr @$str9, i16 6
  call addrspace(1) void @N$PS(ptr %74)
  call addrspace(1) void @N$PS(ptr %73)
  call addrspace(1) void @N$PN()
  call addrspace(1) void @N$BDRP(ptr %73)
  %75 = icmp ne ptr %10, null
  br i1 %75, label %b12, label %b11

b10:
  call addrspace(1) void @N$EBND()
  unreachable

b11:
  call addrspace(1) void @N$BDRP(ptr %10)
  ret i16 0

b12:
  %76 = load i16, ptr %17
  br label %b13

b13:
  %77 = phi i16 [ 0, %b12 ], [ %82, %b15 ]
  %78 = icmp ult i16 %77, %76
  br i1 %78, label %b15, label %b11

b15:
  %79 = shl i16 %77, 1
  %80 = getelementptr i8, ptr %10, i16 %79
  %81 = load ptr, ptr %80
  call addrspace(1) void @N$BDRP(ptr %81)
  %82 = add i16 %77, 1
  br label %b13
}

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare void @N$EBND() addrspace(1)

declare ptr @N$BGRW(ptr, i16, i16) addrspace(1)

declare void @N$PFLD(i8, i8, i8, i8) addrspace(1)

declare void @N$PV(ptr addrspace(1)) addrspace(1)

declare void @N$PS(ptr) addrspace(1)

declare void @N$PN() addrspace(1)

declare i8 @N$VCMP(ptr addrspace(1), ptr addrspace(1)) addrspace(1)

declare ptr @N$VCPY(ptr addrspace(1)) addrspace(1)

declare void @N$PB(i8) addrspace(1)

declare void @N$BDRP(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
