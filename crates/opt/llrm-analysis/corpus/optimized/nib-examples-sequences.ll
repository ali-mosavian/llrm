target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

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

define internal cc 1005 i16 @describe(ptr addrspace(5) nocapture readonly %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = getelementptr inbounds nuw i8, ptr addrspace(5) %0, i16 4
  %2 = load ptr addrspace(1), ptr addrspace(5) %1
  %3 = load i16, ptr addrspace(5) %0
  %4 = icmp eq i16 %3, 0
  br i1 %4, label %b4, label %5

5:
  %6 = icmp eq i16 %3, 1
  br i1 %6, label %b6, label %b5

b2:
  tail call cc 1005 addrspace(1) void @N$PN()
  ret i16 poison

b4:
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6))
  br label %b2

b5:
  %7 = shl i16 %3, 1
  %8 = getelementptr i8, ptr addrspace(1) %2, i16 %7
  %9 = getelementptr i8, ptr addrspace(1) %8, i16 -2
  %10 = add i16 %3, -2
  %11 = load i16, ptr addrspace(1) %2
  tail call cc 1005 addrspace(1) void @N$PI2(i16 %11)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str3, i16 6))
  %12 = load i16, ptr addrspace(1) %9
  tail call cc 1005 addrspace(1) void @N$PI2(i16 %12)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str4, i16 6))
  tail call cc 1005 addrspace(1) void @N$PU2(i16 %10)
  br label %b2

b6:
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str2, i16 6))
  %13 = load i16, ptr addrspace(1) %2
  tail call cc 1005 addrspace(1) void @N$PI2(i16 %13)
  br label %b2
}

define cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
  %1 = alloca [8 x i8]
  %2 = alloca [8 x i8]
  %3 = alloca [8 x i8]
  %4 = alloca [2 x i8]
  store i16 7, ptr %4, !tbaa !2
  %5 = tail call cc 1005 addrspace(1) ptr @N$BGRW(ptr getelementptr inbounds nuw (i8, ptr @$str5, i16 6), i16 4, i16 2)
  store i16 1, ptr %5
  %6 = getelementptr inbounds nuw i8, ptr %5, i16 2
  store i16 2, ptr %6
  %7 = getelementptr inbounds nuw i8, ptr %5, i16 4
  store i16 3, ptr %7
  %8 = getelementptr inbounds nuw i8, ptr %5, i16 6
  store i16 4, ptr %8
  store i32 0, ptr %3
  %9 = getelementptr inbounds nuw i8, ptr %3, i16 4
  store ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str5, i16 6) to ptr addrspace(1)), ptr %9, !tbaa !2
  %10 = addrspacecast ptr %3 to ptr addrspace(5)
  %11 = call cc 1005 addrspace(1) i16 @describe(ptr addrspace(5) %10)
  %12 = addrspacecast ptr %4 to ptr addrspace(1)
  store i32 65537, ptr %2
  %13 = getelementptr inbounds nuw i8, ptr %2, i16 4
  store ptr addrspace(1) %12, ptr %13, !tbaa !2
  %14 = addrspacecast ptr %2 to ptr addrspace(5)
  %15 = call cc 1005 addrspace(1) i16 @describe(ptr addrspace(5) %14)
  %16 = getelementptr i8, ptr %5, i16 -4
  %17 = load i16, ptr %16
  %18 = addrspacecast ptr %5 to ptr addrspace(1)
  store i16 %17, ptr %1, !tbaa !2
  %19 = getelementptr inbounds nuw i8, ptr %1, i16 2
  store i16 %17, ptr %19, !tbaa !2
  %20 = getelementptr inbounds nuw i8, ptr %1, i16 4
  store ptr addrspace(1) %18, ptr %20, !tbaa !2
  %21 = addrspacecast ptr %1 to ptr addrspace(5)
  %22 = call cc 1005 addrspace(1) i16 @describe(ptr addrspace(5) %21)
  %23 = load i16, ptr %16
  %24 = icmp sgt i16 %23, 0
  br i1 %24, label %25, label %29

25:
  %26 = add nsw i16 %23, -1
  %27 = load i16, ptr %5
  %28 = add i16 %26, %27
  br label %29

29:
  %common.op1 = phi i16 [ %28, %25 ], [ -1, %0 ]
  call cc 1005 addrspace(1) void @N$PI2(i16 %common.op1)
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str6, i16 6))
  call cc 1005 addrspace(1) void @N$PI2(i16 -1)
  call cc 1005 addrspace(1) void @N$PN()
  %30 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr getelementptr inbounds nuw (i8, ptr @$str5, i16 6), i16 2, i16 4)
  store i16 1, ptr %30
  %31 = getelementptr i8, ptr %30, i16 2
  store i16 2, ptr %31
  %32 = getelementptr inbounds nuw i8, ptr %30, i16 4
  store i16 3, ptr %32
  %33 = getelementptr i8, ptr %30, i16 6
  store i16 4, ptr %33
  %34 = getelementptr i8, ptr %30, i16 -4
  %35 = load i16, ptr %34
  %36 = icmp sgt i16 %35, 0
  br i1 %36, label %b4, label %b3

b2:
  call cc 1005 addrspace(1) void @N$PN()
  %37 = getelementptr i8, ptr %5, i16 -4
  %38 = load i16, ptr %37
  %39 = icmp sgt i16 %38, 1
  br i1 %39, label %b8, label %b7

b3:
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str9, i16 6))
  br label %b2

b4:
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str7, i16 6))
  %40 = load i16, ptr %30
  call cc 1005 addrspace(1) void @N$PI2(i16 %40)
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str8, i16 6))
  %41 = getelementptr i8, ptr %30, i16 2
  %42 = load i16, ptr %41
  call cc 1005 addrspace(1) void @N$PI2(i16 %42)
  br label %b2

b6:
  call cc 1005 addrspace(1) void @N$PN()
  %43 = getelementptr i8, ptr %5, i16 -4
  %44 = load i16, ptr %43
  %45 = icmp eq i16 %44, 0
  %46 = zext i1 %45 to i8
  call cc 1005 addrspace(1) void @N$PB(i8 1)
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str6, i16 6))
  call cc 1005 addrspace(1) void @N$PB(i8 %46)
  call cc 1005 addrspace(1) void @N$PN()
  call cc 1005 addrspace(1) void @N$BDRP(ptr nonnull %30)
  call cc 1005 addrspace(1) void @N$BDRP(ptr nonnull %5)
  call cc 1005 addrspace(1) void @N$BDRP(ptr getelementptr inbounds nuw (i8, ptr @$str5, i16 6))
  ret i16 0

b7:
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str11, i16 6))
  br label %b6

b8:
  %47 = load i16, ptr %5
  %48 = getelementptr i8, ptr %5, i16 2
  %49 = load i16, ptr %48
  %50 = icmp eq i16 %47, 1
  %51 = icmp eq i16 %49, 2
  %or.cond = select i1 %50, i1 %51, i1 false
  br i1 %or.cond, label %b10, label %b7

b10:
  %52 = add nsw i16 %38, -2
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str10, i16 6))
  call cc 1005 addrspace(1) void @N$PU2(i16 %52)
  br label %b6
}

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PI2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PU2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 ptr @N$BGRW(ptr, i16, i16) addrspace(1)

declare cc 1005 void @N$PB(i8) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$BDRP(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{i8 0, i8 2}
!6 = !{i8 0, i8 2}
