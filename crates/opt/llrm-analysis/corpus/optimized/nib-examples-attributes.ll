target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [14 x i8] c"\08\00\07\00\07\00normal \00"
@$str2 = internal constant [17 x i8] c"\08\00\0A\00\0A\00, warning \00"
@$str3 = internal constant [18 x i8] c"\08\00\0B\00\0B\00, inverted \00"
@$str4 = internal constant [34 x i8] c"\08\00\1B\00\1B\00byte waiting, ready to send\00"
@$str5 = internal constant [15 x i8] c"\08\00\08\00\08\00errors: \00"
@$str6 = internal constant [15 x i8] c"\08\00\08\00\08\00control \00"
@$str7 = internal constant [16 x i8] c"\08\00\09\00\09\00: parity \00"
@$str8 = internal constant [16 x i8] c"\08\00\09\00\09\00, offset \00"

define cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6))
  tail call cc 1005 addrspace(1) void @N$PU1(i8 zeroext 7)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str2, i16 6))
  tail call cc 1005 addrspace(1) void @N$PU1(i8 zeroext -50)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str3, i16 6))
  tail call cc 1005 addrspace(1) void @N$PU1(i8 zeroext 112)
  tail call cc 1005 addrspace(1) void @N$PN()
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str4, i16 6))
  tail call cc 1005 addrspace(1) void @N$PN()
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str5, i16 6))
  tail call cc 1005 addrspace(1) void @N$PB(i8 0)
  tail call cc 1005 addrspace(1) void @N$PN()
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str6, i16 6))
  tail call cc 1005 addrspace(1) void @N$PU1(i8 zeroext 55)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str7, i16 6))
  tail call cc 1005 addrspace(1) void @N$PU1(i8 zeroext 2)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str8, i16 6))
  tail call cc 1005 addrspace(1) void @N$PI1(i8 signext 1)
  tail call cc 1005 addrspace(1) void @N$PN()
  ret i16 0
}

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PU1(i8) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PB(i8) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PI1(i8) addrspace(1) memory(inaccessiblemem: readwrite)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{i8 0, i8 2}
!6 = !{i8 0, i8 2}
