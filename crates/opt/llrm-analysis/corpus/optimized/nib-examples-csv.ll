target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"
@$str2 = internal constant [20 x i8] c"\08\00\0D\00\0D\00Ada, pilot, 7\00"
@$str3 = internal constant [26 x i8] c"\08\00\13\00\13\00Grace,  admiral , 9\00"
@$str4 = internal constant [19 x i8] c"\08\00\0C\00\0C\00Linus ,  , 3\00"
@$str5 = internal constant [18 x i8] c"\08\00\0B\00\0B\00| (no role)\00"
@$str6 = internal constant [9 x i8] c"\08\00\02\00\02\00| \00"
@$str7 = internal constant [17 x i8] c"\08\00\0A\00\0A\00 at level \00"
@$str8 = internal constant [12 x i8] c"\08\00\05\00\05\00Grace\00"
@$str9 = internal constant [8 x i8] c"\08\00\01\00\01\00 \00"

define internal cc 1005 void @field(ptr addrspace(5) nocapture writeonly %0, ptr addrspace(5) nocapture readonly %1, i16 range(i16 0, 3) %2) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" memory(read, argmem: readwrite, inaccessiblemem: readwrite) {
b1:
  %3 = load i16, ptr addrspace(5) %1
  %4 = icmp eq i16 %3, 0
  br i1 %4, label %b5.thread, label %5

5:
  %6 = getelementptr inbounds nuw i8, ptr addrspace(5) %1, i16 4
  %7 = load ptr addrspace(1), ptr addrspace(5) %6
  br label %b9

b5:
  %8 = icmp eq i16 %13, %2
  br i1 %8, label %b18, label %common.ret

b5.thread:
  %9 = icmp eq i16 %2, 0
  %.phi.trans.insert3 = getelementptr inbounds nuw i8, ptr addrspace(5) %1, i16 4
  %10 = load ptr addrspace(1), ptr addrspace(5) %.phi.trans.insert3
  br i1 %9, label %69, label %common.ret

b6:
  %11 = icmp eq i16 %16, %2
  br i1 %11, label %b14, label %b13

b8:
  %12 = phi i16 [ %22, %b13 ], [ %17, %b9 ]
  %13 = phi i16 [ %21, %b13 ], [ %16, %b9 ]
  %14 = add nuw i16 %15, 1
  %exitcond2 = icmp eq i16 %3, %14
  br i1 %exitcond2, label %b5, label %b9

b9:
  %15 = phi i16 [ %14, %b8 ], [ 0, %5 ]
  %16 = phi i16 [ 0, %5 ], [ %13, %b8 ]
  %17 = phi i16 [ 0, %5 ], [ %12, %b8 ]
  %18 = getelementptr i8, ptr addrspace(1) %7, i16 %15
  %19 = load i8, ptr addrspace(1) %18
  %20 = icmp eq i8 %19, 44
  br i1 %20, label %b6, label %b8

b13:
  %21 = add i16 %16, 1
  %22 = add i16 %15, 1
  br label %b8

b14:
  %23 = getelementptr i8, ptr addrspace(1) %7, i16 %15
  %24 = getelementptr i8, ptr addrspace(1) %23, i16 -1
  %25 = icmp ult i16 %15, %17
  br i1 %25, label %b17, label %28

common.ret:
  %sink4 = phi i16 [ %111, %107 ], [ %67, %63 ], [ 0, %b5 ], [ 0, %b5.thread ]
  %sink = phi ptr addrspace(1) [ %110, %107 ], [ %66, %63 ], [ %7, %b5 ], [ %10, %b5.thread ]
  store i16 %sink4, ptr addrspace(5) %0
  %26 = getelementptr i8, ptr addrspace(5) %0, i16 2
  store i16 %sink4, ptr addrspace(5) %26
  %27 = getelementptr i8, ptr addrspace(5) %0, i16 4
  store ptr addrspace(1) %sink, ptr addrspace(5) %27
  ret void

28:
  %29 = getelementptr inbounds i8, ptr addrspace(1) %7, i16 %17
  %30 = mul i16 %17, -1
  %31 = add i16 %15, %30
  %32 = icmp eq i16 %31, 0
  br i1 %32, label %63, label %33

33:
  %34 = getelementptr i8, ptr addrspace(1) %7, i16 %17
  %35 = sub i16 %15, %17
  br label %38

