target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@"C%" = internal global [2 x i8] zeroinitializer
@"X%" = internal global [2 x i8] zeroinitializer
@$string3 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string3, i16 4), [6 x i8] c"after\00" }>
@$string4 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\03\00", ptr getelementptr (i8, ptr @$string4, i16 4), [4 x i8] c"err\00" }>
@$string5 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string5, i16 4), [4 x i8] c"loop" }>
@$string6 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string6, i16 4), [2 x i8] c"a\00" }>
@$string7 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string7, i16 4), [2 x i8] c"b\00" }>

define internal cc 1000 void @__main() addrspace(1) noinline "llrm-fp-eval"="x87-extended" noreturn personality ptr addrspace(1) @llrm.qb.personality {
frame:
  %site = alloca i16
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %site)
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 true)
  store i16 0, ptr @"C%", !tbaa !2
  store i16 300, ptr @"X%", !tbaa !2
  br label %b5

b5:
  %0 = phi i16 [ %10, %9 ], [ 300, %frame ]
  %1 = icmp slt i16 %0, 0
  br i1 %1, label %b6, label %b7

b6:
  store i16 0, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5)
          to label %15 unwind label %landing

b7:
  %2 = icmp samesign ugt i16 %0, 255
  br i1 %2, label %b8, label %b9

b8:
  store i16 0, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5)
          to label %14 unwind label %landing

b9:
  %3 = icmp eq i16 %0, 1
  br i1 %3, label %b11, label %4

4:
  %5 = icmp eq i16 %0, 2
  br i1 %5, label %b13, label %b10

b10:
  store i16 3, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr @$string3)
          to label %16 unwind label %landing

b11:
  store i16 1, ptr %site
  invoke cc 1000 addrspace(1) void @A()
          to label %b10 unwind label %landing

b13:
  store i16 2, ptr %site
  invoke cc 1000 addrspace(1) void @B()
          to label %b10 unwind label %landing

landing:
  %landed = landingpad { ptr, i32 }
          catch ptr null
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 false)
  %6 = extractvalue { ptr, i32 } %landed, 1
  %7 = trunc i32 %6 to i16
  %resumed = call fastcc addrspace(1) i16 @__main$handler(i16 %7)
  %8 = load i16, ptr %site
  switch table i16 %8, label %9 [
    i16 0, label %9
    i16 1, label %11
    i16 2, label %12
    i16 3, label %13
  ]

9:
  %10 = load i16, ptr @"X%", !tbaa !2
  br label %b5

11:
  br label %b11

12:
  br label %b13

13:
  br label %b10

14:
  unreachable

15:
  unreachable

16:
  call cc 1005 addrspace(1) void @llrm.qb.B$CEND()
  unreachable
}

define internal cc 1000 void @A() addrspace(1) noinline "llrm-fp-eval"="x87-extended" personality ptr addrspace(1) @llrm.qb.personality {
frame:
  %site = alloca i16
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %site)
  br label %b1

b1:
  store i16 0, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr @$string6)
          to label %2 unwind label %landing

landing:
  %landed = landingpad { ptr, i32 }
          catch ptr null
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 false)
  %0 = extractvalue { ptr, i32 } %landed, 1
  %1 = trunc i32 %0 to i16
  %resumed = call fastcc addrspace(1) i16 @__main$handler(i16 %1)
  br label %b1

2:
  ret void
}

define internal cc 1000 void @B() addrspace(1) noinline "llrm-fp-eval"="x87-extended" personality ptr addrspace(1) @llrm.qb.personality {
frame:
  %site = alloca i16
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %site)
  br label %b1

b1:
  store i16 0, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr @$string7)
          to label %2 unwind label %landing

landing:
  %landed = landingpad { ptr, i32 }
          catch ptr null
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 false)
  %0 = extractvalue { ptr, i32 } %landed, 1
  %1 = trunc i32 %0 to i16
  %resumed = call fastcc addrspace(1) i16 @__main$handler(i16 %1)
  br label %b1

2:
  ret void
}

declare cc 1000 void @llrm.qb.B$SERR(i16) addrspace(1) noreturn

declare cc 1005 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc 1000 i16 @llrm.qb.B$FERR() addrspace(1) nounwind

declare cc 1005 void @llrm.qb.B$PEI2(i16) addrspace(1)

declare cc 1005 void @llrm.qb.B$PESD(ptr) addrspace(1)

declare cc 1005 void @llrm.qb.B$CEND() addrspace(1) nounwind noreturn

define internal fastcc i16 @__main$handler(i16 %0) addrspace(1) "llrm-fp-eval"="x87-extended" personality ptr addrspace(1) @llrm.qb.personality {
entry:
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string4)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %0)
  %1 = load i16, ptr @"C%", !tbaa !2
  %2 = add i16 %1, 1
  store i16 %2, ptr @"C%", !tbaa !2
  %3 = icmp sgt i16 %2, 3
  br i1 %3, label %b19, label %b21

b19:
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr @$string5)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$CEND()
  unreachable

b21:
  store i16 1, ptr @"X%", !tbaa !2
  tail call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 true)
  ret i16 poison
}

declare i32 @llrm.qb.personality(...) addrspace(1)

declare cc 1000 void @llrm.qb.onerror(i1) addrspace(1) nounwind frame_bound

declare cc 1000 void @llrm.qb.frame(ptr) addrspace(1) nounwind

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
