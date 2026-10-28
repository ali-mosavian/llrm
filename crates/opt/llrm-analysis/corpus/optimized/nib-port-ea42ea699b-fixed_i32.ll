target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

define cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" memory(inaccessiblemem: readwrite) {
  tail call cc 1005 addrspace(1) void @N$PQ4(i32 262144, i8 zeroext 9)
  tail call cc 1005 addrspace(1) void @N$PN()
  tail call cc 1005 addrspace(1) void @N$PQ4(i32 -262144, i8 zeroext 9)
  tail call cc 1005 addrspace(1) void @N$PN()
  tail call cc 1005 addrspace(1) void @N$PQ4(i32 -512, i8 zeroext 9)
  tail call cc 1005 addrspace(1) void @N$PN()
  tail call cc 1005 addrspace(1) void @N$PQ4(i32 -2147483648, i8 zeroext 9)
  tail call cc 1005 addrspace(1) void @N$PN()
  tail call cc 1005 addrspace(1) void @N$PQ4(i32 25600000, i8 zeroext 9)
  tail call cc 1005 addrspace(1) void @N$PN()
  ret i16 0
}

declare i32 @llvm.smul.fix.i32(i32, i32, i32 immarg) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare i32 @llvm.sdiv.fix.i32(i32, i32, i32 immarg) nocallback nofree nosync nounwind willreturn memory(none)

declare cc 1005 void @N$PQ4(i32, i8) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
