target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$str1 = internal constant [12 x i8] c"\08\00\05\00\05\00empty\00"
@$str2 = internal constant [11 x i8] c"\08\00\04\00\04\00one \00"
@$str3 = internal constant [9 x i8] c"\08\00\02\00\02\00..\00"
@$str4 = internal constant [15 x i8] c"\08\00\08\00\08\00 around \00"
@$str5 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"
@$str6 = internal constant [8 x i8] c"\08\00\01\00\01\00 \00"
@$str7 = internal constant [16 x i8] c"\08\00\09\00\09\00first at \00"
@$str8 = internal constant [8 x i8] c"\08\00\01\00\01\00,\00"
@$str9 = internal constant [16 x i8] c"\08\00\09\00\09\00no points\00"
@$str10 = internal constant [24 x i8] c"\08\00\11\00\11\00starts 1, 2 then \00"
@$str11 = internal constant [12 x i8] c"\08\00\05\00\05\00other\00"

define internal i16 @describe(ptr addrspace(1) noalias readonly dereferenceable(8) %0) addrspace(1) {
b1:
  %1 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %2 = load ptr addrspace(1), ptr addrspace(1) %1
  %3 = load i16, ptr addrspace(1) %0
  %4 = icmp eq i16 %3, 0
  br i1 %4, label %b4, label %b3

b2:
  ret i16 0

b3:
  %5 = icmp eq i16 %3, 1
  br i1 %5, label %b6, label %b5

b4:
  %6 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %6)
  call addrspace(1) void @N$PN()
  br label %b2

b5:
  %7 = getelementptr i8, ptr addrspace(1) %2, i16 0
  %8 = add i16 %3, -1
  %9 = shl i16 %8, 1
  %10 = getelementptr i8, ptr addrspace(1) %2, i16 %9
  %11 = add i16 %3, -2
  %12 = load i16, ptr addrspace(1) %7
  call addrspace(1) void @N$PI2(i16 %12)
  %13 = getelementptr i8, ptr @$str3, i16 6
  call addrspace(1) void @N$PS(ptr %13)
  %14 = load i16, ptr addrspace(1) %10
  call addrspace(1) void @N$PI2(i16 %14)
  %15 = getelementptr i8, ptr @$str4, i16 6
  call addrspace(1) void @N$PS(ptr %15)
  call addrspace(1) void @N$PU2(i16 %11)
  call addrspace(1) void @N$PN()
  br label %b2

b6:
  %16 = getelementptr i8, ptr addrspace(1) %2, i16 0
  %17 = getelementptr i8, ptr @$str2, i16 6
  call addrspace(1) void @N$PS(ptr %17)
  %18 = load i16, ptr addrspace(1) %16
  call addrspace(1) void @N$PI2(i16 %18)
  call addrspace(1) void @N$PN()
  br label %b2
}

define internal i16 @head(ptr addrspace(1) noalias readonly dereferenceable(8) %0) addrspace(1) willreturn {
b1:
  %1 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %2 = load ptr addrspace(1), ptr addrspace(1) %1
  %3 = load i16, ptr addrspace(1) %0
  %4 = icmp sge i16 %3, 1
  br i1 %4, label %b4, label %b2

b2:
  ret i16 -1

b4:
  %5 = getelementptr i8, ptr addrspace(1) %2, i16 0
  %6 = add i16 %3, -1
  %7 = load i16, ptr addrspace(1) %5
  %8 = add i16 %7, %6
  ret i16 %8
}

define internal i16 @main() addrspace(1) {
b1:
  %0 = alloca [2 x i8]
  call void @llvm.memset.p0.i16(ptr %0, i8 0, i16 2, i1 false)
  %1 = getelementptr i8, ptr @$str5, i16 6
  %2 = getelementptr inbounds i16, ptr %0, i16 0
  store i16 7, ptr %2, !tbaa !2
  %3 = call addrspace(1) ptr @N$BGRW(ptr %1, i16 4, i16 2)
  %4 = getelementptr i8, ptr %3, i16 0
  store i16 1, ptr %4
  %5 = getelementptr i8, ptr %3, i16 2
  store i16 2, ptr %5
  %6 = getelementptr i8, ptr %3, i16 4
  store i16 3, ptr %6
  %7 = getelementptr i8, ptr %3, i16 6
  store i16 4, ptr %7
  %8 = getelementptr i8, ptr %1, i16 -4
  %9 = load i16, ptr %8
  %10 = addrspacecast ptr %1 to ptr addrspace(1)
  %11 = icmp eq i16 %9, 0
  br i1 %11, label %14, label %12

12:
  %13 = icmp eq i16 %9, 1
  br i1 %13, label %26, label %16

14:
  %15 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %15)
  call addrspace(1) void @N$PN()
  br label %30

