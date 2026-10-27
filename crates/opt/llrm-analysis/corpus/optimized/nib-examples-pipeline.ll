target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [14 x i8] c"\08\00\07\00\07\00closed \00"
@$str2 = internal constant [35 x i8] c"\08\00\1C\00\1C\0012 15 x 11 30 28 err 27 9 10\00"
@$str3 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"
@$str4 = internal constant [15 x i8] c"\08\00\08\00\08\00skipped \00"
@$str5 = internal constant [11 x i8] c"\08\00\04\00\04\00feed\00"
@$str6 = internal constant [12 x i8] c"\08\00\05\00\05\00mean \00"
@$str7 = internal constant [12 x i8] c"\08\00\05\00\05\00alarm\00"
@$str8 = internal constant [16 x i8] c"\08\00\09\00\09\00alarm at \00"
@$str9 = internal constant [11 x i8] c"\08\00\04\00\04\00done\00"

define cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca [54 x i8]
  %1 = alloca [54 x i8]
  store i32 0, ptr %1
  %2 = getelementptr inbounds nuw i8, ptr %1, i16 4
  store i16 0, ptr %2, !tbaa !2
  %3 = getelementptr inbounds nuw i8, ptr %1, i16 6
  store ptr getelementptr inbounds nuw (i8, ptr @$str5, i16 6), ptr %3, !tbaa !2
  %4 = getelementptr inbounds nuw i8, ptr %1, i16 8
  store i32 1835036, ptr %4
  %5 = getelementptr inbounds nuw i8, ptr %1, i16 12
  store ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str2, i16 6) to ptr addrspace(1)), ptr %5, !tbaa !2
  %6 = getelementptr inbounds nuw i8, ptr %1, i16 16
  store ptr null, ptr %6, !tbaa !2
  %7 = getelementptr inbounds nuw i8, ptr %1, i16 18
  store i16 0, ptr %7, !tbaa !2
  %8 = getelementptr inbounds nuw i8, ptr %1, i16 20
  store i32 0, ptr %8
  %9 = getelementptr inbounds nuw i8, ptr %1, i16 24
  store ptr addrspace(1) null, ptr %9, !tbaa !2
  %10 = getelementptr inbounds nuw i8, ptr %1, i16 28
  store ptr null, ptr %10, !tbaa !2
  %11 = getelementptr inbounds nuw i8, ptr %1, i16 30
  store i16 0, ptr %11, !tbaa !2
  %12 = getelementptr inbounds nuw i8, ptr %1, i16 32
  store ptr null, ptr %12, !tbaa !2
  %13 = getelementptr inbounds nuw i8, ptr %1, i16 34
  store i8 0, ptr %13, align 2, !tbaa !2
  %14 = getelementptr inbounds nuw i8, ptr %1, i16 36
  store ptr null, ptr %14, !tbaa !2
  %15 = getelementptr inbounds nuw i8, ptr %1, i16 38
  store i16 0, ptr %15, !tbaa !2
  %16 = getelementptr inbounds nuw i8, ptr %1, i16 40
  store i8 1, ptr %16, align 2, !tbaa !2
  %17 = getelementptr inbounds nuw i8, ptr %1, i16 42
  %18 = addrspacecast ptr %17 to ptr addrspace(1)
  %19 = getelementptr inbounds nuw i8, ptr %1, i16 52
  call void @llvm.memset.p1.i16(ptr addrspace(1) align 2 %18, i8 0, i16 10, i1 false)
  store i8 1, ptr %19, align 2, !tbaa !2
  %20 = addrspacecast ptr %1 to ptr addrspace(5)
  %21 = call cc 1005 addrspace(1) i32 @$state3.next(ptr addrspace(5) %20)
  %22 = trunc i32 %21 to i8
  %23 = icmp eq i8 %22, 0
  br i1 %23, label %24, label %b18

24:
  br label %b19

b18:
  %25 = getelementptr i8, ptr %1, i16 52
  %26 = load i8, ptr %25, align 2, !range !9
  %27 = icmp eq i8 %26, 0
  br i1 %27, label %b21, label %b24

b19:
  %28 = phi i32 [ %29, %b19 ], [ %21, %24 ]
  %extract.shift1 = lshr i32 %28, 16
  %extract.trunc2 = trunc nuw i32 %extract.shift1 to i16
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str6, i16 6))
  tail call cc 1005 addrspace(1) void @N$PI2(i16 %extract.trunc2)
  tail call cc 1005 addrspace(1) void @N$PN()
  %29 = call cc 1005 addrspace(1) i32 @$state3.next(ptr addrspace(5) %20)
  %extract.trunc = trunc i32 %29 to i8
  %30 = icmp eq i8 %extract.trunc, 0
  br i1 %30, label %b19, label %b18

