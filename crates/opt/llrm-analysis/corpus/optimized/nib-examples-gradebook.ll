target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [10 x i8] c"\08\00\03\00\03\00ada\00"
@$str2 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"
@$str3 = internal constant [9 x i8] c"\08\00\02\00\02\00: \00"
@$str4 = internal constant [21 x i8] c"\08\00\0E\00\0E\00 marks, first \00"
@$str5 = internal constant [14 x i8] c"\08\00\07\00\07\00recent \00"
@$str6 = internal constant [17 x i8] c"\08\00\0A\00\0A\00, average \00"
@$str7 = internal constant [12 x i8] c"\08\00\05\00\05\00best \00"
@$str8 = internal constant [21 x i8] c"\08\00\0E\00\0E\00week 2 day 2: \00"
@$str9 = internal constant [20 x i8] c"\08\00\0D\00\0D\00, week 1 has \00"

define cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = tail call cc 1005 addrspace(1) ptr @N$BGRW(ptr getelementptr inbounds nuw (i8, ptr @$str2, i16 6), i16 1, i16 2)
  store i16 71, ptr %0
  %1 = getelementptr i8, ptr %0, i16 -4
  %2 = load i16, ptr %1
  %3 = tail call cc 1005 addrspace(1) ptr @N$BGRW(ptr %0, i16 1, i16 2)
  %4 = shl i16 %2, 1
  %5 = getelementptr inbounds i8, ptr %3, i16 %4
  store i16 64, ptr %5
  %6 = getelementptr i8, ptr %3, i16 -4
  %7 = load i16, ptr %6
  %8 = tail call cc 1005 addrspace(1) ptr @N$BGRW(ptr %3, i16 1, i16 2)
  %9 = shl i16 %7, 1
  %10 = getelementptr inbounds i8, ptr %8, i16 %9
  store i16 80, ptr %10
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6))
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str3, i16 6))
  %11 = getelementptr i8, ptr %8, i16 -4
  %12 = load i16, ptr %11
  tail call cc 1005 addrspace(1) void @N$PU2(i16 %12)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str4, i16 6))
  %13 = load i16, ptr %11
  %14 = icmp eq i16 %13, 0
  br i1 %14, label %b3, label %15

15:
  %16 = load i16, ptr %8
  tail call cc 1005 addrspace(1) void @N$PI2(i16 %16)
  tail call cc 1005 addrspace(1) void @N$PN()
  br label %17

17:
  %18 = phi i16 [ %30, %24 ], [ 0, %15 ]
  %19 = phi i16 [ %29, %24 ], [ %13, %15 ]
  %20 = phi i16 [ 0, %15 ], [ %28, %24 ]
  %21 = getelementptr i8, ptr %8, i16 -4
  %22 = load i16, ptr %21
  %23 = icmp ult i16 %20, %22
  br i1 %23, label %24, label %31

24:
  %25 = getelementptr i8, ptr %8, i16 %18
  %26 = load i16, ptr %25
  %27 = add i16 %26, 5
  store i16 %27, ptr %25
  %28 = add nuw i16 %20, 1
  %29 = add i16 %19, -1
  %30 = add i16 %18, 2
  %exitcond1 = icmp eq i16 %29, 0
  br i1 %exitcond1, label %32, label %17

31:
  tail call cc 1005 addrspace(1) void @N$EBND()
  unreachable

32:
  %33 = getelementptr i8, ptr %8, i16 -4
  %34 = load i16, ptr %33
  %35 = shl i16 %34, 1
  %36 = tail call cc 1005 addrspace(1) ptr @N$BGRW(ptr nonnull %8, i16 1, i16 2)
  %37 = getelementptr inbounds i8, ptr %36, i16 %35
  store i16 5, ptr %37
  %38 = getelementptr i8, ptr %36, i16 -4
  %39 = load i16, ptr %38
  %40 = icmp eq i16 %39, 0
  br i1 %40, label %b5, label %41

b3:
  tail call cc 1005 addrspace(1) void @N$EBND()
  unreachable

41:
  %42 = add i16 %39, -1
  %43 = icmp eq i16 %42, 0
  br i1 %43, label %55, label %44

44:
  %45 = add i16 %39, -1
  %46 = getelementptr i8, ptr %36, i16 2
  br label %47

47:
  %48 = phi ptr [ %46, %44 ], [ %54, %47 ]
  %49 = phi i16 [ %45, %44 ], [ %53, %47 ]
  %50 = phi i16 [ %52, %47 ], [ 0, %44 ]
  %51 = load i16, ptr %48
  %52 = add i16 %51, %50
  %53 = add i16 %49, -1
  %54 = getelementptr i8, ptr %48, i16 2
  %exitcond = icmp eq i16 %53, 0
  br i1 %exitcond, label %55, label %47

