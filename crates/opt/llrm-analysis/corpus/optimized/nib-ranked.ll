target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [8 x i8] c"\08\00\01\00\01\00 \00"

define cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  tail call cc 1005 addrspace(1) void @N$PI2(i16 15)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6))
  tail call cc 1005 addrspace(1) void @N$PI2(i16 106)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6))
  tail call cc 1005 addrspace(1) void @N$PI2(i16 162)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6))
  tail call cc 1005 addrspace(1) void @N$PI4(i32 42)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6))
  tail call cc 1005 addrspace(1) void @N$PU2(i16 9)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6))
  tail call cc 1005 addrspace(1) void @N$PU2(i16 3)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6))
  tail call cc 1005 addrspace(1) void @N$PU2(i16 4)
  tail call cc 1005 addrspace(1) void @N$PN()
  ret i16 0
}

declare cc 1005 void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PI2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PI4(i32) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PU2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare i16 @llvm.umin.i16(i16, i16) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: write)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{i16 0, i16 -32768}
!6 = !{i16 0, i16 -32768}
!7 = !{i16 0, i16 -32768}
!8 = !{i16 0, i16 -32768}