36:
  %37 = add nuw i16 %39, 1
  %exitcond1 = icmp eq i16 %35, %37
  br i1 %exitcond1, label %63, label %38

38:
  %39 = phi i16 [ %37, %36 ], [ 0, %33 ]
  %40 = getelementptr i8, ptr addrspace(1) %34, i16 %39
  %41 = load i8, ptr addrspace(1) %40
  %42 = icmp eq i8 %41, 32
  br i1 %42, label %36, label %49

43:
  %44 = getelementptr i8, ptr addrspace(1) %51, i16 -1
  %45 = icmp ugt i16 %53, %39
  br i1 %45, label %50, label %46

46:
  %47 = phi i16 [ %39, %43 ], [ %52, %55 ]
  %48 = icmp ugt i16 %47, %31
  br i1 %48, label %61, label %59

49:
  br label %50

50:
  %51 = phi ptr addrspace(1) [ %24, %49 ], [ %44, %43 ]
  %52 = phi i16 [ %53, %43 ], [ %31, %49 ]
  %53 = add i16 %52, -1
  %54 = icmp ult i16 %53, %31
  br i1 %54, label %55, label %58

55:
  %56 = load i8, ptr addrspace(1) %51
  %57 = icmp eq i8 %56, 32
  br i1 %57, label %43, label %46

58:
  tail call cc 1005 addrspace(1) void @N$EBND()
  unreachable

59:
  %60 = icmp ugt i16 %39, %47
  br i1 %60, label %62, label %63

61:
  tail call cc 1005 addrspace(1) void @N$EBND()
  unreachable

62:
  tail call cc 1005 addrspace(1) void @N$EBND()
  unreachable

63:
  %64 = phi i16 [ 0, %28 ], [ %39, %59 ], [ %31, %36 ]
  %65 = phi i16 [ 0, %28 ], [ %47, %59 ], [ %31, %36 ]
  %66 = getelementptr inbounds i8, ptr addrspace(1) %29, i16 %64
  %67 = sub nuw i16 %65, %64
  br label %common.ret

b17:
  tail call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b18:
  %68 = icmp ugt i16 %12, %3
  br i1 %68, label %b24, label %69

69:
  %70 = phi ptr addrspace(1) [ %7, %b18 ], [ %10, %b5.thread ]
  %71 = phi i16 [ %12, %b18 ], [ 0, %b5.thread ]
  %72 = getelementptr inbounds i8, ptr addrspace(1) %70, i16 %71
  %73 = sub nuw i16 %3, %71
  %74 = icmp eq i16 %73, 0
  br i1 %74, label %107, label %75

75:
  %76 = getelementptr i8, ptr addrspace(1) %70, i16 %71
  %77 = sub i16 %3, %71
  br label %80

78:
  %79 = add nuw i16 %81, 1
  %exitcond = icmp eq i16 %77, %79
  br i1 %exitcond, label %107, label %80

80:
  %81 = phi i16 [ %79, %78 ], [ 0, %75 ]
  %82 = getelementptr i8, ptr addrspace(1) %76, i16 %81
  %83 = load i8, ptr addrspace(1) %82
  %84 = icmp eq i8 %83, 32
  br i1 %84, label %78, label %91

85:
  %86 = getelementptr i8, ptr addrspace(1) %95, i16 -1
  %87 = icmp ugt i16 %97, %81
  br i1 %87, label %94, label %88

88:
  %89 = phi i16 [ %81, %85 ], [ %96, %99 ]
  %90 = icmp ugt i16 %89, %73
  br i1 %90, label %105, label %103

91:
  %92 = getelementptr i8, ptr addrspace(1) %70, i16 -1
  %93 = getelementptr i8, ptr addrspace(1) %92, i16 %3
  br label %94

94:
  %95 = phi ptr addrspace(1) [ %93, %91 ], [ %86, %85 ]
  %96 = phi i16 [ %97, %85 ], [ %73, %91 ]
  %97 = add i16 %96, -1
  %98 = icmp ult i16 %97, %73
  br i1 %98, label %99, label %102

99:
  %100 = load i8, ptr addrspace(1) %95
  %101 = icmp eq i8 %100, 32
  br i1 %101, label %85, label %88

102:
  tail call cc 1005 addrspace(1) void @N$EBND()
  unreachable

103:
  %104 = icmp ugt i16 %81, %89
  br i1 %104, label %106, label %107

