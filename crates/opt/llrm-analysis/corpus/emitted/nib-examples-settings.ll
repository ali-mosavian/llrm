target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$str1 = internal constant [9 x i8] c"\08\00\02\00\02\00= \00"
@$str2 = internal constant [12 x i8] c"\08\00\05\00\05\00level\00"
@$str3 = internal constant [31 x i8] c"\08\00\18\00\18\00          twice that is \00"
@$str4 = internal constant [34 x i8] c"\08\00\1B\00\1B\00name=Ada;role=pilot;level=7\00"
@$str5 = internal constant [23 x i8] c"\08\00\10\00\10\00 settings, kept \00"

define internal i16 @digits(ptr addrspace(1) noalias readonly dereferenceable(8) %0) addrspace(1) {
b1:
  %1 = alloca i16
  %2 = alloca i16
  store i16 0, ptr %1
  store i16 0, ptr %2
  store i16 0, ptr %2, !tbaa !2
  %3 = load i16, ptr addrspace(1) %0
  store i16 0, ptr %1, !tbaa !2
  br label %b2

b2:
  %4 = load i16, ptr %1, !tbaa !2
  %5 = icmp ult i16 %4, %3
  %6 = sext i1 %5 to i8
  %7 = icmp ne i8 %6, 0
  br i1 %7, label %b3, label %b5

b3:
  %8 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %9 = load ptr addrspace(1), ptr addrspace(1) %8
  %10 = getelementptr i8, ptr addrspace(1) %9, i16 %4
  %11 = load i16, ptr %2, !tbaa !2
  %12 = mul i16 %11, 10
  %13 = load i8, ptr addrspace(1) %10
  %14 = zext i8 %13 to i16
  %15 = add i16 %12, %14
  %16 = zext i8 48 to i16
  %17 = sub i16 %15, %16
  store i16 %17, ptr %2, !tbaa !2
  br label %b4

b4:
  %18 = load i16, ptr %1, !tbaa !2
  %19 = add i16 %18, 1
  store i16 %19, ptr %1, !tbaa !2
  br label %b2

b5:
  %20 = load i16, ptr %2, !tbaa !2
  ret i16 %20
}

define internal void @show(ptr addrspace(1) noalias readonly dereferenceable(8) %0, ptr addrspace(1) noalias readonly dereferenceable(8) %1) addrspace(1) {
b1:
  %2 = alloca i16
  %3 = alloca [8 x i8]
  store i16 0, ptr %2
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 8, i1 false)
  call addrspace(1) void @N$PFLD(i8 8, i8 10, i8 32, i8 1)
  call addrspace(1) void @N$PV(ptr addrspace(1) %0)
  %4 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %4)
  call addrspace(1) void @N$PV(ptr addrspace(1) %1)
  call addrspace(1) void @N$PN()
  %5 = getelementptr i8, ptr @$str2, i16 6
  %6 = getelementptr i8, ptr %5, i16 -4
  %7 = load i16, ptr %6
  %8 = addrspacecast ptr %5 to ptr addrspace(1)
  store i16 %7, ptr %3, !tbaa !2
  %9 = getelementptr inbounds i8, ptr %3, i16 2
  store i16 %7, ptr %9, !tbaa !2
  %10 = getelementptr inbounds i8, ptr %3, i16 4
  store ptr addrspace(1) %8, ptr %10, !tbaa !2
  %11 = addrspacecast ptr %3 to ptr addrspace(1)
  %12 = call addrspace(1) i8 @N$VCMP(ptr addrspace(1) %0, ptr addrspace(1) %11)
  %13 = icmp eq i8 %12, 0
  %14 = sext i1 %13 to i8
  %15 = icmp ne i8 %14, 0
  br i1 %15, label %b2, label %b3

b2:
  %16 = call addrspace(1) i16 @digits(ptr addrspace(1) %1)
  %17 = mul i16 %16, 2
  store i16 %17, ptr %2, !tbaa !2
  %18 = getelementptr i8, ptr @$str3, i16 6
  call addrspace(1) void @N$PS(ptr %18)
  %19 = load i16, ptr %2, !tbaa !2
  call addrspace(1) void @N$PI2(i16 %19)
  call addrspace(1) void @N$PN()
  br label %b4

b3:
  br label %b4

b4:
  ret void
}

define internal ptr @keep(ptr addrspace(1) noalias readonly dereferenceable(8) %0) addrspace(1) {
b1:
  %1 = call addrspace(1) ptr @N$VCPY(ptr addrspace(1) %0)
  ret ptr %1
}

define internal i16 @each_setting(ptr addrspace(1) noalias readonly dereferenceable(8) %0) addrspace(1) {
b1:
  %1 = alloca [8 x i8]
  %2 = alloca [8 x i8]
  %3 = alloca i8
  %4 = alloca i16
  %5 = alloca i8
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca i16
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 8, i1 false)
  store i8 0, ptr %3
  store i16 0, ptr %4
  store i8 0, ptr %5
  store i16 0, ptr %6
  store i16 0, ptr %7
  store i16 0, ptr %8
  store i16 0, ptr %8, !tbaa !2
  store i16 0, ptr %7, !tbaa !2
  store i16 0, ptr %6, !tbaa !2
  br label %b2

