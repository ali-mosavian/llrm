target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@"P&" = internal global [1024 x i8] zeroinitializer, align 2
@P$descriptor = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"P&" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"P&", [6 x i8] c"\04\00\00\01\00\00" }>
@"Q&" = internal global [16 x i8] zeroinitializer, align 2
@Q$descriptor = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"Q&" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"Q&", [6 x i8] c"\04\00\04\00\00\00" }>
@"I%" = internal global [2 x i8] zeroinitializer
@"A%" = internal global [2 x i8] zeroinitializer
@"B%" = internal global [2 x i8] zeroinitializer
@"C%" = internal global [2 x i8] zeroinitializer
@$string5 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string5, i16 4), [4 x i8] c"kept" }>
@$string6 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\06\00", ptr getelementptr (i8, ptr @$string6, i16 4), [6 x i8] c"caught" }>
@$data.2 = internal global i16 0
@$data.3 = internal global i16 0

define internal cc 1000 void @__main() addrspace(1) noinline "llrm-fp-eval"="x87-extended" noreturn personality ptr addrspace(1) @llrm.qb.personality {
frame:
  %site = alloca i16
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %site)
  store i16 0, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$CSCN(i16 1, i16 13, i16 2)
          to label %b3 unwind label %landing

b3:
  br label %b7

b7:
  %0 = phi i16 [ %18, %b7 ], [ 0, %b3 ]
  %1 = phi i16 [ %17, %b7 ], [ 0, %b3 ]
  %2 = phi ptr [ %16, %b7 ], [ @"P&", %b3 ]
  %3 = phi i16 [ %15, %b7 ], [ 256, %b3 ]
  %4 = phi i16 [ 0, %b3 ], [ %14, %b7 ]
  %5 = and i16 %4, 63
  %6 = and i16 %0, 16128
  %7 = zext nneg i16 %6 to i32
  %8 = zext nneg i16 %5 to i32
  %9 = or disjoint i32 %8, %7
  %10 = and i16 %1, 63
  %11 = zext nneg i16 %10 to i32
  %12 = shl nuw nsw i32 %11, 16
  %13 = or disjoint i32 %9, %12
  store i32 %13, ptr %2, !tbaa !2
  %14 = add nuw nsw i16 %4, 1
  %15 = add nsw i16 %3, -1
  %16 = getelementptr i8, ptr %2, i16 4
  %17 = add nuw nsw i16 %1, 5
  %18 = add i16 %0, 768
  %exitcond = icmp eq i16 %15, 0
  br i1 %exitcond, label %b8, label %b7

b8:
  store i16 256, ptr @"I%", align 1, !tbaa !2
  store i32 -1, ptr getelementptr inbounds nuw (i8, ptr @"P&", i16 28), !tbaa !2
  call void @llrm.ia16.out.i8(i16 968, i8 7) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 1) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 2) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 3) memory(inaccessiblemem: readwrite)
  store i16 1, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PALU(ptr addrspace(1) addrspacecast (ptr @"P&" to ptr addrspace(1)), ptr @P$descriptor)
          to label %b15 unwind label %landing

b15:
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 true)
  store i16 0, ptr @"I%", !tbaa !2
  store i16 255, ptr @$data.2, !tbaa !2
  store i16 51, ptr @$data.3, !tbaa !2
  br label %b17

b17:
  %19 = phi i16 [ %40, %b26 ], [ 0, %b15 ]
  %20 = phi i16 [ %39, %b26 ], [ 51, %b15 ]
  %21 = icmp sgt i16 %20, -1
  %22 = load i16, ptr @$data.2, !tbaa !2
  br i1 %21, label %b18, label %b19

b18:
  %23 = icmp sgt i16 %19, %22
  br i1 %23, label %b21, label %b20

b19:
  %24 = icmp slt i16 %19, %22
  br i1 %24, label %b21, label %b20

b20:
  %25 = trunc i16 %19 to i8
  call void @llrm.ia16.out.i8(i16 967, i8 %25) memory(inaccessiblemem: readwrite)
  %26 = call i8 @llrm.ia16.in.i8(i16 969) memory(inaccessiblemem: readwrite)
  %27 = zext i8 %26 to i16
  store i16 %27, ptr @"A%", !tbaa !2
  %28 = call i8 @llrm.ia16.in.i8(i16 969) memory(inaccessiblemem: readwrite)
  %29 = zext i8 %28 to i16
  store i16 %29, ptr @"B%", !tbaa !2
  %30 = call i8 @llrm.ia16.in.i8(i16 969) memory(inaccessiblemem: readwrite)
  %31 = zext i8 %30 to i16
  store i16 %31, ptr @"C%", !tbaa !2
  store i16 2, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %19)
          to label %52 unwind label %landing

