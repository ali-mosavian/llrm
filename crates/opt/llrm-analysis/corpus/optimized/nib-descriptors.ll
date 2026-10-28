target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [12 x i8] c"\08\00\05\00\05\00metal\00"
@$str2 = internal constant [22 x i8] c"\08\00\0F\00\0F\00descriptors: ok\00"

define cc 1005 range(i16 0, 2) i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
common.ret:
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6))
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str2, i16 6))
  tail call cc 1005 addrspace(1) void @N$PN()
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr null)
  ret i16 0
}

declare cc 1005 void @N$BDRP(ptr) addrspace(1)

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