b2:
  %9 = load i16, ptr %6, !tbaa !2
  %10 = load i16, ptr addrspace(1) %0
  %11 = icmp ule i16 %9, %10
  %12 = sext i1 %11 to i8
  %13 = icmp ne i8 %12, 0
  br i1 %13, label %b3, label %b4

b3:
  %14 = load i16, ptr %6, !tbaa !2
  %15 = load i16, ptr addrspace(1) %0
  %16 = icmp eq i16 %14, %15
  %17 = sext i1 %16 to i8
  store i8 %17, ptr %5, !tbaa !2
  %18 = icmp ne i8 %17, 0
  br i1 %18, label %b6, label %b5

b4:
  %19 = load i16, ptr %8, !tbaa !2
  ret i16 %19

b5:
  %20 = load i16, ptr %6, !tbaa !2
  %21 = load i16, ptr addrspace(1) %0
  %22 = icmp ult i16 %20, %21
  %23 = sext i1 %22 to i8
  %24 = icmp ne i8 %23, 0
  br i1 %24, label %b7, label %b8

b6:
  %25 = load i8, ptr %5, !tbaa !2
  %26 = icmp ne i8 %25, 0
  br i1 %26, label %b9, label %b10

b7:
  %27 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %28 = load ptr addrspace(1), ptr addrspace(1) %27
  %29 = getelementptr i8, ptr addrspace(1) %28, i16 %20
  %30 = load i8, ptr addrspace(1) %29
  %31 = icmp eq i8 %30, 59
  %32 = sext i1 %31 to i8
  store i8 %32, ptr %5, !tbaa !2
  br label %b6

b8:
  call addrspace(1) void @N$EBND()
  unreachable

b9:
  %33 = load i16, ptr %7, !tbaa !2
  store i16 %33, ptr %4, !tbaa !2
  br label %b12

b10:
  br label %b11

b11:
  %34 = load i16, ptr %6, !tbaa !2
  %35 = add i16 %34, 1
  store i16 %35, ptr %6, !tbaa !2
  br label %b2

b12:
  %36 = load i16, ptr %4, !tbaa !2
  %37 = load i16, ptr %6, !tbaa !2
  %38 = icmp ult i16 %36, %37
  %39 = sext i1 %38 to i8
  store i8 %39, ptr %3, !tbaa !2
  %40 = icmp ne i8 %39, 0
  br i1 %40, label %b15, label %b16

b13:
  %41 = load i16, ptr %4, !tbaa !2
  %42 = add i16 %41, 1
  store i16 %42, ptr %4, !tbaa !2
  br label %b12

b14:
  %43 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %44 = load ptr addrspace(1), ptr addrspace(1) %43
  %45 = load i16, ptr addrspace(1) %0
  %46 = load i16, ptr %7, !tbaa !2
  %47 = load i16, ptr %4, !tbaa !2
  %48 = icmp ule i16 %47, %45
  %49 = sext i1 %48 to i8
  %50 = icmp ne i8 %49, 0
  br i1 %50, label %b19, label %b20

b15:
  %51 = load i16, ptr %4, !tbaa !2
  %52 = load i16, ptr addrspace(1) %0
  %53 = icmp ult i16 %51, %52
  %54 = sext i1 %53 to i8
  %55 = icmp ne i8 %54, 0
  br i1 %55, label %b17, label %b18

b16:
  %56 = load i8, ptr %3, !tbaa !2
  %57 = icmp ne i8 %56, 0
  br i1 %57, label %b13, label %b14

b17:
  %58 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %59 = load ptr addrspace(1), ptr addrspace(1) %58
  %60 = getelementptr i8, ptr addrspace(1) %59, i16 %51
  %61 = load i8, ptr addrspace(1) %60
  %62 = icmp ne i8 %61, 61
  %63 = sext i1 %62 to i8
  store i8 %63, ptr %3, !tbaa !2
  br label %b16

b18:
  call addrspace(1) void @N$EBND()
  unreachable

b19:
  %64 = icmp ule i16 %46, %47
  %65 = sext i1 %64 to i8
  %66 = icmp ne i8 %65, 0
  br i1 %66, label %b21, label %b22

b20:
  call addrspace(1) void @N$EBND()
  unreachable

b21:
  %67 = getelementptr i8, ptr addrspace(1) %44, i16 %46
  %68 = sub i16 %47, %46
  store i16 %68, ptr %2, !tbaa !2
  %69 = getelementptr inbounds i8, ptr %2, i16 2
  store i16 %68, ptr %69, !tbaa !2
  %70 = getelementptr inbounds i8, ptr %2, i16 4
  store ptr addrspace(1) %67, ptr %70, !tbaa !2
  %71 = addrspacecast ptr %2 to ptr addrspace(1)
  %72 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %73 = load ptr addrspace(1), ptr addrspace(1) %72
  %74 = load i16, ptr addrspace(1) %0
  %75 = load i16, ptr %4, !tbaa !2
  %76 = add i16 %75, 1
  %77 = load i16, ptr %6, !tbaa !2
  %78 = icmp ule i16 %77, %74
  %79 = sext i1 %78 to i8
  %80 = icmp ne i8 %79, 0
  br i1 %80, label %b23, label %b24

