target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$str1 = internal constant [9 x i8] c"\08\00\02\00\02\00: \00"
@$str2 = internal constant [12 x i8] c"\08\00\05\00\05\00hello\00"
@$str3 = internal constant [14 x i8] c"\08\00\07\00\07\00, world\00"
@$str4 = internal constant [8 x i8] c"\08\00\01\00\01\00!\00"
@$str5 = internal constant [10 x i8] c"\08\00\03\00\03\00ada\00"
@$str6 = internal constant [8 x i8] c"\08\00\01\00\01\00[\00"
@$str7 = internal constant [13 x i8] c"\08\00\06\00\06\00] has \00"
@$str8 = internal constant [13 x i8] c"\08\00\06\00\06\00 chars\00"
@$str9 = internal constant [10 x i8] c"\08\00\03\00\03\00bob\00"
@$str10 = internal constant [22 x i8] c"\08\00\0F\00\0F\00ada sorts first\00"
@$str11 = internal constant [12 x i8] c"\08\00\05\00\05\00reset\00"

define internal ptr @shout(ptr %0) addrspace(1) {
b1:
  %1 = alloca i8
  %2 = alloca i8
  %3 = alloca i16
  %4 = alloca ptr
  %5 = alloca ptr
  store i8 0, ptr %1
  store i8 0, ptr %2
  store i16 0, ptr %3
  store ptr null, ptr %4
  store ptr null, ptr %5
  store ptr %0, ptr %5, !tbaa !2
  %6 = load ptr, ptr %5, !tbaa !2
  store ptr null, ptr %5, !tbaa !2
  store ptr %6, ptr %4, !tbaa !2
  store i16 0, ptr %3, !tbaa !2
  br label %b2

b2:
  %7 = load i16, ptr %3, !tbaa !2
  %8 = load ptr, ptr %4, !tbaa !2
  %9 = getelementptr i8, ptr %8, i16 -4
  %10 = load i16, ptr %9
  %11 = icmp ult i16 %7, %10
  %12 = sext i1 %11 to i8
  %13 = icmp ne i8 %12, 0
  br i1 %13, label %b3, label %b4

b3:
  %14 = load ptr, ptr %4, !tbaa !2
  %15 = load i16, ptr %3, !tbaa !2
  %16 = getelementptr i8, ptr %14, i16 -4
  %17 = load i16, ptr %16
  %18 = icmp ult i16 %15, %17
  %19 = sext i1 %18 to i8
  %20 = icmp ne i8 %19, 0
  br i1 %20, label %b5, label %b6

b4:
  %21 = load ptr, ptr %4, !tbaa !2
  store ptr null, ptr %4, !tbaa !2
  %22 = load ptr, ptr %4, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %22)
  %23 = load ptr, ptr %5, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %23)
  ret ptr %21

b5:
  %24 = getelementptr i8, ptr %14, i16 %15
  %25 = load i8, ptr %24
  store i8 %25, ptr %2, !tbaa !2
  %26 = load i8, ptr %2, !tbaa !2
  %27 = icmp ule i8 97, %26
  %28 = sext i1 %27 to i8
  store i8 %28, ptr %1, !tbaa !2
  %29 = icmp ne i8 %28, 0
  br i1 %29, label %b8, label %b7

b6:
  call addrspace(1) void @N$EBND()
  unreachable

b7:
  %30 = load i8, ptr %1, !tbaa !2
  %31 = icmp ne i8 %30, 0
  br i1 %31, label %b9, label %b10

b8:
  %32 = icmp ule i8 %26, 122
  %33 = sext i1 %32 to i8
  store i8 %33, ptr %1, !tbaa !2
  br label %b7

b9:
  %34 = load ptr, ptr %4, !tbaa !2
  %35 = call addrspace(1) ptr @N$BRES(ptr %34, i16 0, i16 1)
  store ptr %35, ptr %4, !tbaa !2
  %36 = load i16, ptr %3, !tbaa !2
  %37 = getelementptr i8, ptr %35, i16 -4
  %38 = load i16, ptr %37
  %39 = icmp ult i16 %36, %38
  %40 = sext i1 %39 to i8
  %41 = icmp ne i8 %40, 0
  br i1 %41, label %b12, label %b13

b10:
  br label %b11

b11:
  %42 = load i16, ptr %3, !tbaa !2
  %43 = add i16 %42, 1
  store i16 %43, ptr %3, !tbaa !2
  br label %b2

b12:
  %44 = getelementptr i8, ptr %35, i16 %36
  %45 = load i8, ptr %2, !tbaa !2
  %46 = zext i8 %45 to i16
  %47 = sub i16 %46, 32
  %48 = trunc i16 %47 to i8
  store i8 %48, ptr %44
  br label %b11

b13:
  call addrspace(1) void @N$EBND()
  unreachable
}

define internal ptr @label(ptr %0, i16 %1) addrspace(1) {
b1:
  %2 = alloca ptr
  store ptr null, ptr %2
  store ptr %0, ptr %2, !tbaa !2
  call addrspace(1) void @N$PBEG()
  %3 = load ptr, ptr %2, !tbaa !2
  call addrspace(1) void @N$PS(ptr %3)
  %4 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %4)
  call addrspace(1) void @N$PI2(i16 %1)
  %5 = call addrspace(1) ptr @N$PEND()
  %6 = load ptr, ptr %2, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %6)
  ret ptr %5
}

