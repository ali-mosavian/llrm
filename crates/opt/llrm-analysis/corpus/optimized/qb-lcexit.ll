target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@"STOPAT%" = internal global [2 x i8] zeroinitializer
@$string4 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\06\00", ptr getelementptr (i8, ptr @$string4, i16 4), [6 x i8] c"COUNT=" }>
@$string5 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\06\00", ptr getelementptr (i8, ptr @$string5, i16 4), [6 x i8] c"TOTAL=" }>
@$string6 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string6, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" noreturn norecurse {
b1:
  tail call cc 1005 addrspace(1) void @llrm.qb.B$RDI2(ptr addrspace(1) addrspacecast (ptr @"STOPAT%" to ptr addrspace(1)))
  %0 = load i16, ptr @"STOPAT%"
  %1 = icmp eq i16 %0, 0
  br i1 %1, label %b4, label %2

2:
  %3 = add i16 %0, -1
  %4 = tail call i16 @llvm.umin.i16(i16 %3, i16 9)
  %5 = zext nneg i16 %4 to i32
  %6 = tail call i16 @llvm.umin.i16(i16 %3, i16 9)
  %7 = zext nneg i16 %6 to i32
  %8 = tail call i16 @llvm.umin.i16(i16 %3, i16 9)
  %9 = add nsw i16 %8, -1
  %10 = zext i16 %9 to i32
  %11 = mul nuw nsw i32 %7, %10
  %12 = lshr i32 %11, 1
  %13 = add nuw nsw i32 %12, %5
  %14 = add nuw nsw i16 %8, 1
  br label %b4

b4:
  %15 = phi i32 [ %13, %2 ], [ 0, %b1 ]
  %16 = phi i16 [ %14, %2 ], [ 0, %b1 ]
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string4)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %16)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string5)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 %15)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr @$string6)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$CEND()
  unreachable
}

declare cc 1005 void @llrm.qb.B$RDI2(ptr addrspace(1) nocapture noretain) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PSSD(ptr) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PEI2(i16) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PEI4(i32) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PESD(ptr) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$CEND() addrspace(1) nocallback nounwind noreturn

declare i32 @llvm.umin.i32(i32, i32) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare i33 @llvm.umin.i33(i33, i33) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare i16 @llvm.umin.i16(i16, i16) nocallback nofree nosync nounwind speculatable willreturn memory(none)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