16:
  %17 = getelementptr i8, ptr addrspace(1) %10, i16 0
  %18 = add i16 %9, -1
  %19 = shl i16 %18, 1
  %20 = getelementptr i8, ptr addrspace(1) %10, i16 %19
  %21 = add i16 %9, -2
  %22 = load i16, ptr addrspace(1) %17
  call addrspace(1) void @N$PI2(i16 %22)
  %23 = getelementptr i8, ptr @$str3, i16 6
  call addrspace(1) void @N$PS(ptr %23)
  %24 = load i16, ptr addrspace(1) %20
  call addrspace(1) void @N$PI2(i16 %24)
  %25 = getelementptr i8, ptr @$str4, i16 6
  call addrspace(1) void @N$PS(ptr %25)
  call addrspace(1) void @N$PU2(i16 %21)
  call addrspace(1) void @N$PN()
  br label %30

26:
  %27 = getelementptr i8, ptr addrspace(1) %10, i16 0
  %28 = getelementptr i8, ptr @$str2, i16 6
  call addrspace(1) void @N$PS(ptr %28)
  %29 = load i16, ptr addrspace(1) %27
  call addrspace(1) void @N$PI2(i16 %29)
  call addrspace(1) void @N$PN()
  br label %30

30:
  %31 = addrspacecast ptr %0 to ptr addrspace(1)
  %32 = getelementptr i8, ptr addrspace(1) %31, i16 0
  %33 = getelementptr i8, ptr @$str2, i16 6
  call addrspace(1) void @N$PS(ptr %33)
  %34 = load i16, ptr addrspace(1) %32
  call addrspace(1) void @N$PI2(i16 %34)
  call addrspace(1) void @N$PN()
  %35 = getelementptr i8, ptr %3, i16 -4
  %36 = load i16, ptr %35
  %37 = addrspacecast ptr %3 to ptr addrspace(1)
  %38 = icmp eq i16 %36, 0
  br i1 %38, label %41, label %39

39:
  %40 = icmp eq i16 %36, 1
  br i1 %40, label %53, label %43

41:
  %42 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %42)
  call addrspace(1) void @N$PN()
  br label %56

43:
  %44 = getelementptr i8, ptr addrspace(1) %37, i16 0
  %45 = add i16 %36, -1
  %46 = shl i16 %45, 1
  %47 = getelementptr i8, ptr addrspace(1) %37, i16 %46
  %48 = add i16 %36, -2
  %49 = load i16, ptr addrspace(1) %44
  call addrspace(1) void @N$PI2(i16 %49)
  %50 = getelementptr i8, ptr @$str3, i16 6
  call addrspace(1) void @N$PS(ptr %50)
  %51 = load i16, ptr addrspace(1) %47
  call addrspace(1) void @N$PI2(i16 %51)
  %52 = getelementptr i8, ptr @$str4, i16 6
  call addrspace(1) void @N$PS(ptr %52)
  call addrspace(1) void @N$PU2(i16 %48)
  call addrspace(1) void @N$PN()
  br label %56

53:
  %54 = getelementptr i8, ptr addrspace(1) %37, i16 0
  call addrspace(1) void @N$PS(ptr %33)
  %55 = load i16, ptr addrspace(1) %54
  call addrspace(1) void @N$PI2(i16 %55)
  call addrspace(1) void @N$PN()
  br label %56

56:
  %57 = load i16, ptr %35
  %58 = icmp sge i16 %57, 1
  br i1 %58, label %59, label %64

59:
  %60 = getelementptr i8, ptr addrspace(1) %37, i16 0
  %61 = add i16 %57, -1
  %62 = load i16, ptr addrspace(1) %60
  %63 = add i16 %62, %61
  br label %64

64:
  %65 = phi i16 [ -1, %56 ], [ %63, %59 ]
  %66 = load i16, ptr %8
  %67 = icmp sge i16 %66, 1
  br i1 %67, label %68, label %73