b21:
  store i32 0, ptr %0
  %31 = getelementptr inbounds nuw i8, ptr %0, i16 4
  store i16 0, ptr %31, !tbaa !2
  %32 = getelementptr inbounds nuw i8, ptr %0, i16 6
  store ptr getelementptr inbounds nuw (i8, ptr @$str7, i16 6), ptr %32, !tbaa !2
  %33 = getelementptr inbounds nuw i8, ptr %0, i16 8
  store i32 1835036, ptr %33
  %34 = getelementptr inbounds nuw i8, ptr %0, i16 12
  store ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str2, i16 6) to ptr addrspace(1)), ptr %34, !tbaa !2
  %35 = getelementptr inbounds nuw i8, ptr %0, i16 16
  store ptr null, ptr %35, !tbaa !2
  %36 = getelementptr inbounds nuw i8, ptr %0, i16 18
  store i16 0, ptr %36, !tbaa !2
  %37 = getelementptr inbounds nuw i8, ptr %0, i16 20
  store i32 0, ptr %37
  %38 = getelementptr inbounds nuw i8, ptr %0, i16 24
  store ptr addrspace(1) null, ptr %38, !tbaa !2
  %39 = getelementptr inbounds nuw i8, ptr %0, i16 28
  store ptr null, ptr %39, !tbaa !2
  %40 = getelementptr inbounds nuw i8, ptr %0, i16 30
  store i16 0, ptr %40, !tbaa !2
  %41 = getelementptr inbounds nuw i8, ptr %0, i16 32
  store ptr null, ptr %41, !tbaa !2
  %42 = getelementptr inbounds nuw i8, ptr %0, i16 34
  store i8 0, ptr %42, align 2, !tbaa !2
  %43 = getelementptr inbounds nuw i8, ptr %0, i16 36
  store ptr null, ptr %43, !tbaa !2
  %44 = getelementptr inbounds nuw i8, ptr %0, i16 38
  store i16 0, ptr %44, !tbaa !2
  %45 = getelementptr inbounds nuw i8, ptr %0, i16 40
  store i8 1, ptr %45, align 2, !tbaa !2
  %46 = getelementptr inbounds nuw i8, ptr %0, i16 42
  %47 = addrspacecast ptr %46 to ptr addrspace(1)
  %48 = getelementptr inbounds nuw i8, ptr %0, i16 52
  call void @llvm.memset.p1.i16(ptr addrspace(1) align 2 %47, i8 0, i16 10, i1 false)
  store i8 1, ptr %48, align 2, !tbaa !2
  %49 = addrspacecast ptr %0 to ptr addrspace(5)
  br label %b41

b24:
  %50 = getelementptr i8, ptr %1, i16 36
  %51 = load ptr, ptr %50, !tbaa !2
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr %51)
  %52 = getelementptr i8, ptr %1, i16 40
  %53 = load i8, ptr %52, align 2, !tbaa !2, !range !10
  %54 = icmp eq i8 %53, 0
  br i1 %54, label %b21, label %b26

b26:
  %55 = getelementptr i8, ptr %1, i16 6
  %56 = load ptr, ptr %55, !tbaa !2
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr %56)
  %57 = getelementptr i8, ptr %1, i16 34
  %58 = load i8, ptr %57, align 2, !tbaa !2, !range !11
  %59 = icmp eq i8 %58, 0
  br i1 %59, label %b27, label %64

b27:
  %60 = getelementptr i8, ptr %1, i16 28
  %61 = load ptr, ptr %60, !tbaa !2
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr %61)
  %62 = getelementptr i8, ptr %1, i16 32
  %63 = load ptr, ptr %62, !tbaa !2
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr %63)
  br label %b21

64:
  %65 = addrspacecast ptr %6 to ptr addrspace(5)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6))
  %66 = load ptr, ptr addrspace(5) %65
  tail call cc 1005 addrspace(1) void @N$PS(ptr %66)
  tail call cc 1005 addrspace(1) void @N$PN()
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr %66)
  br label %b27

b41:
  %67 = call cc 1005 addrspace(1) i32 @$state3.next(ptr addrspace(5) %49)
  %extract.trunc3 = trunc i32 %67 to i8
  %68 = icmp eq i8 %extract.trunc3, 0
  br i1 %68, label %b46, label %b43

b43:
  %69 = getelementptr i8, ptr %0, i16 52
  %70 = load i8, ptr %69, align 2, !range !17
  %71 = icmp eq i8 %70, 0
  br i1 %71, label %b51, label %b54

b46:
  %extract.shift3 = lshr i32 %67, 16
  %extract.trunc5 = trunc nuw i32 %extract.shift3 to i16
  %72 = icmp sgt i16 %extract.trunc5, 20
  br i1 %72, label %b47, label %b41

b47:
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str8, i16 6))
  tail call cc 1005 addrspace(1) void @N$PI2(i16 %extract.trunc5)
  tail call cc 1005 addrspace(1) void @N$PN()
  br label %b43