b21:
  call void @llrm.ia16.out.i8(i16 967, i8 7) memory(inaccessiblemem: readwrite)
  %32 = call i8 @llrm.ia16.in.i8(i16 969) memory(inaccessiblemem: readwrite)
  %33 = zext i8 %32 to i16
  store i16 %33, ptr @"A%", !tbaa !2
  %34 = call i8 @llrm.ia16.in.i8(i16 969) memory(inaccessiblemem: readwrite)
  %35 = zext i8 %34 to i16
  store i16 %35, ptr @"B%", !tbaa !2
  %36 = call i8 @llrm.ia16.in.i8(i16 969) memory(inaccessiblemem: readwrite)
  %37 = zext i8 %36 to i16
  store i16 %37, ptr @"C%", !tbaa !2
  store i16 3, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string5)
          to label %58 unwind label %landing

b26:
  %38 = load i16, ptr @"I%", !tbaa !2
  %39 = load i16, ptr @$data.3, !tbaa !2
  %40 = add nsw i16 %39, %38
  store i16 %40, ptr @"I%", !tbaa !2
  br label %b17

b31:
  store i16 4, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PALU(ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @"P&", i16 800) to ptr addrspace(1)), ptr @P$descriptor)
          to label %b32 unwind label %landing

b32:
  store i16 5, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PALU(ptr addrspace(1) addrspacecast (ptr @"Q&" to ptr addrspace(1)), ptr @Q$descriptor)
          to label %b33 unwind label %landing

b33:
  store i32 64, ptr getelementptr inbounds nuw (i8, ptr @"P&", i16 36), !tbaa !2
  store i16 6, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PALU(ptr addrspace(1) addrspacecast (ptr @"P&" to ptr addrspace(1)), ptr @P$descriptor)
          to label %b35 unwind label %landing

b35:
  store i16 7, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$CSCN(i16 1, i16 0, i16 2)
          to label %b36 unwind label %landing

b36:
  call cc 1005 addrspace(1) void @llrm.qb.B$CEND()
  unreachable

landing:
  %landed = landingpad { ptr, i32 }
          catch ptr null
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 false)
  %41 = extractvalue { ptr, i32 } %landed, 1
  %42 = trunc i32 %41 to i16
  %resumed = call fastcc addrspace(1) i16 @__main$handler(i16 %42)
  %43 = load i16, ptr %site
  switch table i16 %43, label %44 [
    i16 0, label %44
    i16 1, label %45
    i16 2, label %46
    i16 3, label %47
    i16 4, label %48
    i16 5, label %49
    i16 6, label %50
    i16 7, label %51
  ]

44:
  br label %b3

45:
  br label %b15

46:
  br label %b26

47:
  br label %b31

48:
  br label %b32

49:
  br label %b33

50:
  br label %b35

51:
  br label %b36

52:
  %53 = load i16, ptr @"A%", !tbaa !2
  store i16 2, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %53)
          to label %54 unwind label %landing

54:
  %55 = load i16, ptr @"B%", !tbaa !2
  store i16 2, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %55)
          to label %56 unwind label %landing

56:
  %57 = load i16, ptr @"C%", !tbaa !2
  store i16 2, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %57)
          to label %b26 unwind label %landing

58:
  %59 = load i16, ptr @"A%", !tbaa !2
  store i16 3, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %59)
          to label %60 unwind label %landing

60:
  %61 = load i16, ptr @"B%", !tbaa !2
  store i16 3, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %61)
          to label %62 unwind label %landing

62:
  %63 = load i16, ptr @"C%", !tbaa !2
  store i16 3, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %63)
          to label %b31 unwind label %landing
}

declare cc 1000 void @llrm.qb.B$SERR(i16) addrspace(1) noreturn

declare cc 1000 void @llrm.qb.B$CSCN(i16, i16, i16) addrspace(1)

declare cc 1005 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc 1000 i16 @llrm.qb.B$FERR() addrspace(1) nounwind

declare cc 1005 void @llrm.qb.B$PEI2(i16) addrspace(1)

declare void @llrm.ia16.out.i8(i16, i8) nocallback nofree nounwind willreturn memory(readwrite)

declare cc 1005 void @llrm.qb.B$PALU(ptr addrspace(1), ptr) addrspace(1)

declare i8 @llrm.ia16.in.i8(i16) nocallback nofree nounwind willreturn memory(readwrite)

declare cc 1005 void @llrm.qb.B$PSI2(i16) addrspace(1)

declare cc 1005 void @llrm.qb.B$CEND() addrspace(1) nounwind noreturn

define internal fastcc i16 @__main$handler(i16 %0) addrspace(1) "llrm-fp-eval"="x87-extended" personality ptr addrspace(1) @llrm.qb.personality {
entry:
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string6)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %0)
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