68:
  %69 = getelementptr i8, ptr addrspace(1) %10, i16 0
  %70 = add i16 %66, -1
  %71 = load i16, ptr addrspace(1) %69
  %72 = add i16 %71, %70
  br label %73

73:
  %74 = phi i16 [ -1, %64 ], [ %72, %68 ]
  call addrspace(1) void @N$PI2(i16 %65)
  %75 = getelementptr i8, ptr @$str6, i16 6
  call addrspace(1) void @N$PS(ptr %75)
  call addrspace(1) void @N$PI2(i16 %74)
  call addrspace(1) void @N$PN()
  %76 = call addrspace(1) ptr @N$BGRW(ptr %1, i16 2, i16 4)
  %77 = getelementptr i8, ptr %76, i16 0
  store i16 1, ptr %77
  %78 = getelementptr i8, ptr %77, i16 2
  store i16 2, ptr %78
  %79 = getelementptr i8, ptr %76, i16 4
  store i16 3, ptr %79
  %80 = getelementptr i8, ptr %79, i16 2
  store i16 4, ptr %80
  %81 = getelementptr i8, ptr %76, i16 -4
  %82 = load i16, ptr %81
  %83 = addrspacecast ptr %76 to ptr addrspace(1)
  %84 = icmp sge i16 %82, 1
  br i1 %84, label %b4, label %b3

b2:
  %85 = load i16, ptr %35
  %86 = icmp sge i16 %85, 2
  br i1 %86, label %b8, label %b7

b3:
  %87 = getelementptr i8, ptr @$str9, i16 6
  call addrspace(1) void @N$PS(ptr %87)
  call addrspace(1) void @N$PN()
  br label %b2

b4:
  %88 = getelementptr i8, ptr addrspace(1) %83, i16 0
  %89 = getelementptr i8, ptr addrspace(1) %88, i16 2
  %90 = getelementptr i8, ptr @$str7, i16 6
  call addrspace(1) void @N$PS(ptr %90)
  %91 = load i16, ptr addrspace(1) %88
  call addrspace(1) void @N$PI2(i16 %91)
  %92 = getelementptr i8, ptr @$str8, i16 6
  call addrspace(1) void @N$PS(ptr %92)
  %93 = load i16, ptr addrspace(1) %89
  call addrspace(1) void @N$PI2(i16 %93)
  call addrspace(1) void @N$PN()
  br label %b2

b6:
  %94 = load i16, ptr %8
  %95 = icmp eq i16 %94, 0
  %96 = sext i1 %95 to i8
  %97 = load i16, ptr %35
  %98 = icmp eq i16 %97, 0
  %99 = sext i1 %98 to i8
  call addrspace(1) void @N$PB(i8 %96)
  call addrspace(1) void @N$PS(ptr %75)
  call addrspace(1) void @N$PB(i8 %99)
  call addrspace(1) void @N$PN()
  call addrspace(1) void @N$BDRP(ptr %76)
  call addrspace(1) void @N$BDRP(ptr %3)
  call addrspace(1) void @N$BDRP(ptr %1)
  ret i16 0

b7:
  %100 = getelementptr i8, ptr @$str11, i16 6
  call addrspace(1) void @N$PS(ptr %100)
  call addrspace(1) void @N$PN()
  br label %b6

b8:
  %101 = getelementptr i8, ptr addrspace(1) %37, i16 0
  %102 = load i16, ptr addrspace(1) %101
  %103 = getelementptr i8, ptr addrspace(1) %37, i16 2
  %104 = load i16, ptr addrspace(1) %103
  %105 = icmp eq i16 %102, 1
  br i1 %105, label %b9, label %b7

b9:
  %106 = icmp eq i16 %104, 2
  br i1 %106, label %b10, label %b7

b10:
  %107 = add i16 %85, -2
  %108 = getelementptr i8, ptr @$str10, i16 6
  call addrspace(1) void @N$PS(ptr %108)
  call addrspace(1) void @N$PU2(i16 %107)
  call addrspace(1) void @N$PN()
  br label %b6
}

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare void @N$PS(ptr) addrspace(1)

declare void @N$PN() addrspace(1)

declare void @N$PI2(i16) addrspace(1)

declare void @N$PU2(i16) addrspace(1)

declare ptr @N$BGRW(ptr, i16, i16) addrspace(1)

declare void @N$PB(i8) addrspace(1)

declare void @N$BDRP(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