b51:
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str9, i16 6))
  tail call cc 1005 addrspace(1) void @N$PN()
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr getelementptr inbounds nuw (i8, ptr @$str2, i16 6))
  ret i16 0

b54:
  %73 = getelementptr i8, ptr %0, i16 36
  %74 = load ptr, ptr %73, !tbaa !2
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr %74)
  %75 = getelementptr i8, ptr %0, i16 40
  %76 = load i8, ptr %75, align 2, !tbaa !2, !range !18
  %77 = icmp eq i8 %76, 0
  br i1 %77, label %b51, label %b56

b56:
  %78 = getelementptr i8, ptr %0, i16 6
  %79 = load ptr, ptr %78, !tbaa !2
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr %79)
  %80 = getelementptr i8, ptr %0, i16 34
  %81 = load i8, ptr %80, align 2, !tbaa !2, !range !19
  %82 = icmp eq i8 %81, 0
  br i1 %82, label %b57, label %87

b57:
  %83 = getelementptr i8, ptr %0, i16 28
  %84 = load ptr, ptr %83, !tbaa !2
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr %84)
  %85 = getelementptr i8, ptr %0, i16 32
  %86 = load ptr, ptr %85, !tbaa !2
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr %86)
  br label %b51

87:
  %88 = addrspacecast ptr %35 to ptr addrspace(5)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6))
  %89 = load ptr, ptr addrspace(5) %88
  tail call cc 1005 addrspace(1) void @N$PS(ptr %89)
  tail call cc 1005 addrspace(1) void @N$PN()
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr %89)
  br label %b57
}

