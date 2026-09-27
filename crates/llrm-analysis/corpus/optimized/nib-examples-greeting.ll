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
  br label %b2

b2:
  %1 = phi ptr [ %0, %b1 ], [ %18, %b11 ]
  %2 = phi i16 [ 0, %b1 ], [ %19, %b11 ]
  %3 = getelementptr i8, ptr %1, i16 -4
  %4 = load i16, ptr %3
  %5 = icmp ult i16 %2, %4
  br i1 %5, label %b3, label %b4

b3:
  %6 = getelementptr i8, ptr %1, i16 %2
  %7 = load i8, ptr %6
  %8 = icmp uge i8 %7, 97
  %9 = sext i1 %8 to i8
  br i1 %8, label %b8, label %b7

b4:
  call addrspace(1) void @N$BDRP(ptr null)
  call addrspace(1) void @N$BDRP(ptr null)
  ret ptr %1

b7:
  %10 = phi i8 [ %9, %b3 ], [ %13, %b8 ]
  %11 = icmp ne i8 %10, 0
  br i1 %11, label %b9, label %b11

b8:
  %12 = icmp ule i8 %7, 122
  %13 = sext i1 %12 to i8
  br label %b7

b9:
  %14 = call addrspace(1) ptr @N$BRES(ptr %1, i16 0, i16 1)
  %15 = getelementptr i8, ptr %14, i16 -4
  %16 = load i16, ptr %15
  %17 = icmp ult i16 %2, %16
  br i1 %17, label %b12, label %b13

b11:
  %18 = phi ptr [ %1, %b7 ], [ %14, %b12 ]
  %19 = add i16 %2, 1
  br label %b2

b12:
  %20 = getelementptr i8, ptr %14, i16 %2
  %21 = zext i8 %7 to i16
  %22 = add i16 %21, -32
  %23 = trunc i16 %22 to i8
  store i8 %23, ptr %20
  br label %b11

b13:
  call addrspace(1) void @N$EBND()
  unreachable
}

define internal ptr @label(ptr %0, i16 %1) addrspace(1) {
b1:
  call addrspace(1) void @N$PBEG()
  call addrspace(1) void @N$PS(ptr %0)
  %2 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %2)
  call addrspace(1) void @N$PI2(i16 %1)
  %3 = call addrspace(1) ptr @N$PEND()
  call addrspace(1) void @N$BDRP(ptr %0)
  ret ptr %3
}

