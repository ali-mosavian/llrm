target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [12 x i8] zeroinitializer
@$string3 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\03\00", ptr getelementptr (i8, ptr @$string3, i16 4), [4 x i8] c"AND\00" }>
@$string4 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string4, i16 4), [6 x i8] c"TWICE\00" }>
@$string6 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\06\00", ptr getelementptr (i8, ptr @$string6, i16 4), [6 x i8] c"NESTED" }>
@$string7 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string7, i16 4), [4 x i8] c"DONE" }>
@$string8 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string8, i16 4), [2 x i8] c"=\00" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" norecurse {
  tail call cc 1005 addrspace(1) void @llrm.qb.B$SASS(ptr @$string3, ptr @$data)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr nonnull @$data)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string8)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 33818120)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$STDL(ptr @$data)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$SASS(ptr @$string4, ptr getelementptr inbounds nuw (i8, ptr @$data, i16 4))
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr nonnull getelementptr inbounds nuw (i8, ptr @$data, i16 4))
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string8)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 67636240)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$STDL(ptr getelementptr inbounds nuw (i8, ptr @$data, i16 4))
  tail call cc 1005 addrspace(1) void @llrm.qb.B$SASS(ptr @$string6, ptr getelementptr inbounds nuw (i8, ptr @$data, i16 8))
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr nonnull getelementptr inbounds nuw (i8, ptr @$data, i16 8))
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string8)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 135272480)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$STDL(ptr getelementptr inbounds nuw (i8, ptr @$data, i16 8))
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr @$string7)
  ret void
}

define cc 1000 range(i32 0, -1) i32 @"TWICE&"(ptr dereferenceable(4) nocapture readonly %0) addrspace(1) "llrm-fp-eval"="x87-extended" memory(argmem: read) willreturn nounwind nofree nosync norecurse {
b1:
  %1 = load i32, ptr %0
  %2 = shl i32 %1, 1
  ret i32 %2
}

define cc 1000 void @REPORT(ptr dereferenceable(4) %0, ptr dereferenceable(4) nocapture readonly %1) addrspace(1) "llrm-fp-eval"="x87-extended" norecurse {
b1:
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr nonnull %0)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string8)
  %2 = load i32, ptr %1
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 %2)
  ret void
}

declare cc 1005 void @llrm.qb.B$SASS(ptr, ptr) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$STDL(ptr) addrspace(1) nocallback nounwind

declare cc 1005 void @llrm.qb.B$PESD(ptr) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PSSD(ptr) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PEI4(i32) addrspace(1) nocallback

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
