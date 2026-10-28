target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"
@$str2 = internal constant [13 x i8] c"\08\00\06\00\06\00linear\00"
@$str3 = internal constant [9 x i8] c"\08\00\02\00\02\00in\00"
@$str4 = internal constant [10 x i8] c"\08\00\03\00\03\00out\00"
@$str5 = internal constant [13 x i8] c"\08\00\06\00\06\00smooth\00"
@$str6 = internal constant [11 x i8] c"\08\00\04\00\04\00step\00"
@$str7 = internal constant [8 x i8] c"\08\00\01\00\01\00 \00"

define cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = tail call cc 1005 addrspace(1) ptr @N$BGRW(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6), i16 5, i16 2)
  store ptr getelementptr inbounds nuw (i8, ptr @$str2, i16 6), ptr %0
  %1 = getelementptr inbounds nuw i8, ptr %0, i16 2
  store ptr getelementptr inbounds nuw (i8, ptr @$str3, i16 6), ptr %1
  %2 = getelementptr inbounds nuw i8, ptr %0, i16 4
  store ptr getelementptr inbounds nuw (i8, ptr @$str4, i16 6), ptr %2
  %3 = getelementptr inbounds nuw i8, ptr %0, i16 6
  store ptr getelementptr inbounds nuw (i8, ptr @$str5, i16 6), ptr %3
  %4 = getelementptr inbounds nuw i8, ptr %0, i16 8
  store ptr getelementptr inbounds nuw (i8, ptr @$str6, i16 6), ptr %4
  %5 = tail call cc 1005 addrspace(1) ptr @N$BGRW(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6), i16 5, i16 2)
  store i16 0, ptr %5
  %6 = getelementptr inbounds nuw i8, ptr %5, i16 2
  store i16 1, ptr %6
  %7 = getelementptr inbounds nuw i8, ptr %5, i16 4
  store i16 2, ptr %7
  %8 = getelementptr inbounds nuw i8, ptr %5, i16 6
  store i16 3, ptr %8
  %9 = getelementptr inbounds nuw i8, ptr %5, i16 8
  store i16 4, ptr %9
  %10 = getelementptr i8, ptr %5, i16 -4
  %11 = load i16, ptr %10
  %12 = icmp eq i16 %11, 0
  br i1 %12, label %b5.thread, label %13

b5.thread:
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr nonnull %5)
  br label %b15

13:
  br label %b3

b3:
  %14 = phi i16 [ 0, %13 ], [ %83, %b13 ]
  %15 = getelementptr i8, ptr %5, i16 -4
  %16 = load i16, ptr %15
  %17 = icmp ult i16 %14, %16
  br i1 %17, label %b6, label %b7

b5:
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr %5)
  %18 = icmp eq ptr %0, null
  br i1 %18, label %b14, label %b15

b6:
  %19 = shl i16 %14, 1
  %20 = getelementptr inbounds i8, ptr %5, i16 %19
  %21 = load i16, ptr %20
  tail call cc 1005 addrspace(1) void @N$PBEG()
  %22 = getelementptr i8, ptr %0, i16 -4
  %23 = load i16, ptr %22
  %24 = icmp ult i16 %14, %23
  br i1 %24, label %b8, label %b9

b7:
  tail call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b8:
  %25 = getelementptr inbounds i8, ptr %0, i16 %19
  %26 = load ptr, ptr %25
  tail call cc 1005 addrspace(1) void @N$PFLD(i8 zeroext 6, i8 zeroext 10, i8 zeroext 32, i8 zeroext 1)
  tail call cc 1005 addrspace(1) void @N$PS(ptr %26)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str7, i16 6))
  %27 = tail call cc 1005 addrspace(1) ptr @N$PEND()
  br label %28

b9:
  tail call cc 1005 addrspace(1) void @N$EBND()
  unreachable

28:
  %29 = phi i16 [ %82, %71 ], [ 0, %b8 ]
  %30 = phi i16 [ %81, %71 ], [ 0, %b8 ]
  %31 = phi i16 [ %80, %71 ], [ 200, %b8 ]
  %32 = phi i16 [ %79, %71 ], [ 100, %b8 ]
  %33 = phi ptr [ %27, %b8 ], [ %78, %71 ]
  %34 = icmp ugt i16 %21, 3
  br i1 %34, label %46, label %35