55:
  %56 = phi i16 [ 0, %41 ], [ %52, %47 ]
  %57 = sdiv i16 %56, %42
  %58 = srem i16 %56, %42
  %59 = icmp eq i16 %58, 0
  %60 = xor i16 %58, %42
  %61 = ashr i16 %60, 15
  %62 = select i1 %59, i16 0, i16 %61
  %63 = add i16 %62, %57
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str5, i16 6))
  tail call cc 1005 addrspace(1) void @N$PU2(i16 %42)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str6, i16 6))
  tail call cc 1005 addrspace(1) void @N$PI2(i16 %63)
  tail call cc 1005 addrspace(1) void @N$PN()
  %64 = getelementptr i8, ptr %36, i16 -4
  %65 = load i16, ptr %64
  %66 = icmp eq i16 %65, 0
  br i1 %66, label %b9, label %67

67:
  br label %b7

b5:
  tail call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b7:
  %68 = phi ptr [ %36, %67 ], [ %74, %b7 ]
  %69 = phi i16 [ %65, %67 ], [ %73, %b7 ]
  %70 = phi i16 [ %72, %b7 ], [ 0, %67 ]
  %71 = load i16, ptr %68
  %72 = tail call i16 @llvm.smax.i16(i16 %71, i16 %70)
  %73 = add i16 %69, -1
  %74 = getelementptr i8, ptr %68, i16 2
  %exitcond3 = icmp eq i16 %73, 0
  br i1 %exitcond3, label %b9, label %b7

b9:
  %75 = phi i16 [ 0, %55 ], [ %72, %b7 ]
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str7, i16 6))
  tail call cc 1005 addrspace(1) void @N$PI2(i16 %75)
  tail call cc 1005 addrspace(1) void @N$PN()
  %76 = tail call cc 1005 addrspace(1) ptr @N$BGRW(ptr getelementptr inbounds nuw (i8, ptr @$str2, i16 6), i16 1, i16 2)
  %77 = tail call cc 1005 addrspace(1) ptr @N$BGRW(ptr getelementptr inbounds nuw (i8, ptr @$str2, i16 6), i16 3, i16 2)
  store i16 1, ptr %77
  %78 = getelementptr inbounds nuw i8, ptr %77, i16 2
  store i16 2, ptr %78
  %79 = getelementptr inbounds nuw i8, ptr %77, i16 4
  store i16 3, ptr %79
  store ptr %77, ptr %76
  %80 = getelementptr i8, ptr %76, i16 -4
  %81 = load i16, ptr %80
  %82 = tail call cc 1005 addrspace(1) ptr @N$BGRW(ptr %76, i16 1, i16 2)
  %83 = shl i16 %81, 1
  %84 = getelementptr inbounds i8, ptr %82, i16 %83
  %85 = tail call cc 1005 addrspace(1) ptr @N$BGRW(ptr getelementptr inbounds nuw (i8, ptr @$str2, i16 6), i16 2, i16 2)
  store i16 4, ptr %85
  %86 = getelementptr inbounds nuw i8, ptr %85, i16 2
  store i16 5, ptr %86
  store ptr %85, ptr %84
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str8, i16 6))
  %87 = getelementptr i8, ptr %82, i16 -4
  %88 = load i16, ptr %87
  %89 = icmp ugt i16 %88, 1
  br i1 %89, label %b13, label %b14

b13:
  %90 = getelementptr inbounds nuw i8, ptr %82, i16 2
  %91 = load ptr, ptr %90
  %92 = getelementptr i8, ptr %91, i16 -4
  %93 = load i16, ptr %92
  %94 = icmp ugt i16 %93, 1
  br i1 %94, label %b15, label %b16

b14:
  tail call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b15:
  %95 = getelementptr inbounds nuw i8, ptr %91, i16 2
  %96 = load i16, ptr %95
  tail call cc 1005 addrspace(1) void @N$PI2(i16 %96)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str9, i16 6))
  %97 = getelementptr i8, ptr %82, i16 -4
  %98 = load i16, ptr %97
  %99 = icmp eq i16 %98, 0
  br i1 %99, label %b18, label %b20

b16:
  tail call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b18:
  tail call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b19:
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr nonnull %82)
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr nonnull %36)
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6))
  ret i16 0

b20:
  %100 = load ptr, ptr %82
  %101 = getelementptr i8, ptr %100, i16 -4
  %102 = load i16, ptr %101
  tail call cc 1005 addrspace(1) void @N$PU2(i16 %102)
  tail call cc 1005 addrspace(1) void @N$PN()
  br label %b23

b23:
  %103 = phi i16 [ %108, %b23 ], [ 0, %b20 ]
  %104 = phi i16 [ %107, %b23 ], [ %98, %b20 ]
  %105 = getelementptr i8, ptr %82, i16 %103
  %106 = load ptr, ptr %105
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr %106)
  %107 = add i16 %104, -1
  %108 = add i16 %103, 2
  %exitcond2 = icmp eq i16 %107, 0
  br i1 %exitcond2, label %b19, label %b23
}

declare cc 1005 ptr @N$BGRW(ptr, i16, i16) addrspace(1)

declare cc 1005 void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PU2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PI2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$BDRP(ptr) addrspace(1)

declare i16 @llvm.smax.i16(i16, i16) nocallback nofree nosync nounwind speculatable willreturn memory(none)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