define internal i16 @main() addrspace(1) {
b1:
  %0 = alloca [8 x i8]
  %1 = alloca [8 x i8]
  %2 = alloca ptr
  %3 = alloca ptr
  %4 = alloca ptr
  %5 = alloca ptr
  call void @llvm.memset.p0.i16(ptr %0, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 8, i1 false)
  store ptr null, ptr %2
  store ptr null, ptr %3
  store ptr null, ptr %4
  store ptr null, ptr %5
  %6 = getelementptr i8, ptr @$str2, i16 6
  store ptr %6, ptr %5, !tbaa !2
  %7 = load ptr, ptr %5, !tbaa !2
  %8 = getelementptr i8, ptr @$str3, i16 6
  %9 = call addrspace(1) ptr @N$TCAT(ptr %7, ptr %8)
  store ptr %9, ptr %4, !tbaa !2
  %10 = getelementptr i8, ptr @$str4, i16 6
  %11 = load ptr, ptr %4, !tbaa !2
  %12 = call addrspace(1) ptr @N$TAPP(ptr %11, ptr %10)
  store ptr %12, ptr %4, !tbaa !2
  %13 = load ptr, ptr %4, !tbaa !2
  call addrspace(1) void @N$PS(ptr %13)
  call addrspace(1) void @N$PN()
  %14 = load ptr, ptr %4, !tbaa !2
  %15 = call addrspace(1) ptr @N$BCLN(ptr %14, i16 1)
  %16 = call addrspace(1) ptr @shout(ptr %15)
  store ptr %16, ptr %3, !tbaa !2
  %17 = load ptr, ptr %3, !tbaa !2
  call addrspace(1) void @N$PS(ptr %17)
  call addrspace(1) void @N$PN()
  %18 = load ptr, ptr %5, !tbaa !2
  call addrspace(1) void @N$PS(ptr %18)
  call addrspace(1) void @N$PN()
  %19 = getelementptr i8, ptr @$str5, i16 6
  %20 = call addrspace(1) ptr @label(ptr %19, i16 42)
  store ptr %20, ptr %2, !tbaa !2
  %21 = getelementptr i8, ptr @$str6, i16 6
  call addrspace(1) void @N$PS(ptr %21)
  %22 = load ptr, ptr %2, !tbaa !2
  call addrspace(1) void @N$PS(ptr %22)
  %23 = getelementptr i8, ptr @$str7, i16 6
  call addrspace(1) void @N$PS(ptr %23)
  %24 = load ptr, ptr %2, !tbaa !2
  %25 = getelementptr i8, ptr %24, i16 -4
  %26 = load i16, ptr %25
  call addrspace(1) void @N$PU2(i16 %26)
  %27 = getelementptr i8, ptr @$str8, i16 6
  call addrspace(1) void @N$PS(ptr %27)
  call addrspace(1) void @N$PN()
  %28 = load ptr, ptr %2, !tbaa !2
  %29 = getelementptr i8, ptr @$str9, i16 6
  %30 = call addrspace(1) ptr @label(ptr %29, i16 1)
  %31 = getelementptr i8, ptr %28, i16 -4
  %32 = load i16, ptr %31
  %33 = addrspacecast ptr %28 to ptr addrspace(1)
  store i16 %32, ptr %1, !tbaa !2
  %34 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 %32, ptr %34, !tbaa !2
  %35 = getelementptr inbounds i8, ptr %1, i16 4
  store ptr addrspace(1) %33, ptr %35, !tbaa !2
  %36 = addrspacecast ptr %1 to ptr addrspace(1)
  %37 = getelementptr i8, ptr %30, i16 -4
  %38 = load i16, ptr %37
  %39 = addrspacecast ptr %30 to ptr addrspace(1)
  store i16 %38, ptr %0, !tbaa !2
  %40 = getelementptr inbounds i8, ptr %0, i16 2
  store i16 %38, ptr %40, !tbaa !2
  %41 = getelementptr inbounds i8, ptr %0, i16 4
  store ptr addrspace(1) %39, ptr %41, !tbaa !2
  %42 = addrspacecast ptr %0 to ptr addrspace(1)
  %43 = call addrspace(1) i8 @N$VCMP(ptr addrspace(1) %36, ptr addrspace(1) %42)
  %44 = icmp slt i8 %43, 0
  %45 = sext i1 %44 to i8
  call addrspace(1) void @N$BDRP(ptr %30)
  %46 = icmp ne i8 %45, 0
  br i1 %46, label %b2, label %b3

b2:
  %47 = getelementptr i8, ptr @$str10, i16 6
  call addrspace(1) void @N$PS(ptr %47)
  call addrspace(1) void @N$PN()
  br label %b4

b3:
  br label %b4

b4:
  %48 = getelementptr i8, ptr @$str11, i16 6
  %49 = load ptr, ptr %4, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %49)
  store ptr %48, ptr %4, !tbaa !2
  %50 = load ptr, ptr %4, !tbaa !2
  call addrspace(1) void @N$PS(ptr %50)
  call addrspace(1) void @N$PN()
  %51 = load ptr, ptr %2, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %51)
  %52 = load ptr, ptr %3, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %52)
  %53 = load ptr, ptr %4, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %53)
  %54 = load ptr, ptr %5, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %54)
  ret i16 0
}

declare void @N$BDRP(ptr) addrspace(1)

declare void @N$EBND() addrspace(1)

declare ptr @N$BRES(ptr, i16, i16) addrspace(1)

declare void @N$PBEG() addrspace(1)

declare void @N$PS(ptr) addrspace(1)

declare void @N$PI2(i16) addrspace(1)

declare ptr @N$PEND() addrspace(1)

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare ptr @N$TCAT(ptr, ptr) addrspace(1)

declare ptr @N$TAPP(ptr, ptr) addrspace(1)

declare void @N$PN() addrspace(1)

declare ptr @N$BCLN(ptr, i16) addrspace(1)

declare void @N$PU2(i16) addrspace(1)

declare i8 @N$VCMP(ptr addrspace(1), ptr addrspace(1)) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
