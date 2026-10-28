target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$string3 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string3, i16 4), [4 x i8] c"ELT=" }>
@$string4 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string4, i16 4), [4 x i8] c"ELE=" }>
@$string5 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string5, i16 4), [4 x i8] c"EGT=" }>
@$string6 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string6, i16 4), [4 x i8] c"EGE=" }>
@$string7 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string7, i16 4), [4 x i8] c"EEQ=" }>
@$string8 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string8, i16 4), [4 x i8] c"ENE=" }>
@$string9 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string9, i16 4), [4 x i8] c"FLT=" }>
@$string10 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string10, i16 4), [4 x i8] c"FLE=" }>
@$string11 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string11, i16 4), [4 x i8] c"FGT=" }>
@$string12 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string12, i16 4), [4 x i8] c"FGE=" }>
@$string13 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string13, i16 4), [4 x i8] c"FEQ=" }>
@$string14 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string14, i16 4), [4 x i8] c"FNE=" }>
@$string15 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string15, i16 4), [4 x i8] c"GLT=" }>
@$string16 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string16, i16 4), [4 x i8] c"GLE=" }>
@$string17 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string17, i16 4), [4 x i8] c"GGT=" }>
@$string18 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string18, i16 4), [4 x i8] c"GGE=" }>
@$string19 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string19, i16 4), [4 x i8] c"GEQ=" }>
@$string20 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string20, i16 4), [4 x i8] c"GNE=" }>
@$string21 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string21, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" norecurse {
b1:
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string3)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 -1)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 0)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string4)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 -1)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 0)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string5)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 0)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 -1)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string6)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 0)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 -1)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string7)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 0)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 0)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string8)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 -1)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 -1)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string9)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 -1)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 0)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string10)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 -1)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 0)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string11)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 0)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 -1)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string12)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 0)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 -1)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string13)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 0)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 0)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string14)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 -1)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 -1)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string15)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 -1)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 0)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string16)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 -1)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 0)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string17)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 0)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 -1)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string18)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 0)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 -1)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string19)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 0)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 0)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string20)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 -1)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 -1)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr @$string21)
  ret void
}

declare cc 1005 void @llrm.qb.B$PSSD(ptr) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PSI2(i16) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PEI2(i16) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PESD(ptr) addrspace(1) nocallback

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
