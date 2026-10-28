target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$string3 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string3, i16 4), [4 x i8] c"AND=" }>
@$string4 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\03\00", ptr getelementptr (i8, ptr @$string4, i16 4), [4 x i8] c"OR=\00" }>
@$string5 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string5, i16 4), [4 x i8] c"XOR=" }>
@$string6 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string6, i16 4), [4 x i8] c"ADD=" }>
@$string7 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string7, i16 4), [4 x i8] c"SUB=" }>
@$string8 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string8, i16 4), [4 x i8] c"NEG=" }>
@$string9 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\06\00", ptr getelementptr (i8, ptr @$string9, i16 4), [6 x i8] c"CHAIN=" }>
@$string10 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\06\00", ptr getelementptr (i8, ptr @$string10, i16 4), [6 x i8] c"CARRY=" }>
@$string11 = internal constant <{ [2 x i8], ptr, [8 x i8] }> <{ [2 x i8] c"\07\00", ptr getelementptr (i8, ptr @$string11, i16 4), [8 x i8] c"BORROW=\00" }>
@$string12 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string12, i16 4), [6 x i8] c"INTS=\00" }>
@$string13 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string13, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" norecurse {
b1:
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string3)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 33818120)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string4)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 524246911)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string5)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 490428791)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string6)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 558065031)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string7)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 52774761)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string8)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 -305419896)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string9)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 524246911)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string10)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 65536)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string11)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 65535)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string12)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 258)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 772)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr @$string13)
  ret void
}

declare cc 1005 void @llrm.qb.B$PSSD(ptr) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PEI4(i32) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PSI2(i16) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PEI2(i16) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PESD(ptr) addrspace(1) nocallback

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