define internal cc 1005 range(i32 0, -65535) i32 @$state3.next(ptr addrspace(5) %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b2:
  %1 = load i16, ptr addrspace(5) %0
  %2 = icmp ugt i16 %1, 8
  br i1 %2, label %b35, label %3

3:
  switch table i16 %1, label %4 [
    i16 0, label %4
    i16 1, label %7
    i16 2, label %8
    i16 3, label %9
    i16 4, label %12
    i16 5, label %13
    i16 6, label %14
    i16 7, label %14
    i16 8, label %14
  ]

4:
  %5 = addrspacecast ptr addrspace(5) %0 to ptr addrspace(1)
  %6 = getelementptr inbounds nuw i8, ptr addrspace(1) %5, i16 42
  tail call void @llvm.memset.p1.i16(ptr addrspace(1) nonnull align 2 %6, i8 0, i16 8, i1 false)
  br label %b2.jt2

7:
  br label %b35

8:
  br label %38

9:
  %10 = getelementptr i8, ptr addrspace(5) %0, i16 50
  %11 = load i16, ptr addrspace(5) %10
  br label %b16

12:
  br label %b22

13:
  br label %b30

14:
  br label %b2.jt2

b2.jt1:
  %15 = getelementptr i8, ptr addrspace(5) %0, i16 2
  store i16 0, ptr addrspace(5) %15
  %16 = getelementptr i8, ptr addrspace(5) %0, i16 4
  store i16 0, ptr addrspace(5) %16
  %17 = getelementptr i8, ptr addrspace(5) %0, i16 6
  store ptr null, ptr addrspace(5) %17
  %18 = getelementptr i8, ptr addrspace(5) %0, i16 8
  store i16 0, ptr addrspace(5) %18
  %19 = getelementptr i8, ptr addrspace(5) %0, i16 10
  store i16 0, ptr addrspace(5) %19
  %20 = getelementptr i8, ptr addrspace(5) %0, i16 12
  store ptr addrspace(1) null, ptr addrspace(5) %20
  %21 = getelementptr i8, ptr addrspace(5) %0, i16 16
  store ptr null, ptr addrspace(5) %21
  %22 = getelementptr i8, ptr addrspace(5) %0, i16 18
  store i16 0, ptr addrspace(5) %22
  %23 = getelementptr i8, ptr addrspace(5) %0, i16 20
  store i16 0, ptr addrspace(5) %23
  %24 = getelementptr i8, ptr addrspace(5) %0, i16 22
  store i16 0, ptr addrspace(5) %24
  %25 = getelementptr i8, ptr addrspace(5) %0, i16 24
  store ptr addrspace(1) null, ptr addrspace(5) %25
  %26 = getelementptr i8, ptr addrspace(5) %0, i16 28
  store ptr null, ptr addrspace(5) %26
  %27 = getelementptr i8, ptr addrspace(5) %0, i16 30
  store i16 0, ptr addrspace(5) %27
  %28 = getelementptr i8, ptr addrspace(5) %0, i16 32
  store ptr null, ptr addrspace(5) %28
  %29 = getelementptr i8, ptr addrspace(5) %0, i16 34
  store i8 0, ptr addrspace(5) %29, align 2
  %30 = getelementptr i8, ptr addrspace(5) %0, i16 36
  store ptr null, ptr addrspace(5) %30
  %31 = getelementptr i8, ptr addrspace(5) %0, i16 38
  store i16 0, ptr addrspace(5) %31
  %32 = getelementptr i8, ptr addrspace(5) %0, i16 40
  store i8 0, ptr addrspace(5) %32, align 2
  %33 = getelementptr i8, ptr addrspace(5) %0, i16 52
  store i8 0, ptr addrspace(5) %33, align 2
  br label %b35

b2.jt4:
  %34 = getelementptr i8, ptr addrspace(5) %0, i16 2
  store i16 1, ptr addrspace(5) %34
  store i16 4, ptr addrspace(5) %0
  br label %b22

b2.jt3:
  %35 = phi i16 [ %56, %54 ], [ %75, %74 ]
  %36 = getelementptr i8, ptr addrspace(5) %0, i16 2
  store i16 7, ptr addrspace(5) %36
  %37 = getelementptr i8, ptr addrspace(5) %0, i16 50
  store i16 %35, ptr addrspace(5) %37
  store i16 3, ptr addrspace(5) %0
  br label %b16

b2.jt2:
  store i16 2, ptr addrspace(5) %0
  br label %38

38:
  %39 = getelementptr i8, ptr addrspace(5) %0, i16 2
  %40 = load i16, ptr addrspace(5) %39
  %41 = icmp ugt i16 %40, 8
  br i1 %41, label %b2.jt4, label %42

42:
  switch table i16 %40, label %43 [
    i16 0, label %43
    i16 1, label %44
    i16 2, label %45
    i16 3, label %46
    i16 4, label %52
    i16 5, label %53
    i16 6, label %54
    i16 7, label %53
    i16 8, label %57
  ]

43:
  br label %79

44:
  br label %b2.jt4

45:
  br label %81

46:
  %47 = getelementptr i8, ptr addrspace(5) %0, i16 36
  %48 = load ptr, ptr addrspace(5) %47
  %49 = getelementptr i8, ptr %48, i16 -4
  %50 = load i16, ptr %49
  %51 = icmp eq i16 %50, 0
  br i1 %51, label %74, label %228

52:
  br label %241

53:
  br label %259

54:
  %55 = getelementptr i8, ptr addrspace(5) %0, i16 38
  %56 = load i16, ptr addrspace(5) %55
  br label %b2.jt3

57:
  br label %262

58:
  %59 = getelementptr i8, ptr addrspace(5) %0, i16 4
  store i16 0, ptr addrspace(5) %59
  %60 = getelementptr i8, ptr addrspace(5) %0, i16 6
  store ptr null, ptr addrspace(5) %60
  %61 = getelementptr i8, ptr addrspace(5) %0, i16 8
  store i16 0, ptr addrspace(5) %61
  %62 = getelementptr i8, ptr addrspace(5) %0, i16 10
  store i16 0, ptr addrspace(5) %62
  %63 = getelementptr i8, ptr addrspace(5) %0, i16 12
  store ptr addrspace(1) null, ptr addrspace(5) %63
  %64 = getelementptr i8, ptr addrspace(5) %0, i16 16
  store ptr null, ptr addrspace(5) %64
  %65 = getelementptr i8, ptr addrspace(5) %0, i16 18
  store i16 0, ptr addrspace(5) %65
  %66 = getelementptr i8, ptr addrspace(5) %0, i16 20
  store i16 0, ptr addrspace(5) %66
  %67 = getelementptr i8, ptr addrspace(5) %0, i16 22
  store i16 0, ptr addrspace(5) %67
  %68 = getelementptr i8, ptr addrspace(5) %0, i16 24
  store ptr addrspace(1) null, ptr addrspace(5) %68
  %69 = getelementptr i8, ptr addrspace(5) %0, i16 28
  store ptr null, ptr addrspace(5) %69
  %70 = getelementptr i8, ptr addrspace(5) %0, i16 30
  store i16 0, ptr addrspace(5) %70
  %71 = getelementptr i8, ptr addrspace(5) %0, i16 32
  store ptr null, ptr addrspace(5) %71
  %72 = getelementptr i8, ptr addrspace(5) %0, i16 34
  store i8 0, ptr addrspace(5) %72, align 2
  %73 = getelementptr i8, ptr addrspace(5) %0, i16 40
  store i8 0, ptr addrspace(5) %73, align 2
  br label %b2.jt4

74:
  %75 = phi i16 [ 0, %46 ], [ %239, %236 ]
  %76 = getelementptr i8, ptr addrspace(5) %0, i16 38
  store i16 %75, ptr addrspace(5) %76
  br label %b2.jt3

77:
  %78 = getelementptr i8, ptr addrspace(5) %0, i16 2
  store i16 4, ptr addrspace(5) %78
  br label %241

79:
  %80 = getelementptr i8, ptr addrspace(5) %0, i16 2
  store i16 2, ptr addrspace(5) %80
  br label %81

81:
  %82 = getelementptr i8, ptr addrspace(5) %0, i16 4
  %83 = load i16, ptr addrspace(5) %82
  %84 = icmp ugt i16 %83, 5
  br i1 %84, label %77, label %85

85:
  switch table i16 %83, label %86 [
    i16 0, label %86
    i16 1, label %92
    i16 2, label %93
    i16 3, label %94
    i16 4, label %101
    i16 5, label %104
  ]

86:
  %87 = getelementptr i8, ptr addrspace(5) %0, i16 6
  %88 = load ptr, ptr addrspace(5) %87
  store ptr null, ptr addrspace(5) %87
  %89 = getelementptr i8, ptr addrspace(5) %0, i16 34
  %90 = load i8, ptr addrspace(5) %89, align 2, !range !32
  %91 = icmp eq i8 %90, 0
  br i1 %91, label %114, label %130

92:
  br label %77

93:
  br label %135

94:
  %95 = getelementptr i8, ptr addrspace(5) %0, i16 4
  store i16 5, ptr addrspace(5) %95
  %96 = getelementptr i8, ptr addrspace(5) %0, i16 32
  %97 = load ptr, ptr addrspace(5) %96
  store ptr null, ptr addrspace(5) %96
  %98 = getelementptr i8, ptr addrspace(5) %0, i16 36
  %99 = load ptr, ptr addrspace(5) %98
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr %99)
  store ptr %97, ptr addrspace(5) %98
  %100 = getelementptr i8, ptr addrspace(5) %0, i16 2
  store i16 3, ptr addrspace(5) %100
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr null)
  br label %38

