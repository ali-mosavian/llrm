target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$string3 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string3, i16 4), [4 x i8] c"ONE=" }>
@$string4 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\06\00", ptr getelementptr (i8, ptr @$string4, i16 4), [6 x i8] c"CONST=" }>
@$string5 = internal constant <{ [2 x i8], ptr, [8 x i8] }> <{ [2 x i8] c"\07\00", ptr getelementptr (i8, ptr @$string5, i16 4), [8 x i8] c"CONST2=\00" }>
@$string6 = internal constant <{ [2 x i8], ptr, [8 x i8] }> <{ [2 x i8] c"\07\00", ptr getelementptr (i8, ptr @$string6, i16 4), [8 x i8] c"MODMOD=\00" }>
@$string7 = internal constant <{ [2 x i8], ptr, [8 x i8] }> <{ [2 x i8] c"\07\00", ptr getelementptr (i8, ptr @$string7, i16 4), [8 x i8] c"DIVDIV=\00" }>
@$string8 = internal constant <{ [2 x i8], ptr, [8 x i8] }> <{ [2 x i8] c"\07\00", ptr getelementptr (i8, ptr @$string8, i16 4), [8 x i8] c"NEGMOD=\00" }>
@$string9 = internal constant <{ [2 x i8], ptr, [8 x i8] }> <{ [2 x i8] c"\07\00", ptr getelementptr (i8, ptr @$string9, i16 4), [8 x i8] c"NEGDIV=\00" }>
@$string10 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string10, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" noreturn norecurse {
b1:
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string3)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 0)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string4)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 0)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string5)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 13106)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string6)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 13106)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string7)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 9)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string8)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 -13106)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string9)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 -9)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr @$string10)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$CEND()
  unreachable
}

declare cc 1000 void @llrm.qb.B$SERR(i16) addrspace(1) nocallback noreturn

declare cc 1005 void @llrm.qb.B$PSSD(ptr) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PEI4(i32) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PESD(ptr) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$CEND() addrspace(1) nocallback nounwind noreturn

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