define internal i16 @main() addrspace(1) {
b1:
  %0 = alloca [8 x i8]
  %1 = alloca [8 x i8]
  call void @llvm.memset.p0.i16(ptr %0, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 8, i1 false)
  %2 = getelementptr i8, ptr @$str2, i16 6
  %3 = getelementptr i8, ptr @$str3, i16 6
  %4 = call addrspace(1) ptr @N$TCAT(ptr %2, ptr %3)
  %5 = getelementptr i8, ptr @$str4, i16 6
  %6 = call addrspace(1) ptr @N$TAPP(ptr %4, ptr %5)
  call addrspace(1) void @N$PS(ptr %6)
  call addrspace(1) void @N$PN()
  %7 = call addrspace(1) ptr @N$BCLN(ptr %6, i16 1)
  br label %8

8:
  %9 = phi ptr [ %7, %b1 ], [ %55, %54 ]
  %10 = phi i16 [ 0, %b1 ], [ %56, %54 ]
  %11 = getelementptr i8, ptr %9, i16 -4
  %12 = load i16, ptr %11
  %13 = icmp ult i16 %10, %12
  br i1 %13, label %14, label %19

14:
  %15 = getelementptr i8, ptr %9, i16 %10
  %16 = load i8, ptr %15
  %17 = icmp uge i8 %16, 97
  %18 = sext i1 %17 to i8
  br i1 %17, label %46, label %43

19:
  call addrspace(1) void @N$BDRP(ptr null)
  call addrspace(1) void @N$BDRP(ptr null)
  call addrspace(1) void @N$PS(ptr %9)
  call addrspace(1) void @N$PN()
  call addrspace(1) void @N$PS(ptr %2)
  call addrspace(1) void @N$PN()
  %20 = getelementptr i8, ptr @$str5, i16 6
  call addrspace(1) void @N$PBEG()
  call addrspace(1) void @N$PS(ptr %20)
  %21 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %21)
  call addrspace(1) void @N$PI2(i16 42)
  %22 = call addrspace(1) ptr @N$PEND()
  call addrspace(1) void @N$BDRP(ptr %20)
  %23 = getelementptr i8, ptr @$str6, i16 6
  call addrspace(1) void @N$PS(ptr %23)
  call addrspace(1) void @N$PS(ptr %22)
  %24 = getelementptr i8, ptr @$str7, i16 6
  call addrspace(1) void @N$PS(ptr %24)
  %25 = getelementptr i8, ptr %22, i16 -4
  %26 = load i16, ptr %25
  call addrspace(1) void @N$PU2(i16 %26)
  %27 = getelementptr i8, ptr @$str8, i16 6
  call addrspace(1) void @N$PS(ptr %27)
  call addrspace(1) void @N$PN()
  %28 = getelementptr i8, ptr @$str9, i16 6
  call addrspace(1) void @N$PBEG()
  call addrspace(1) void @N$PS(ptr %28)
  call addrspace(1) void @N$PS(ptr %21)
  call addrspace(1) void @N$PI2(i16 1)
  %29 = call addrspace(1) ptr @N$PEND()
  call addrspace(1) void @N$BDRP(ptr %28)
  %30 = load i16, ptr %25
  %31 = addrspacecast ptr %22 to ptr addrspace(1)
  store i16 %30, ptr %1, !tbaa !2
  %32 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 %30, ptr %32, !tbaa !2
  %33 = getelementptr inbounds i8, ptr %1, i16 4
  store ptr addrspace(1) %31, ptr %33, !tbaa !2
  %34 = addrspacecast ptr %1 to ptr addrspace(1)
  %35 = getelementptr i8, ptr %29, i16 -4
  %36 = load i16, ptr %35
  %37 = addrspacecast ptr %29 to ptr addrspace(1)
  store i16 %36, ptr %0, !tbaa !2
  %38 = getelementptr inbounds i8, ptr %0, i16 2
  store i16 %36, ptr %38, !tbaa !2
  %39 = getelementptr inbounds i8, ptr %0, i16 4
  store ptr addrspace(1) %37, ptr %39, !tbaa !2
  %40 = addrspacecast ptr %0 to ptr addrspace(1)
  %41 = call addrspace(1) i8 @N$VCMP(ptr addrspace(1) %34, ptr addrspace(1) %40)
  %42 = icmp slt i8 %41, 0
  call addrspace(1) void @N$BDRP(ptr %29)
  br i1 %42, label %b2, label %b4

43:
  %44 = phi i8 [ %18, %14 ], [ %48, %46 ]
  %45 = icmp ne i8 %44, 0
  br i1 %45, label %49, label %54

46:
  %47 = icmp ule i8 %16, 122
  %48 = sext i1 %47 to i8
  br label %43

49:
  %50 = call addrspace(1) ptr @N$BRES(ptr %9, i16 0, i16 1)
  %51 = getelementptr i8, ptr %50, i16 -4
  %52 = load i16, ptr %51
  %53 = icmp ult i16 %10, %52
  br i1 %53, label %57, label %62

54:
  %55 = phi ptr [ %9, %43 ], [ %50, %57 ]
  %56 = add i16 %10, 1
  br label %8

57:
  %58 = getelementptr i8, ptr %50, i16 %10
  %59 = zext i8 %16 to i16
  %60 = add i16 %59, -32
  %61 = trunc i16 %60 to i8
  store i8 %61, ptr %58
  br label %54

62:
  call addrspace(1) void @N$EBND()
  unreachable

b2:
  %63 = getelementptr i8, ptr @$str10, i16 6
  call addrspace(1) void @N$PS(ptr %63)
  call addrspace(1) void @N$PN()
  br label %b4

b4:
  %64 = getelementptr i8, ptr @$str11, i16 6
  call addrspace(1) void @N$BDRP(ptr %6)
  call addrspace(1) void @N$PS(ptr %64)
  call addrspace(1) void @N$PN()
  call addrspace(1) void @N$BDRP(ptr %22)
  call addrspace(1) void @N$BDRP(ptr %9)
  call addrspace(1) void @N$BDRP(ptr %64)
  call addrspace(1) void @N$BDRP(ptr %2)
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