101:
  %102 = getelementptr i8, ptr addrspace(5) %0, i16 28
  %103 = load ptr, ptr addrspace(5) %102
  br label %214

104:
  %105 = getelementptr i8, ptr addrspace(5) %0, i16 32
  %106 = load ptr, ptr addrspace(5) %105
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr %106)
  store ptr null, ptr addrspace(5) %105
  br label %133

107:
  %108 = getelementptr i8, ptr addrspace(5) %0, i16 16
  store ptr null, ptr addrspace(5) %108
  %109 = getelementptr i8, ptr addrspace(5) %0, i16 34
  store i8 0, ptr addrspace(5) %109, align 2
  %110 = getelementptr i8, ptr addrspace(5) %0, i16 6
  %111 = load ptr, ptr addrspace(5) %110
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr %111)
  store ptr null, ptr addrspace(5) %110
  br label %77

112:
  %113 = getelementptr i8, ptr addrspace(5) %0, i16 4
  store i16 4, ptr addrspace(5) %113
  br label %214

114:
  %115 = getelementptr i8, ptr addrspace(5) %0, i16 16
  store ptr %88, ptr addrspace(5) %115
  %116 = getelementptr i8, ptr addrspace(5) %0, i16 34
  store i8 1, ptr addrspace(5) %116, align 2
  %117 = getelementptr i8, ptr addrspace(5) %0, i16 8
  %118 = load i16, ptr addrspace(5) %117
  %119 = getelementptr i8, ptr addrspace(5) %0, i16 20
  store i16 %118, ptr addrspace(5) %119
  %120 = getelementptr i8, ptr addrspace(5) %0, i16 10
  %121 = load i16, ptr addrspace(5) %120
  %122 = getelementptr i8, ptr addrspace(5) %0, i16 22
  store i16 %121, ptr addrspace(5) %122
  %123 = getelementptr i8, ptr addrspace(5) %0, i16 12
  %124 = load ptr addrspace(1), ptr addrspace(5) %123
  %125 = getelementptr i8, ptr addrspace(5) %0, i16 24
  store ptr addrspace(1) %124, ptr addrspace(5) %125
  %126 = getelementptr i8, ptr addrspace(5) %0, i16 28
  %127 = load ptr, ptr addrspace(5) %126
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr %127)
  %128 = getelementptr i8, ptr addrspace(5) %0, i16 18
  store i16 0, ptr addrspace(5) %128
  store ptr null, ptr addrspace(5) %126
  %129 = getelementptr i8, ptr addrspace(5) %0, i16 30
  store i16 0, ptr addrspace(5) %129
  br label %133

130:
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6))
  %131 = getelementptr i8, ptr addrspace(5) %0, i16 16
  %132 = load ptr, ptr addrspace(5) %131
  tail call cc 1005 addrspace(1) void @N$PS(ptr %132)
  tail call cc 1005 addrspace(1) void @N$PN()
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr %132)
  br label %114

133:
  %134 = getelementptr i8, ptr addrspace(5) %0, i16 4
  store i16 2, ptr addrspace(5) %134
  br label %135

135:
  %136 = getelementptr i8, ptr addrspace(5) %0, i16 18
  %137 = load i16, ptr addrspace(5) %136
  %138 = getelementptr i8, ptr addrspace(5) %0, i16 20
  %139 = load i16, ptr addrspace(5) %138
  %140 = getelementptr i8, ptr addrspace(5) %0, i16 24
  %141 = load ptr addrspace(1), ptr addrspace(5) %140
  %142 = getelementptr i8, ptr addrspace(5) %0, i16 28
  %143 = load ptr, ptr addrspace(5) %142, align 2
  %144 = getelementptr i8, ptr addrspace(5) %0, i16 30
  %145 = load i16, ptr addrspace(5) %144, align 2
  br label %146

