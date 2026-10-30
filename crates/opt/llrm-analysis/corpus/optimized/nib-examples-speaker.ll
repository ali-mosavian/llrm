target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [27 x i8] c"\08\00\14\00\14\00playing; a key stops\00"
@$str2 = internal constant [14 x i8] c"\08\00\07\00\07\00stopped\00"
@$str3 = internal constant [11 x i8] c"\08\00\04\00\04\00done\00"

define cc 1005 range(i16 0, 2) i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca [60 x i8]
  store i32 262474, ptr %0
  %1 = getelementptr inbounds nuw i8, ptr %0, i16 4
  store i32 262474, ptr %1
  %2 = getelementptr inbounds nuw i8, ptr %0, i16 8
  store i32 262493, ptr %2
  %3 = getelementptr inbounds nuw i8, ptr %0, i16 12
  store i32 262536, ptr %3
  %4 = getelementptr inbounds nuw i8, ptr %0, i16 16
  store i32 262536, ptr %4
  %5 = getelementptr inbounds nuw i8, ptr %0, i16 20
  store i32 262493, ptr %5
  %6 = getelementptr inbounds nuw i8, ptr %0, i16 24
  store i32 262474, ptr %6
  %7 = getelementptr inbounds nuw i8, ptr %0, i16 28
  store i32 262438, ptr %7
  %8 = getelementptr inbounds nuw i8, ptr %0, i16 32
  store i32 262406, ptr %8
  %9 = getelementptr inbounds nuw i8, ptr %0, i16 36
  store i32 262406, ptr %9
  %10 = getelementptr inbounds nuw i8, ptr %0, i16 40
  store i32 262438, ptr %10
  %11 = getelementptr inbounds nuw i8, ptr %0, i16 44
  store i32 262474, ptr %11
  %12 = getelementptr inbounds nuw i8, ptr %0, i16 48
  store i32 393546, ptr %12
  %13 = getelementptr inbounds nuw i8, ptr %0, i16 52
  store i32 131366, ptr %13
  %14 = getelementptr inbounds nuw i8, ptr %0, i16 56
  store i32 524582, ptr %14
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6))
  tail call cc 1005 addrspace(1) void @N$PN()
  br label %15

15:
  %16 = phi i16 [ 0, %b1 ], [ %73, %72 ]
  %17 = getelementptr inbounds nuw [4 x i8], ptr %0, i16 %16
  %18 = load i16, ptr %17, !tbaa !2
  %19 = zext i16 %18 to i32
  %20 = udiv i32 1193182, %19
  %21 = trunc i32 %20 to i16
  tail call void @llrm.ia16.asm.b0b6e64388d8e64288f8e642e4610c03e661.bx.-.ax_flags.n(i16 %21)
  %22 = getelementptr inbounds nuw i8, ptr %17, i16 2
  %23 = load i16, ptr %22, !tbaa !2
  %24 = tail call { i16, i16 } @llrm.ia16.asm.cd1a.ax.cx_dx.ax_flags.n(i16 0)
  %25 = icmp eq i16 %23, 0
  br i1 %25, label %51, label %26

26:
  %27 = extractvalue { i16, i16 } %24, 0
  %28 = zext i16 %27 to i32
  %29 = shl nuw i32 %28, 16
  %30 = extractvalue { i16, i16 } %24, 1
  %31 = zext i16 %30 to i32
  %32 = or disjoint i32 %29, %31
  br label %33

33:
  %34 = phi i32 [ %32, %26 ], [ %45, %38 ]
  %35 = phi i16 [ %23, %26 ], [ %49, %38 ]
  %36 = tail call i16 @llrm.ia16.asm.b401cd16b800007405cd16b80100.-.ax.flags.n()
  %37 = icmp eq i16 %36, 0
  br i1 %37, label %38, label %b6

38:
  %39 = tail call { i16, i16 } @llrm.ia16.asm.cd1a.ax.cx_dx.ax_flags.n(i16 0)
  %40 = extractvalue { i16, i16 } %39, 0
  %41 = extractvalue { i16, i16 } %39, 1
  %42 = zext i16 %40 to i32
  %43 = shl nuw i32 %42, 16
  %44 = zext i16 %41 to i32
  %45 = or disjoint i32 %43, %44
  %46 = icmp eq i32 %45, %34
  %47 = xor i1 %46, true
  %48 = sext i1 %47 to i16
  %49 = add i16 %35, %48
  %50 = icmp eq i16 %49, 0
  br i1 %50, label %51, label %33

common.ret:
  %sink = phi ptr [ getelementptr inbounds nuw (i8, ptr @$str2, i16 6), %b6 ], [ getelementptr inbounds nuw (i8, ptr @$str3, i16 6), %72 ]
  %common.op = phi i16 [ 1, %b6 ], [ 0, %72 ]
  tail call cc 1005 addrspace(1) void @N$PS(ptr %sink)
  tail call cc 1005 addrspace(1) void @N$PN()
  ret i16 %common.op

b6:
  tail call void @llrm.ia16.asm.e46124fce661.-.-.ax_flags.n()
  br label %common.ret

51:
  tail call void @llrm.ia16.asm.e46124fce661.-.-.ax_flags.n()
  %52 = tail call { i16, i16 } @llrm.ia16.asm.cd1a.ax.cx_dx.ax_flags.n(i16 0)
  %53 = extractvalue { i16, i16 } %52, 0
  %54 = zext i16 %53 to i32
  %55 = shl nuw i32 %54, 16
  %56 = extractvalue { i16, i16 } %52, 1
  %57 = zext i16 %56 to i32
  %58 = or disjoint i32 %55, %57
  br label %59

59:
  %60 = phi i32 [ %58, %51 ], [ %70, %63 ]
  %61 = tail call i16 @llrm.ia16.asm.b401cd16b800007405cd16b80100.-.ax.flags.n()
  %62 = icmp eq i16 %61, 0
  br i1 %62, label %63, label %72

63:
  %64 = tail call { i16, i16 } @llrm.ia16.asm.cd1a.ax.cx_dx.ax_flags.n(i16 0)
  %65 = extractvalue { i16, i16 } %64, 0
  %66 = extractvalue { i16, i16 } %64, 1
  %67 = zext i16 %65 to i32
  %68 = shl nuw i32 %67, 16
  %69 = zext i16 %66 to i32
  %70 = or disjoint i32 %68, %69
  %71 = icmp eq i32 %70, %60
  br i1 %71, label %59, label %72

72:
  %73 = add nuw nsw i16 %16, 1
  %exitcond = icmp eq i16 %73, 15
  br i1 %exitcond, label %common.ret, label %15
}

declare { i16, i16 } @llrm.ia16.asm.cd1a.ax.cx_dx.ax_flags.n(i16) nounwind memory(inaccessiblemem: readwrite)

declare void @llrm.ia16.asm.b0b6e64388d8e64288f8e642e4610c03e661.bx.-.ax_flags.n(i16) nounwind memory(inaccessiblemem: readwrite)

declare void @llrm.ia16.asm.e46124fce661.-.-.ax_flags.n() nounwind memory(inaccessiblemem: readwrite)

declare i16 @llrm.ia16.asm.b401cd16b800007405cd16b80100.-.ax.flags.n() nounwind memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{i8 0, i8 2}
