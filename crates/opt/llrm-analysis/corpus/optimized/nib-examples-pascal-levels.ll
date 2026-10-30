target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [9 x i8] c"\08\00\02\00\02\00..\00"
@$str2 = internal constant [19 x i8] c"\08\00\0C\00\0C\00 clamped to \00"
@$str3 = internal constant [12 x i8] c"\08\00\05\00\05\00% -> \00"

define cc 1000 i16 @CLAMP(i16 %0, i16 %1, i16 %2) addrspace(1) "llrm-fp-eval"="x87-extended" memory(none) willreturn nounwind nofree nosync norecurse {
b1:
  %3 = icmp slt i16 %0, %1
  %4 = tail call i16 @llvm.smin.i16(i16 %0, i16 %2)
  %5 = select i1 %3, i16 %1, i16 %4
  ret i16 %5
}

define cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca [12 x i8]
  %1 = getelementptr inbounds nuw i8, ptr %0, i16 2
  store i32 1048536, ptr %0
  %2 = getelementptr inbounds nuw i8, ptr %0, i16 4
  %3 = getelementptr inbounds nuw i8, ptr %0, i16 6
  store i32 6488134, ptr %2
  %4 = getelementptr inbounds nuw i8, ptr %0, i16 8
  %5 = getelementptr inbounds nuw i8, ptr %0, i16 10
  store i32 17039500, ptr %4
  %6 = addrspacecast ptr %0 to ptr addrspace(1)
  %7 = call cc 1000 addrspace(1) i32 @SPAN(ptr addrspace(1) %6, i16 6)
  %extract.trunc = trunc i32 %7 to i16
  %extract.shift = lshr i32 %7, 16
  %extract.trunc1 = trunc nuw i32 %extract.shift to i16
  call cc 1000 addrspace(1) void @CLAMP_ALL(ptr addrspace(1) %6, i16 6, i16 0, i16 100)
  %8 = call cc 1000 addrspace(1) i32 @SPAN(ptr addrspace(1) %6, i16 6)
  %extract.trunc2 = trunc i32 %8 to i16
  %extract.shift1 = lshr i32 %8, 16
  %extract.trunc3 = trunc nuw i32 %extract.shift1 to i16
  call cc 1005 addrspace(1) void @N$PI2(i16 %extract.trunc)
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6))
  call cc 1005 addrspace(1) void @N$PI2(i16 %extract.trunc1)
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str2, i16 6))
  call cc 1005 addrspace(1) void @N$PI2(i16 %extract.trunc2)
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6))
  call cc 1005 addrspace(1) void @N$PI2(i16 %extract.trunc3)
  call cc 1005 addrspace(1) void @N$PN()
  %9 = load i16, ptr %0, !tbaa !2
  %10 = call cc 1000 addrspace(1) i16 @SCALE(i16 %9, i16 255, i16 100)
  call cc 1005 addrspace(1) void @N$PFLD(i8 zeroext 3, i8 zeroext 10, i8 zeroext 32, i8 zeroext 0)
  call cc 1005 addrspace(1) void @N$PI2(i16 %9)
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str3, i16 6))
  call cc 1005 addrspace(1) void @N$PFLD(i8 zeroext 3, i8 zeroext 10, i8 zeroext 32, i8 zeroext 0)
  call cc 1005 addrspace(1) void @N$PI2(i16 %10)
  call cc 1005 addrspace(1) void @N$PN()
  %11 = load i16, ptr %1, !tbaa !2
  %12 = call cc 1000 addrspace(1) i16 @SCALE(i16 %11, i16 255, i16 100)
  call cc 1005 addrspace(1) void @N$PFLD(i8 zeroext 3, i8 zeroext 10, i8 zeroext 32, i8 zeroext 0)
  call cc 1005 addrspace(1) void @N$PI2(i16 %11)
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str3, i16 6))
  call cc 1005 addrspace(1) void @N$PFLD(i8 zeroext 3, i8 zeroext 10, i8 zeroext 32, i8 zeroext 0)
  call cc 1005 addrspace(1) void @N$PI2(i16 %12)
  call cc 1005 addrspace(1) void @N$PN()
  %13 = load i16, ptr %2, !tbaa !2
  %14 = call cc 1000 addrspace(1) i16 @SCALE(i16 %13, i16 255, i16 100)
  call cc 1005 addrspace(1) void @N$PFLD(i8 zeroext 3, i8 zeroext 10, i8 zeroext 32, i8 zeroext 0)
  call cc 1005 addrspace(1) void @N$PI2(i16 %13)
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str3, i16 6))
  call cc 1005 addrspace(1) void @N$PFLD(i8 zeroext 3, i8 zeroext 10, i8 zeroext 32, i8 zeroext 0)
  call cc 1005 addrspace(1) void @N$PI2(i16 %14)
  call cc 1005 addrspace(1) void @N$PN()
  %15 = load i16, ptr %3, !tbaa !2
  %16 = call cc 1000 addrspace(1) i16 @SCALE(i16 %15, i16 255, i16 100)
  call cc 1005 addrspace(1) void @N$PFLD(i8 zeroext 3, i8 zeroext 10, i8 zeroext 32, i8 zeroext 0)
  call cc 1005 addrspace(1) void @N$PI2(i16 %15)
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str3, i16 6))
  call cc 1005 addrspace(1) void @N$PFLD(i8 zeroext 3, i8 zeroext 10, i8 zeroext 32, i8 zeroext 0)
  call cc 1005 addrspace(1) void @N$PI2(i16 %16)
  call cc 1005 addrspace(1) void @N$PN()
  %17 = load i16, ptr %4, !tbaa !2
  %18 = call cc 1000 addrspace(1) i16 @SCALE(i16 %17, i16 255, i16 100)
  call cc 1005 addrspace(1) void @N$PFLD(i8 zeroext 3, i8 zeroext 10, i8 zeroext 32, i8 zeroext 0)
  call cc 1005 addrspace(1) void @N$PI2(i16 %17)
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str3, i16 6))
  call cc 1005 addrspace(1) void @N$PFLD(i8 zeroext 3, i8 zeroext 10, i8 zeroext 32, i8 zeroext 0)
  call cc 1005 addrspace(1) void @N$PI2(i16 %18)
  call cc 1005 addrspace(1) void @N$PN()
  %19 = load i16, ptr %5, !tbaa !2
  %20 = call cc 1000 addrspace(1) i16 @SCALE(i16 %19, i16 255, i16 100)
  call cc 1005 addrspace(1) void @N$PFLD(i8 zeroext 3, i8 zeroext 10, i8 zeroext 32, i8 zeroext 0)
  call cc 1005 addrspace(1) void @N$PI2(i16 %19)
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str3, i16 6))
  call cc 1005 addrspace(1) void @N$PFLD(i8 zeroext 3, i8 zeroext 10, i8 zeroext 32, i8 zeroext 0)
  call cc 1005 addrspace(1) void @N$PI2(i16 %20)
  call cc 1005 addrspace(1) void @N$PN()
  ret i16 0
}

declare cc 1000 i32 @SPAN(ptr addrspace(1), i16) addrspace(1)

declare cc 1000 void @CLAMP_ALL(ptr addrspace(1), i16, i16, i16) addrspace(1)

declare cc 1005 void @N$PI2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1000 i16 @SCALE(i16, i16, i16) addrspace(1)

declare cc 1005 void @N$PFLD(i8, i8, i8, i8) addrspace(1) memory(inaccessiblemem: readwrite)

declare i16 @llvm.smin.i16(i16, i16) nocallback nofree nosync nounwind speculatable willreturn memory(none)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