146:
  %147 = phi i16 [ %205, %204 ], [ %145, %135 ]
  %148 = phi ptr [ %206, %204 ], [ %143, %135 ]
  %149 = phi i16 [ %137, %135 ], [ %sink, %204 ]
  %150 = icmp ugt i16 %149, 16
  br i1 %150, label %112, label %151

151:
  switch table i16 %149, label %152 [
    i16 0, label %152
    i16 1, label %155
    i16 2, label %156
    i16 3, label %159
    i16 4, label %161
    i16 5, label %164
    i16 6, label %169
    i16 7, label %174
    i16 8, label %180
    i16 9, label %181
    i16 10, label %182
    i16 11, label %183
    i16 12, label %184
    i16 13, label %186
    i16 14, label %187
    i16 15, label %188
    i16 16, label %187
  ]

152:
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr %148)
  %153 = getelementptr i8, ptr addrspace(5) %0, i16 28
  store ptr getelementptr inbounds nuw (i8, ptr @$str3, i16 6), ptr addrspace(5) %153
  %154 = getelementptr i8, ptr addrspace(5) %0, i16 30
  store i16 0, ptr addrspace(5) %154
  br label %204

155:
  br label %112

156:
  %157 = icmp ult i16 %147, %139
  %158 = select i1 %157, i16 3, i16 5
  br label %204

159:
  %160 = icmp ult i16 %147, %139
  br i1 %160, label %190, label %195

161:
  %162 = add i16 %147, 1
  %163 = getelementptr i8, ptr addrspace(5) %0, i16 30
  store i16 %162, ptr addrspace(5) %163
  br label %204

164:
  %165 = getelementptr i8, ptr %148, i16 -4
  %166 = load i16, ptr %165
  %167 = icmp eq i16 %166, 0
  %168 = select i1 %167, i16 14, i16 13
  br label %204

169:
  %170 = getelementptr i8, ptr %148, i16 -4
  %171 = load i16, ptr %170
  %172 = icmp eq i16 %171, 0
  %173 = select i1 %172, i16 10, i16 9
  br label %204

174:
  %175 = getelementptr i8, ptr %148, i16 -4
  %176 = load i16, ptr %175
  %177 = tail call cc 1005 addrspace(1) ptr @N$BGRW(ptr %148, i16 1, i16 1)
  %178 = getelementptr i8, ptr addrspace(5) %0, i16 28
  store ptr %177, ptr addrspace(5) %178
  %179 = icmp ult i16 %147, %139
  br i1 %179, label %196, label %203

180:
  br label %204

181:
  br label %208

182:
  br label %204

183:
  br label %204

184:
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr %148)
  %185 = getelementptr i8, ptr addrspace(5) %0, i16 28
  store ptr getelementptr inbounds nuw (i8, ptr @$str3, i16 6), ptr addrspace(5) %185
  br label %204

186:
  br label %208

187:
  br label %204

188:
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr %148)
  %189 = getelementptr i8, ptr addrspace(5) %0, i16 28
  store ptr null, ptr addrspace(5) %189
  br label %204

190:
  %191 = getelementptr inbounds i8, ptr addrspace(1) %141, i16 %147
  %192 = load i8, ptr addrspace(1) %191
  %193 = icmp eq i8 %192, 32
  %194 = select i1 %193, i16 6, i16 7
  br label %204

195:
  tail call cc 1005 addrspace(1) void @N$EBND()
  unreachable

196:
  %197 = getelementptr inbounds i8, ptr %177, i16 %176
  %198 = getelementptr inbounds i8, ptr addrspace(1) %141, i16 %147
  %199 = load i8, ptr addrspace(1) %198
  store i8 %199, ptr %197
  %200 = getelementptr i8, ptr %177, i16 -4
  %201 = load i16, ptr %200
  %202 = getelementptr inbounds i8, ptr %177, i16 %201
  store i8 0, ptr %202
  br label %204

203:
  tail call cc 1005 addrspace(1) void @N$EBND()
  unreachable

