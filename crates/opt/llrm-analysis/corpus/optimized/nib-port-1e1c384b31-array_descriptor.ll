target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

define cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" memory(inaccessiblemem: readwrite) {
b1:
  tail call cc 1005 addrspace(1) void @N$PU2(i16 3)
  tail call cc 1005 addrspace(1) void @N$PN()
  ret i16 10
}

declare cc 1005 void @N$PU2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
