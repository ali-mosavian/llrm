target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [11 x i8] c"\08\00\04\00\04\00lap \00"
@$str2 = internal constant [9 x i8] c"\08\00\02\00\02\00: \00"
@$str3 = internal constant [8 x i8] c"\08\00\01\00\01\00s\00"
@$str4 = internal constant [19 x i8] c"\08\00\0C\00\0C\00fastest lap \00"
@$str5 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"
@$str6 = internal constant [10 x i8] c"\08\00\03\00\03\00ada\00"
@$str7 = internal constant [10 x i8] c"\08\00\03\00\03\00bob\00"
@$str8 = internal constant [13 x i8] c"\08\00\06\00\06\00 made \00"
@$str9 = internal constant [11 x i8] c"\08\00\04\00\04\00 is \00"
@$str10 = internal constant [13 x i8] c"\08\00\06\00\06\00 short\00"
@$str11 = internal constant [15 x i8] c"\08\00\08\00\08\00s pace: \00"
@$str12 = internal constant [20 x i8] c"\08\00\0D\00\0D\00s for 12 laps\00"

define cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
  %1 = alloca [4 x i8]
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6))
  tail call cc 1005 addrspace(1) void @N$PU2(i16 1)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str2, i16 6))
  tail call cc 1005 addrspace(1) void @N$PI2(i16 62)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str3, i16 6))
  tail call cc 1005 addrspace(1) void @N$PN()
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6))
  tail call cc 1005 addrspace(1) void @N$PU2(i16 2)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str2, i16 6))
  tail call cc 1005 addrspace(1) void @N$PI2(i16 58)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str3, i16 6))
  tail call cc 1005 addrspace(1) void @N$PN()
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6))
  tail call cc 1005 addrspace(1) void @N$PU2(i16 3)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str2, i16 6))
  tail call cc 1005 addrspace(1) void @N$PI2(i16 60)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str3, i16 6))
  tail call cc 1005 addrspace(1) void @N$PN()
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6))
  tail call cc 1005 addrspace(1) void @N$PU2(i16 4)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str2, i16 6))
  tail call cc 1005 addrspace(1) void @N$PI2(i16 57)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str3, i16 6))
  tail call cc 1005 addrspace(1) void @N$PN()
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str4, i16 6))
  tail call cc 1005 addrspace(1) void @N$PU2(i16 4)
  tail call cc 1005 addrspace(1) void @N$PN()
  %2 = tail call cc 1005 addrspace(1) ptr @N$BGRW(ptr getelementptr inbounds nuw (i8, ptr @$str5, i16 6), i16 2, i16 4)
  store ptr getelementptr inbounds nuw (i8, ptr @$str6, i16 6), ptr %2
  %3 = getelementptr i8, ptr %2, i16 2
  store i16 12, ptr %3
  %4 = getelementptr inbounds nuw i8, ptr %2, i16 4
  store ptr getelementptr inbounds nuw (i8, ptr @$str7, i16 6), ptr %4
  %5 = getelementptr i8, ptr %2, i16 6
  store i16 9, ptr %5
  store i32 655370, ptr %1
  %6 = getelementptr i8, ptr %2, i16 -4
  %7 = load i16, ptr %6
  %8 = addrspacecast ptr %1 to ptr addrspace(5)
  %9 = icmp eq i16 %7, 0
  br i1 %9, label %b35, label %10

10:
  %11 = add i16 %7, -1
  %12 = call i16 @llvm.umin.i16(i16 %11, i16 1)
  %13 = add nuw nsw i16 %12, 1
  %14 = getelementptr i8, ptr %2, i16 2
  br label %b23

b23:
  %15 = phi ptr [ %27, %b29 ], [ %14, %10 ]
  %16 = phi ptr addrspace(5) [ %26, %b29 ], [ %8, %10 ]
  %17 = phi i16 [ %25, %b29 ], [ %13, %10 ]
  %18 = getelementptr i8, ptr %15, i16 -2
  %19 = load i16, ptr %15
  %20 = load i16, ptr addrspace(5) %16
  %21 = icmp slt i16 %19, %20
  %22 = load ptr, ptr %18
  tail call cc 1005 addrspace(1) void @N$PS(ptr %22)
  br i1 %21, label %b28, label %b27

b27:
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str8, i16 6))
  tail call cc 1005 addrspace(1) void @N$PI2(i16 %20)
  br label %b29

b28:
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str9, i16 6))
  %23 = load i16, ptr %15
  %24 = sub i16 %20, %23
  tail call cc 1005 addrspace(1) void @N$PI2(i16 %24)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str10, i16 6))
  br label %b29

b29:
  tail call cc 1005 addrspace(1) void @N$PN()
  %25 = add nsw i16 %17, -1
  %26 = getelementptr i8, ptr addrspace(5) %16, i16 2
  %27 = getelementptr i8, ptr %15, i16 4
  %28 = icmp eq i16 %25, 0
  br i1 %28, label %b35, label %b23

b34:
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr nonnull %2)
  ret i16 0

b35:
  tail call cc 1005 addrspace(1) void @N$PI2(i16 58)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str11, i16 6))
  tail call cc 1005 addrspace(1) void @N$PI2(i16 696)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str12, i16 6))
  tail call cc 1005 addrspace(1) void @N$PN()
  tail call cc 1005 addrspace(1) void @N$PI2(i16 59)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str11, i16 6))
  tail call cc 1005 addrspace(1) void @N$PI2(i16 708)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str12, i16 6))
  tail call cc 1005 addrspace(1) void @N$PN()
  tail call cc 1005 addrspace(1) void @N$PI2(i16 60)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str11, i16 6))
  tail call cc 1005 addrspace(1) void @N$PI2(i16 720)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str12, i16 6))
  tail call cc 1005 addrspace(1) void @N$PN()
  %29 = getelementptr i8, ptr %2, i16 -4
  %30 = load i16, ptr %29
  %31 = icmp eq i16 %30, 0
  br i1 %31, label %b34, label %32

32:
  br label %b38

b38:
  %33 = phi ptr [ %2, %32 ], [ %37, %b38 ]
  %34 = phi i16 [ %30, %32 ], [ %36, %b38 ]
  %35 = load ptr, ptr %33
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr %35)
  %36 = add i16 %34, -1
  %37 = getelementptr i8, ptr %33, i16 4
  %exitcond = icmp eq i16 %36, 0
  br i1 %exitcond, label %b34, label %b38
}

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PU2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 ptr @N$BGRW(ptr, i16, i16) addrspace(1)

declare cc 1005 void @N$PI2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$BDRP(ptr) addrspace(1)

declare i16 @llvm.umin.i16(i16, i16) nocallback nofree nosync nounwind speculatable willreturn memory(none)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{i16 0, i16 16384}
!6 = !{i16 0, i16 -32768}
