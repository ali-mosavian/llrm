target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" norecurse {
b1:
  %0 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$FHEX(i32 65535)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr %0)
  %1 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$FHEX(i32 65520)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr %1)
  %2 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$FHEX(i32 65534)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr %2)
  %3 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$FHEX(i32 255)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr %3)
  %4 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$FHEX(i32 32767)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr %4)
  %5 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$FOCT(i32 65535)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr %5)
  %6 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$FOCT(i32 65520)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr %6)
  %7 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$FOCT(i32 65534)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr %7)
  %8 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$FOCT(i32 255)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr %8)
  %9 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$FHEX(i32 -1)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr %9)
  %10 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$FHEX(i32 -16)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr %10)
  %11 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$FOCT(i32 -1)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr %11)
  %12 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$FHEX(i32 65535)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr %12)
  %13 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$FOCT(i32 65535)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr %13)
  %14 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$FHEX(i32 -16)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr %14)
  %15 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$FOCT(i32 -16)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr %15)
  %16 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$FHEX(i32 65535)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr %16)
  %17 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$FHEX(i32 65536)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr %17)
  %18 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$FHEX(i32 -32768)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr %18)
  %19 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$FHEX(i32 65535)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr %19)
  %20 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$FOCT(i32 65535)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr %20)
  %21 = tail call i32 @llvm.lrint.i32.f32(float -1.500000e+00)
  %22 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$FHEX(i32 %21)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr %22)
  %23 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$FHEX(i32 100000)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr %23)
  ret void
}

declare cc 1005 ptr @llrm.qb.B$FHEX(i32) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PSSD(ptr) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PESD(ptr) addrspace(1) nocallback

declare cc 1005 ptr @llrm.qb.B$FOCT(i32) addrspace(1) nocallback

declare i32 @llvm.lrint.i32.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