105:
  tail call cc 1005 addrspace(1) void @N$EBND()
  unreachable

106:
  tail call cc 1005 addrspace(1) void @N$EBND()
  unreachable

107:
  %108 = phi i16 [ 0, %69 ], [ %81, %103 ], [ %73, %78 ]
  %109 = phi i16 [ 0, %69 ], [ %89, %103 ], [ %73, %78 ]
  %110 = getelementptr inbounds i8, ptr addrspace(1) %72, i16 %108
  %111 = sub nuw i16 %109, %108
  br label %common.ret

b24:
  tail call cc 1005 addrspace(1) void @N$EBND()
  unreachable
}

define cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
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
  %9 = tail call cc 1005 addrspace(1) ptr @N$BGRW(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6), i16 3, i16 2)
  store ptr getelementptr inbounds nuw (i8, ptr @$str2, i16 6), ptr %9
  %10 = getelementptr inbounds nuw i8, ptr %9, i16 2
  store ptr getelementptr inbounds nuw (i8, ptr @$str3, i16 6), ptr %10
  %11 = getelementptr inbounds nuw i8, ptr %9, i16 4
  store ptr getelementptr inbounds nuw (i8, ptr @$str4, i16 6), ptr %11
  %12 = getelementptr i8, ptr %9, i16 -4
  %13 = load i16, ptr %12
  %14 = icmp eq i16 %13, 0
  br i1 %14, label %b10, label %15

15:
  %16 = addrspacecast ptr %8 to ptr addrspace(5)
  %17 = addrspacecast ptr %8 to ptr addrspace(1)
  %18 = addrspacecast ptr %7 to ptr addrspace(5)
  %19 = addrspacecast ptr %6 to ptr addrspace(5)
  %20 = addrspacecast ptr %6 to ptr addrspace(1)
  %21 = addrspacecast ptr %5 to ptr addrspace(5)
  %22 = addrspacecast ptr %4 to ptr addrspace(5)
  %23 = addrspacecast ptr %4 to ptr addrspace(1)
  %24 = addrspacecast ptr %3 to ptr addrspace(5)
  br label %b3

b3:
  %25 = phi i16 [ %47, %b8 ], [ 0, %15 ]
  %26 = phi i16 [ %46, %b8 ], [ %13, %15 ]
  %27 = getelementptr i8, ptr %9, i16 %25
  %28 = load ptr, ptr %27
  %29 = getelementptr i8, ptr %28, i16 -4
  %30 = load i16, ptr %29
  %31 = addrspacecast ptr %28 to ptr addrspace(1)
  store i16 %30, ptr %7, !tbaa !2
  %32 = getelementptr i8, ptr %7, i16 2
  store i16 %30, ptr %32, !tbaa !2
  %33 = getelementptr i8, ptr %7, i16 4
  store ptr addrspace(1) %31, ptr %33, !tbaa !2
  call cc 1005 addrspace(1) void @field(ptr addrspace(5) %16, ptr addrspace(5) %18, i16 0)
  %34 = load i16, ptr %29
  store i16 %34, ptr %5, !tbaa !2
  %35 = getelementptr i8, ptr %5, i16 2
  store i16 %34, ptr %35, !tbaa !2
  %36 = getelementptr i8, ptr %5, i16 4
  store ptr addrspace(1) %31, ptr %36, !tbaa !2
  call cc 1005 addrspace(1) void @field(ptr addrspace(5) %19, ptr addrspace(5) %21, i16 1)
  %37 = load i16, ptr addrspace(5) %19
  %38 = icmp eq i16 %37, 0
  br i1 %38, label %b6, label %b7

b5:
  %39 = getelementptr i8, ptr %9, i16 -4
  %40 = load i16, ptr %39
  %41 = icmp ugt i16 %40, 1
  br i1 %41, label %b12, label %b10

b6:
  call cc 1005 addrspace(1) void @N$PFLD(i8 zeroext 6, i8 zeroext 10, i8 zeroext 32, i8 zeroext 1)
  call cc 1005 addrspace(1) void @N$PV(ptr addrspace(1) %17)
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str5, i16 6))
  br label %b8