204:
  %sink = phi i16 [ 4, %180 ], [ 1, %188 ], [ 15, %187 ], [ 11, %184 ], [ 8, %183 ], [ 11, %182 ], [ 8, %196 ], [ %158, %156 ], [ %168, %164 ], [ %173, %169 ], [ %194, %190 ], [ 2, %161 ], [ 2, %152 ]
  %205 = phi i16 [ %147, %180 ], [ %147, %188 ], [ %147, %187 ], [ %147, %184 ], [ %147, %183 ], [ %147, %182 ], [ %147, %196 ], [ %147, %156 ], [ %147, %164 ], [ %147, %169 ], [ %147, %190 ], [ %162, %161 ], [ 0, %152 ]
  %206 = phi ptr [ %148, %180 ], [ null, %188 ], [ %148, %187 ], [ getelementptr inbounds nuw (i8, ptr @$str3, i16 6), %184 ], [ %148, %183 ], [ %148, %182 ], [ %177, %196 ], [ %148, %156 ], [ %148, %164 ], [ %148, %169 ], [ %148, %190 ], [ %148, %161 ], [ getelementptr inbounds nuw (i8, ptr @$str3, i16 6), %152 ]
  %207 = getelementptr i8, ptr addrspace(5) %0, i16 18
  store i16 %sink, ptr addrspace(5) %207
  br label %146

208:
  %sink1 = phi i16 [ 16, %186 ], [ 12, %181 ]
  %209 = getelementptr i8, ptr addrspace(5) %0, i16 18
  store i16 %sink1, ptr addrspace(5) %209
  %210 = getelementptr i8, ptr addrspace(5) %0, i16 28
  store ptr null, ptr addrspace(5) %210
  %211 = getelementptr i8, ptr addrspace(5) %0, i16 32
  %212 = load ptr, ptr addrspace(5) %211
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr %212)
  store ptr %148, ptr addrspace(5) %211
  %213 = getelementptr i8, ptr addrspace(5) %0, i16 4
  store i16 3, ptr addrspace(5) %213
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr null)
  br label %81

214:
  %215 = phi ptr [ %103, %101 ], [ %148, %112 ]
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr %215)
  %216 = getelementptr i8, ptr addrspace(5) %0, i16 18
  store i16 0, ptr addrspace(5) %216
  %217 = getelementptr i8, ptr addrspace(5) %0, i16 20
  store i16 0, ptr addrspace(5) %217
  %218 = getelementptr i8, ptr addrspace(5) %0, i16 22
  store i16 0, ptr addrspace(5) %218
  %219 = getelementptr i8, ptr addrspace(5) %0, i16 24
  store ptr addrspace(1) null, ptr addrspace(5) %219
  %220 = getelementptr i8, ptr addrspace(5) %0, i16 28
  store ptr null, ptr addrspace(5) %220
  %221 = getelementptr i8, ptr addrspace(5) %0, i16 30
  store i16 0, ptr addrspace(5) %221
  %222 = getelementptr i8, ptr addrspace(5) %0, i16 34
  %223 = load i8, ptr addrspace(5) %222, align 2, !range !33
  %224 = icmp eq i8 %223, 0
  br i1 %224, label %107, label %225

225:
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6))
  %226 = getelementptr i8, ptr addrspace(5) %0, i16 16
  %227 = load ptr, ptr addrspace(5) %226
  tail call cc 1005 addrspace(1) void @N$PS(ptr %227)
  tail call cc 1005 addrspace(1) void @N$PN()
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr %227)
  br label %107

228:
  %229 = phi i16 [ %239, %236 ], [ 0, %46 ]
  %230 = phi i16 [ %240, %236 ], [ 0, %46 ]
  %231 = getelementptr inbounds i8, ptr %48, i16 %230
  %232 = load i8, ptr %231
  %233 = zext nneg i8 %232 to i16
  %234 = icmp ult i16 %233, 48
  %235 = icmp ugt i16 %233, 57
  %or.cond = or i1 %234, %235
  br i1 %or.cond, label %262, label %236

236:
  %237 = mul i16 %229, 10
  %238 = add i16 %237, -48
  %239 = add i16 %238, %233
  %240 = add nuw i16 %230, 1
  %exitcond = icmp eq i16 %240, %50
  br i1 %exitcond, label %74, label %228

241:
  %242 = getelementptr i8, ptr addrspace(5) %0, i16 40
  %243 = load i8, ptr addrspace(5) %242, align 2, !range !30
  %244 = icmp eq i8 %243, 0
  br i1 %244, label %58, label %245

245:
  %246 = getelementptr i8, ptr addrspace(5) %0, i16 6
  %247 = load ptr, ptr addrspace(5) %246
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr %247)
  %248 = getelementptr i8, ptr addrspace(5) %0, i16 34
  %249 = load i8, ptr addrspace(5) %248, align 2, !range !31
  %250 = icmp eq i8 %249, 0
  br i1 %250, label %251, label %256

251:
  %252 = getelementptr i8, ptr addrspace(5) %0, i16 28
  %253 = load ptr, ptr addrspace(5) %252
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr %253)
  %254 = getelementptr i8, ptr addrspace(5) %0, i16 32
  %255 = load ptr, ptr addrspace(5) %254
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr %255)
  br label %58

256:
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6))
  %257 = getelementptr i8, ptr addrspace(5) %0, i16 16
  %258 = load ptr, ptr addrspace(5) %257
  tail call cc 1005 addrspace(1) void @N$PS(ptr %258)
  tail call cc 1005 addrspace(1) void @N$PN()
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr %258)
  br label %251

