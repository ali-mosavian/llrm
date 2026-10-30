target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [9 x i8] c"\08\00\02\00\02\00: \00"
@$str2 = internal constant [16 x i8] c"\08\00\09\00\09\00first six\00"

define cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca [12 x i8]
  call void @llvm.memset.p0.i16(ptr nonnull align 2 %0, i8 0, i16 12, i1 false)
  br label %b11

b11:
  %1 = phi i16 [ -60, %b1 ], [ %11, %b11 ]
  %2 = phi i16 [ 2024, %b1 ], [ %4, %b11 ]
  %3 = mul i16 %2, 25173
  %4 = add i16 %3, 13849
  %5 = lshr i16 %4, 8
  %.lhs.trunc1 = trunc nuw i16 %5 to i8
  %6 = urem i8 %.lhs.trunc1, 6
  %7 = zext nneg i8 %6 to i16
  %8 = getelementptr inbounds nuw [2 x i8], ptr %0, i16 %7
  %9 = load i16, ptr %8, !tbaa !2
  %10 = add i16 %9, 1
  store i16 %10, ptr %8, !tbaa !2
  %11 = add nsw i16 %1, 1
  %12 = icmp eq i16 %11, 0
  br i1 %12, label %b23, label %b11

b23:
  tail call cc 1005 addrspace(1) void @N$PI2(i16 1)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6))
  %13 = load i16, ptr %0, !tbaa !2
  tail call cc 1005 addrspace(1) void @N$PFLD(i8 zeroext 2, i8 zeroext 10, i8 zeroext 32, i8 zeroext 0)
  tail call cc 1005 addrspace(1) void @N$PU2(i16 %13)
  tail call cc 1005 addrspace(1) void @N$PN()
  tail call cc 1005 addrspace(1) void @N$PI2(i16 2)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6))
  %14 = getelementptr inbounds nuw i8, ptr %0, i16 2
  %15 = load i16, ptr %14, !tbaa !2
  tail call cc 1005 addrspace(1) void @N$PFLD(i8 zeroext 2, i8 zeroext 10, i8 zeroext 32, i8 zeroext 0)
  tail call cc 1005 addrspace(1) void @N$PU2(i16 %15)
  tail call cc 1005 addrspace(1) void @N$PN()
  tail call cc 1005 addrspace(1) void @N$PI2(i16 3)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6))
  %16 = getelementptr inbounds nuw i8, ptr %0, i16 4
  %17 = load i16, ptr %16, !tbaa !2
  tail call cc 1005 addrspace(1) void @N$PFLD(i8 zeroext 2, i8 zeroext 10, i8 zeroext 32, i8 zeroext 0)
  tail call cc 1005 addrspace(1) void @N$PU2(i16 %17)
  tail call cc 1005 addrspace(1) void @N$PN()
  tail call cc 1005 addrspace(1) void @N$PI2(i16 4)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6))
  %18 = getelementptr inbounds nuw i8, ptr %0, i16 6
  %19 = load i16, ptr %18, !tbaa !2
  tail call cc 1005 addrspace(1) void @N$PFLD(i8 zeroext 2, i8 zeroext 10, i8 zeroext 32, i8 zeroext 0)
  tail call cc 1005 addrspace(1) void @N$PU2(i16 %19)
  tail call cc 1005 addrspace(1) void @N$PN()
  tail call cc 1005 addrspace(1) void @N$PI2(i16 5)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6))
  %20 = getelementptr inbounds nuw i8, ptr %0, i16 8
  %21 = load i16, ptr %20, !tbaa !2
  tail call cc 1005 addrspace(1) void @N$PFLD(i8 zeroext 2, i8 zeroext 10, i8 zeroext 32, i8 zeroext 0)
  tail call cc 1005 addrspace(1) void @N$PU2(i16 %21)
  tail call cc 1005 addrspace(1) void @N$PN()
  tail call cc 1005 addrspace(1) void @N$PI2(i16 6)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6))
  %22 = getelementptr inbounds nuw i8, ptr %0, i16 10
  %23 = load i16, ptr %22, !tbaa !2
  tail call cc 1005 addrspace(1) void @N$PFLD(i8 zeroext 2, i8 zeroext 10, i8 zeroext 32, i8 zeroext 0)
  tail call cc 1005 addrspace(1) void @N$PU2(i16 %23)
  tail call cc 1005 addrspace(1) void @N$PN()
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str2, i16 6))
  tail call cc 1005 addrspace(1) void @N$PN()
  ret i16 0
}

declare cc 1005 void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PI2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PFLD(i8, i8, i8, i8) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PU2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: write)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{i8 0, i8 2}