b7:
  %42 = getelementptr i8, ptr %28, i16 -4
  %43 = load i16, ptr %42
  store i16 %43, ptr %3, !tbaa !2
  %44 = getelementptr i8, ptr %3, i16 2
  store i16 %43, ptr %44, !tbaa !2
  %45 = getelementptr i8, ptr %3, i16 4
  store ptr addrspace(1) %31, ptr %45, !tbaa !2
  call cc 1005 addrspace(1) void @field(ptr addrspace(5) %22, ptr addrspace(5) %24, i16 2)
  call cc 1005 addrspace(1) void @N$PFLD(i8 zeroext 6, i8 zeroext 10, i8 zeroext 32, i8 zeroext 1)
  call cc 1005 addrspace(1) void @N$PV(ptr addrspace(1) %17)
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str6, i16 6))
  call cc 1005 addrspace(1) void @N$PV(ptr addrspace(1) %20)
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str7, i16 6))
  call cc 1005 addrspace(1) void @N$PV(ptr addrspace(1) %23)
  br label %b8

b8:
  call cc 1005 addrspace(1) void @N$PN()
  %46 = add i16 %26, -1
  %47 = add i16 %25, 2
  %exitcond1 = icmp eq i16 %46, 0
  br i1 %exitcond1, label %b5, label %b3

b10:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b11:
  call cc 1005 addrspace(1) void @N$BDRP(ptr nonnull %9)
  ret i16 0

b12:
  %48 = addrspacecast ptr %2 to ptr addrspace(5)
  %49 = addrspacecast ptr %2 to ptr addrspace(1)
  %50 = getelementptr i8, ptr %9, i16 2
  %51 = load ptr, ptr %50
  %52 = getelementptr i8, ptr %51, i16 -4
  %53 = load i16, ptr %52
  %54 = addrspacecast ptr %51 to ptr addrspace(1)
  store i16 %53, ptr %1, !tbaa !2
  %55 = getelementptr inbounds nuw i8, ptr %1, i16 2
  store i16 %53, ptr %55, !tbaa !2
  %56 = getelementptr inbounds nuw i8, ptr %1, i16 4
  store ptr addrspace(1) %54, ptr %56, !tbaa !2
  %57 = addrspacecast ptr %1 to ptr addrspace(5)
  call cc 1005 addrspace(1) void @field(ptr addrspace(5) %48, ptr addrspace(5) %57, i16 0)
  store i32 327685, ptr %0
  %58 = getelementptr inbounds nuw i8, ptr %0, i16 4
  store ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str8, i16 6) to ptr addrspace(1)), ptr %58, !tbaa !2
  %59 = addrspacecast ptr %0 to ptr addrspace(1)
  %60 = call cc 1005 addrspace(1) i8 @N$VCMP(ptr addrspace(1) %49, ptr addrspace(1) %59)
  %61 = icmp eq i8 %60, 0
  %62 = zext i1 %61 to i8
  %63 = call cc 1005 addrspace(1) ptr @N$VCPY(ptr addrspace(1) %49)
  call cc 1005 addrspace(1) void @N$PB(i8 %62)
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str9, i16 6))
  call cc 1005 addrspace(1) void @N$PS(ptr %63)
  call cc 1005 addrspace(1) void @N$PN()
  call cc 1005 addrspace(1) void @N$BDRP(ptr %63)
  %64 = getelementptr i8, ptr %9, i16 -4
  %65 = load i16, ptr %64
  %66 = icmp eq i16 %65, 0
  br i1 %66, label %b11, label %67

67:
  br label %b15

b15:
  %68 = phi ptr [ %9, %67 ], [ %72, %b15 ]
  %69 = phi i16 [ %65, %67 ], [ %71, %b15 ]
  %70 = load ptr, ptr %68
  call cc 1005 addrspace(1) void @N$BDRP(ptr %70)
  %71 = add i16 %69, -1
  %72 = getelementptr i8, ptr %68, i16 2
  %exitcond = icmp eq i16 %71, 0
  br i1 %exitcond, label %b11, label %b15
}

declare cc 1005 void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

declare cc 1005 ptr @N$BGRW(ptr, i16, i16) addrspace(1)

declare cc 1005 void @N$PFLD(i8, i8, i8, i8) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PV(ptr addrspace(1)) addrspace(1)

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 i8 @N$VCMP(ptr addrspace(1), ptr addrspace(1)) addrspace(1) memory(read)

declare cc 1005 ptr @N$VCPY(ptr addrspace(1)) addrspace(1)

declare cc 1005 void @N$PB(i8) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$BDRP(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{i8 0, i8 2}