b22:
  call addrspace(1) void @N$EBND()
  unreachable

b23:
  %81 = icmp ule i16 %76, %77
  %82 = sext i1 %81 to i8
  %83 = icmp ne i8 %82, 0
  br i1 %83, label %b25, label %b26

b24:
  call addrspace(1) void @N$EBND()
  unreachable

b25:
  %84 = getelementptr i8, ptr addrspace(1) %73, i16 %76
  %85 = sub i16 %77, %76
  store i16 %85, ptr %1, !tbaa !2
  %86 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 %85, ptr %86, !tbaa !2
  %87 = getelementptr inbounds i8, ptr %1, i16 4
  store ptr addrspace(1) %84, ptr %87, !tbaa !2
  %88 = addrspacecast ptr %1 to ptr addrspace(1)
  call addrspace(1) void @show(ptr addrspace(1) %71, ptr addrspace(1) %88)
  %89 = load i16, ptr %8, !tbaa !2
  %90 = add i16 %89, 1
  store i16 %90, ptr %8, !tbaa !2
  %91 = load i16, ptr %6, !tbaa !2
  %92 = add i16 %91, 1
  store i16 %92, ptr %7, !tbaa !2
  br label %b11

b26:
  call addrspace(1) void @N$EBND()
  unreachable
}

define internal i16 @main() addrspace(1) {
b1:
  %0 = alloca ptr
  %1 = alloca [8 x i8]
  %2 = alloca i16
  %3 = alloca [8 x i8]
  %4 = alloca ptr
  store ptr null, ptr %0
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 8, i1 false)
  store i16 0, ptr %2
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 8, i1 false)
  store ptr null, ptr %4
  %5 = getelementptr i8, ptr @$str4, i16 6
  store ptr %5, ptr %4, !tbaa !2
  %6 = load ptr, ptr %4, !tbaa !2
  %7 = getelementptr i8, ptr %6, i16 -4
  %8 = load i16, ptr %7
  %9 = addrspacecast ptr %6 to ptr addrspace(1)
  store i16 %8, ptr %3, !tbaa !2
  %10 = getelementptr inbounds i8, ptr %3, i16 2
  store i16 %8, ptr %10, !tbaa !2
  %11 = getelementptr inbounds i8, ptr %3, i16 4
  store ptr addrspace(1) %9, ptr %11, !tbaa !2
  %12 = addrspacecast ptr %3 to ptr addrspace(1)
  %13 = call addrspace(1) i16 @each_setting(ptr addrspace(1) %12)
  store i16 %13, ptr %2, !tbaa !2
  %14 = load ptr, ptr %4, !tbaa !2
  %15 = getelementptr i8, ptr %14, i16 -4
  %16 = load i16, ptr %15
  %17 = addrspacecast ptr %14 to ptr addrspace(1)
  %18 = icmp ule i16 8, %16
  %19 = sext i1 %18 to i8
  %20 = icmp ne i8 %19, 0
  br i1 %20, label %b2, label %b3

b2:
  %21 = getelementptr i8, ptr addrspace(1) %17, i16 5
  store i16 3, ptr %1, !tbaa !2
  %22 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 3, ptr %22, !tbaa !2
  %23 = getelementptr inbounds i8, ptr %1, i16 4
  store ptr addrspace(1) %21, ptr %23, !tbaa !2
  %24 = addrspacecast ptr %1 to ptr addrspace(1)
  %25 = call addrspace(1) ptr @keep(ptr addrspace(1) %24)
  store ptr %25, ptr %0, !tbaa !2
  %26 = load i16, ptr %2, !tbaa !2
  call addrspace(1) void @N$PU2(i16 %26)
  %27 = getelementptr i8, ptr @$str5, i16 6
  call addrspace(1) void @N$PS(ptr %27)
  %28 = load ptr, ptr %0, !tbaa !2
  call addrspace(1) void @N$PS(ptr %28)
  call addrspace(1) void @N$PN()
  %29 = load ptr, ptr %0, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %29)
  %30 = load ptr, ptr %4, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %30)
  ret i16 0

b3:
  call addrspace(1) void @N$EBND()
  unreachable
}

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare void @N$PFLD(i8, i8, i8, i8) addrspace(1)

declare void @N$PV(ptr addrspace(1)) addrspace(1)

declare void @N$PS(ptr) addrspace(1)

declare void @N$PN() addrspace(1)

declare i8 @N$VCMP(ptr addrspace(1), ptr addrspace(1)) addrspace(1)

declare void @N$PI2(i16) addrspace(1)

declare ptr @N$VCPY(ptr addrspace(1)) addrspace(1)

declare void @N$EBND() addrspace(1)

declare void @N$PU2(i16) addrspace(1)

declare void @N$BDRP(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