259:
  %260 = getelementptr i8, ptr addrspace(5) %0, i16 36
  %261 = load ptr, ptr addrspace(5) %260
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr %261)
  store ptr null, ptr addrspace(5) %260
  br label %79

262:
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str4, i16 6))
  %263 = getelementptr i8, ptr addrspace(5) %0, i16 36
  %264 = load ptr, ptr addrspace(5) %263
  tail call cc 1005 addrspace(1) void @N$PS(ptr %264)
  tail call cc 1005 addrspace(1) void @N$PN()
  br label %259

b16:
  %265 = phi i16 [ %11, %9 ], [ %35, %b2.jt3 ]
  %266 = getelementptr i8, ptr addrspace(5) %0, i16 48
  %267 = load i16, ptr addrspace(5) %266
  %268 = urem i16 %267, 3
  %269 = shl nuw nsw i16 %268, 1
  %270 = getelementptr i8, ptr addrspace(5) %0, i16 %269
  %271 = getelementptr i8, ptr addrspace(5) %270, i16 42
  store i16 %265, ptr addrspace(5) %271
  %272 = load i16, ptr addrspace(5) %266
  %273 = add i16 %272, 1
  store i16 %273, ptr addrspace(5) %266
  %274 = icmp ugt i16 %273, 2
  br i1 %274, label %b30, label %b2.jt2

b22:
  %275 = getelementptr i8, ptr addrspace(5) %0, i16 52
  %276 = load i8, ptr addrspace(5) %275, align 2, !range !27
  %277 = icmp eq i8 %276, 0
  br i1 %277, label %b2.jt1, label %b24

b24:
  %278 = getelementptr i8, ptr addrspace(5) %0, i16 36
  %279 = load ptr, ptr addrspace(5) %278
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr %279)
  %280 = getelementptr i8, ptr addrspace(5) %0, i16 40
  %281 = load i8, ptr addrspace(5) %280, align 2, !range !28
  %282 = icmp eq i8 %281, 0
  br i1 %282, label %b2.jt1, label %b26

b26:
  %283 = getelementptr i8, ptr addrspace(5) %0, i16 6
  %284 = load ptr, ptr addrspace(5) %283
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr %284)
  %285 = getelementptr i8, ptr addrspace(5) %0, i16 34
  %286 = load i8, ptr addrspace(5) %285, align 2, !range !29
  %287 = icmp eq i8 %286, 0
  br i1 %287, label %b27, label %292

b27:
  %288 = getelementptr i8, ptr addrspace(5) %0, i16 28
  %289 = load ptr, ptr addrspace(5) %288
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr %289)
  %290 = getelementptr i8, ptr addrspace(5) %0, i16 32
  %291 = load ptr, ptr addrspace(5) %290
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr %291)
  br label %b2.jt1

292:
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6))
  %293 = getelementptr i8, ptr addrspace(5) %0, i16 16
  %294 = load ptr, ptr addrspace(5) %293
  tail call cc 1005 addrspace(1) void @N$PS(ptr %294)
  tail call cc 1005 addrspace(1) void @N$PN()
  %295 = load ptr, ptr addrspace(5) %293
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr %295)
  br label %b27

common.ret:
  %common.op = phi i32 [ %insert.shift1, %b30 ], [ 1, %b35 ]
  ret i32 %common.op

b30:
  store i16 8, ptr addrspace(5) %0
  %296 = getelementptr i8, ptr addrspace(5) %0, i16 42
  %297 = load i16, ptr addrspace(5) %296
  %298 = getelementptr i8, ptr addrspace(5) %0, i16 44
  %299 = load i16, ptr addrspace(5) %298
  %300 = add i16 %299, %297
  %301 = getelementptr i8, ptr addrspace(5) %0, i16 46
  %302 = load i16, ptr addrspace(5) %301
  %303 = add i16 %300, %302
  %304 = sdiv i16 %303, 3
  %305 = srem i16 %303, 3
  %306 = ashr i16 %305, 15
  %307 = add nsw i16 %306, %304
  %insert.ext3 = zext i16 %307 to i32
  %insert.shift1 = shl nuw i32 %insert.ext3, 16
  br label %common.ret

b35:
  store i16 1, ptr addrspace(5) %0
  br label %common.ret
}

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PI2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$BDRP(ptr) addrspace(1)

declare cc 1005 void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

declare cc 1005 ptr @N$BGRW(ptr, i16, i16) addrspace(1)

declare void @llvm.memset.p1.i16(ptr addrspace(1) nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @llvm.memcpy.p1.p1.i16(ptr addrspace(1) noalias nocapture writeonly, ptr addrspace(1) noalias nocapture readonly, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)

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
!35 = !{i8 0, i8 2}
!36 = !{i8 0, i8 2}
!37 = !{i8 0, i8 2}
