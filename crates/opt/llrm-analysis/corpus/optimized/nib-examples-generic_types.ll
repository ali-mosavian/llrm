target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [10 x i8] c"\08\00\03\00\03\00ada\00"
@$str2 = internal constant [8 x i8] c"\08\00\01\00\01\00 \00"
@$str3 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"
@$str4 = internal constant [11 x i8] c"\08\00\04\00\04\00top \00"
@$str5 = internal constant [12 x i8] c"\08\00\05\00\05\00empty\00"
@$str6 = internal constant [14 x i8] c"\08\00\07\00\07\00stored \00"
@$str7 = internal constant [15 x i8] c"\08\00\08\00\08\00full at \00"
@$str8 = internal constant [18 x i8] c"\08\00\0B\00\0B\00first roll \00"
@$str11 = internal constant [19 x i8] c"\08\00\0C\00\0C\00nothing held\00"

define cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6))
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str2, i16 6))
  tail call cc 1005 addrspace(1) void @N$PI2(i16 42)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str2, i16 6))
  tail call cc 1005 addrspace(1) void @N$PU1(i8 zeroext 7)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str2, i16 6))
  tail call cc 1005 addrspace(1) void @N$PB(i8 1)
  tail call cc 1005 addrspace(1) void @N$PN()
  %1 = tail call cc 1005 addrspace(1) ptr @N$BGRW(ptr getelementptr inbounds nuw (i8, ptr @$str3, i16 6), i16 2, i16 2)
  store i16 5, ptr %1
  %2 = getelementptr inbounds nuw i8, ptr %1, i16 2
  store i16 9, ptr %2
  %3 = getelementptr i8, ptr %1, i16 -4
  %4 = load i16, ptr %3
  %5 = icmp eq i16 %4, 0
  br i1 %5, label %b3, label %b4

b2:
  tail call cc 1005 addrspace(1) void @N$PN()
  %6 = tail call cc 1005 addrspace(1) ptr @N$BGRW(ptr getelementptr inbounds nuw (i8, ptr @$str3, i16 6), i16 3, i16 2)
  store i16 11, ptr %6
  %7 = getelementptr inbounds nuw i8, ptr %6, i16 2
  store i16 12, ptr %7
  %8 = getelementptr inbounds nuw i8, ptr %6, i16 4
  store i16 13, ptr %8
  %9 = getelementptr i8, ptr %6, i16 -4
  %10 = load i16, ptr %9
  %11 = icmp eq i16 %10, 0
  br i1 %11, label %b19, label %12

12:
  br label %17

b3:
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str5, i16 6))
  br label %b2

b4:
  %13 = shl i16 %4, 1
  %14 = getelementptr i8, ptr %1, i16 %13
  %15 = getelementptr i8, ptr %14, i16 -2
  %16 = load i16, ptr %15
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str4, i16 6))
  tail call cc 1005 addrspace(1) void @N$PI2(i16 %16)
  br label %b2

17:
  %18 = phi ptr [ %6, %12 ], [ %27, %b10 ]
  %19 = phi i16 [ %10, %12 ], [ %26, %b10 ]
  %20 = phi ptr [ %24, %b10 ], [ %1, %12 ]
  %21 = getelementptr i8, ptr %20, i16 -4
  %22 = load i16, ptr %21
  %23 = icmp eq i16 %22, 3
  br i1 %23, label %b10, label %b12

b10:
  %sink1 = phi ptr [ getelementptr inbounds nuw (i8, ptr @$str6, i16 6), %b12 ], [ getelementptr inbounds nuw (i8, ptr @$str7, i16 6), %17 ]
  %24 = phi ptr [ %29, %b12 ], [ %20, %17 ]
  tail call cc 1005 addrspace(1) void @N$PS(ptr %sink1)
  %25 = load i16, ptr %18
  tail call cc 1005 addrspace(1) void @N$PI2(i16 %25)
  tail call cc 1005 addrspace(1) void @N$PN()
  %26 = add i16 %19, -1
  %27 = getelementptr i8, ptr %18, i16 2
  %exitcond = icmp eq i16 %26, 0
  br i1 %exitcond, label %b19, label %17

b12:
  %28 = load i16, ptr %18
  %29 = tail call cc 1005 addrspace(1) ptr @N$BGRW(ptr nonnull %20, i16 1, i16 2)
  %30 = shl i16 %22, 1
  %31 = getelementptr inbounds i8, ptr %29, i16 %30
  store i16 %28, ptr %31
  br label %b10

b19:
  %32 = phi ptr [ %1, %b2 ], [ %24, %b10 ]
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr nonnull %6)
  tail call cc 1005 addrspace(1) void @N$PU1(i8 zeroext -56)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str2, i16 6))
  tail call cc 1005 addrspace(1) void @N$PI2(i16 -1)
  tail call cc 1005 addrspace(1) void @N$PN()
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str8, i16 6))
  tail call cc 1005 addrspace(1) void @N$PI2(i16 4)
  tail call cc 1005 addrspace(1) void @N$PN()
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str11, i16 6))
  tail call cc 1005 addrspace(1) void @N$PN()
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr nonnull %32)
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6))
  ret i16 0
}

declare cc 1005 ptr @N$BGRW(ptr, i16, i16) addrspace(1)

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PI2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PU1(i8) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PB(i8) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$BDRP(ptr) addrspace(1)

declare cc 1005 void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{i8 0, i8 2}
!6 = !{i8 0, i8 2}