35:
  switch table i16 %21, label %36 [
    i16 0, label %36
    i16 1, label %37
    i16 2, label %40
    i16 3, label %44
  ]

36:
  br label %71

37:
  %38 = mul nuw nsw i16 %29, %29
  %39 = udiv i16 %38, 100
  br label %71

40:
  %41 = mul nuw nsw i16 %32, %32
  %42 = udiv i16 %41, 100
  %43 = sub nuw nsw i16 100, %42
  br label %71

44:
  %45 = icmp samesign ult i16 %29, 50
  br i1 %45, label %49, label %59

46:
  %tmp = trunc i16 %29 to i8
  %47 = udiv i8 %tmp, 50
  %.zext = zext nneg i8 %47 to i16
  %48 = mul nuw nsw i16 %.zext, 50
  br label %71

49:
  %50 = mul nuw i16 %30, %30
  %51 = sdiv i16 %50, 100
  %52 = srem i16 %50, 100
  %53 = ashr i16 %52, 15
  %54 = add nsw i16 %53, %51
  %55 = sdiv i16 %54, 2
  %56 = lshr i16 %54, 15
  %57 = and i16 %56, %54
  %58 = sub nsw i16 %55, %57
  br label %71

59:
  %60 = mul nuw i16 %31, %31
  %61 = sdiv i16 %60, -100
  %62 = srem i16 %60, 100
  %63 = lshr i16 %62, 15
  %64 = add nsw i16 %61, 100
  %65 = add nsw i16 %64, %63
  %66 = sdiv i16 %65, 2
  %67 = lshr i16 %65, 15
  %68 = and i16 %67, %65
  %69 = add nsw i16 %66, 50
  %70 = sub nsw i16 %69, %68
  br label %71

71:
  %common.op = phi i16 [ %39, %37 ], [ %43, %40 ], [ %48, %46 ], [ %58, %49 ], [ %70, %59 ], [ %29, %36 ]
  %72 = mul nsw i16 %common.op, 40
  %73 = sdiv i16 %72, 100
  %74 = srem i16 %72, 100
  %75 = ashr i16 %74, 15
  %76 = add nsw i16 %75, %73
  tail call cc 1005 addrspace(1) void @N$PBEG()
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str7, i16 6))
  tail call cc 1005 addrspace(1) void @N$PFLD(i8 zeroext 3, i8 zeroext 10, i8 zeroext 32, i8 zeroext 0)
  tail call cc 1005 addrspace(1) void @N$PI2(i16 %76)
  %77 = tail call cc 1005 addrspace(1) ptr @N$PEND()
  %78 = tail call cc 1005 addrspace(1) ptr @N$TAPP(ptr %33, ptr %77)
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr %77)
  %79 = add nsw i16 %32, -20
  %80 = add nsw i16 %31, -40
  %81 = add nuw nsw i16 %30, 40
  %82 = add nuw nsw i16 %29, 20
  %exitcond1 = icmp eq i16 %79, -20
  br i1 %exitcond1, label %b13, label %28

b13:
  tail call cc 1005 addrspace(1) void @N$PS(ptr %78)
  tail call cc 1005 addrspace(1) void @N$PN()
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr %78)
  %83 = add nuw i16 %14, 1
  %exitcond2 = icmp eq i16 %83, %11
  br i1 %exitcond2, label %b5, label %b3

b14:
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr %0)
  ret i16 0

b15:
  %84 = getelementptr i8, ptr %0, i16 -4
  %85 = load i16, ptr %84
  %86 = icmp eq i16 %85, 0
  br i1 %86, label %b14, label %87

87:
  br label %b18

b18:
  %88 = phi ptr [ %0, %87 ], [ %92, %b18 ]
  %89 = phi i16 [ %85, %87 ], [ %91, %b18 ]
  %90 = load ptr, ptr %88
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr %90)
  %91 = add i16 %89, -1
  %92 = getelementptr i8, ptr %88, i16 2
  %exitcond = icmp eq i16 %91, 0
  br i1 %exitcond, label %b14, label %b18
}

declare cc 1005 ptr @N$BGRW(ptr, i16, i16) addrspace(1)

declare cc 1005 void @N$BDRP(ptr) addrspace(1)

declare cc 1005 void @N$PBEG() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PFLD(i8, i8, i8, i8) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 ptr @N$PEND() addrspace(1)

declare cc 1005 void @N$PI2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 ptr @N$TAPP(ptr, ptr) addrspace(1)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
