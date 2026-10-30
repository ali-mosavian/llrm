target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [28 x i8] c"\08\00\15\00\15\00move  north then east\00"
@$str2 = internal constant [8 x i8] c"\08\00\01\00\01\00[\00"
@$str3 = internal constant [8 x i8] c"\08\00\01\00\01\00]\00"
@$str4 = internal constant [15 x i8] c"\08\00\08\00\08\00 words, \00"
@$str5 = internal constant [22 x i8] c"\08\00\0F\00\0F\00 letters, head \00"

define cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
  %1 = alloca [8 x i8]
  %2 = alloca [8 x i8]
  %3 = addrspacecast ptr %2 to ptr addrspace(5)
  store i32 262148, ptr addrspace(5) %3
  %4 = getelementptr inbounds nuw i8, ptr addrspace(5) %3, i16 4
  store ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6) to ptr addrspace(1)), ptr addrspace(5) %4
  %5 = addrspacecast ptr %1 to ptr addrspace(5)
  %6 = addrspacecast ptr %1 to ptr addrspace(1)
  %7 = getelementptr inbounds nuw i8, ptr addrspace(5) %5, i16 4
  store i32 327685, ptr addrspace(5) %5
  store ptr addrspace(1) getelementptr inbounds nuw (i8, ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6) to ptr addrspace(1)), i16 6), ptr addrspace(5) %7
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str2, i16 6))
  call cc 1005 addrspace(1) void @N$PV(ptr addrspace(1) %6)
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str3, i16 6))
  call cc 1005 addrspace(1) void @N$PN()
  br label %10

8:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

9:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

10:
  %11 = phi i16 [ 11, %0 ], [ %17, %16 ]
  %12 = getelementptr i8, ptr @$str1, i16 %11
  %13 = getelementptr i8, ptr %12, i16 6
  %14 = load i8, ptr %13
  %15 = icmp eq i8 %14, 32
  br i1 %15, label %16, label %19

16:
  %17 = add nuw nsw i16 %11, 1
  %18 = icmp eq i16 %17, 21
  br i1 %18, label %34, label %10

19:
  br label %20

20:
  %21 = phi i16 [ %27, %26 ], [ %11, %19 ]
  %22 = getelementptr i8, ptr @$str1, i16 %21
  %23 = getelementptr i8, ptr %22, i16 6
  %24 = load i8, ptr %23
  %25 = icmp eq i8 %24, 32
  br i1 %25, label %29, label %26

26:
  %27 = add nuw nsw i16 %21, 1
  %28 = icmp eq i16 %27, 21
  br i1 %28, label %31, label %20

29:
  %30 = icmp ugt i16 %21, 21
  br i1 %30, label %8, label %31

31:
  %32 = phi i16 [ %21, %29 ], [ 21, %26 ]
  %33 = icmp ugt i16 %11, %32
  br i1 %33, label %9, label %34

34:
  %35 = phi i16 [ %11, %31 ], [ 21, %16 ]
  %36 = phi i16 [ %32, %31 ], [ 21, %16 ]
  %37 = getelementptr inbounds nuw i8, ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6) to ptr addrspace(1)), i16 %35
  %38 = sub nuw nsw i16 %36, %35
  store i16 %38, ptr addrspace(5) %5
  %39 = getelementptr i8, ptr addrspace(5) %5, i16 2
  store i16 %38, ptr addrspace(5) %39
  %40 = getelementptr i8, ptr addrspace(5) %5, i16 4
  store ptr addrspace(1) %37, ptr addrspace(5) %40
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str2, i16 6))
  call cc 1005 addrspace(1) void @N$PV(ptr addrspace(1) %6)
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str3, i16 6))
  call cc 1005 addrspace(1) void @N$PN()
  %41 = icmp samesign ult i16 %36, 21
  br i1 %41, label %42, label %65

42:
  br label %43

43:
  %44 = phi i16 [ %50, %49 ], [ %36, %42 ]
  %45 = getelementptr i8, ptr @$str1, i16 %44
  %46 = getelementptr i8, ptr %45, i16 6
  %47 = load i8, ptr %46
  %48 = icmp eq i8 %47, 32
  br i1 %48, label %49, label %52

49:
  %50 = add nuw nsw i16 %44, 1
  %51 = icmp eq i16 %50, 21
  br i1 %51, label %65, label %43

52:
  br label %53

53:
  %54 = phi i16 [ %60, %59 ], [ %44, %52 ]
  %55 = getelementptr i8, ptr @$str1, i16 %54
  %56 = getelementptr i8, ptr %55, i16 6
  %57 = load i8, ptr %56
  %58 = icmp eq i8 %57, 32
  br i1 %58, label %62, label %59

59:
  %60 = add nuw i16 %54, 1
  %61 = icmp eq i16 %60, 21
  br i1 %61, label %62, label %53

62:
  %63 = phi i16 [ 21, %59 ], [ %54, %53 ]
  %64 = icmp ugt i16 %44, %63
  br i1 %64, label %9, label %65

65:
  %66 = phi i16 [ 21, %34 ], [ %44, %62 ], [ 21, %49 ]
  %67 = phi i16 [ 21, %34 ], [ %63, %62 ], [ 21, %49 ]
  %68 = add nuw nsw i16 %38, 5
  %69 = addrspacecast ptr %2 to ptr addrspace(1)
  %70 = getelementptr inbounds nuw i8, ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6) to ptr addrspace(1)), i16 %66
  %71 = sub nuw nsw i16 %67, %66
  store i16 %71, ptr addrspace(5) %5
  %72 = getelementptr i8, ptr addrspace(5) %5, i16 2
  store i16 %71, ptr addrspace(5) %72
  %73 = getelementptr i8, ptr addrspace(5) %5, i16 4
  store ptr addrspace(1) %70, ptr addrspace(5) %73
  %74 = add nuw nsw i16 %71, %68
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str2, i16 6))
  call cc 1005 addrspace(1) void @N$PV(ptr addrspace(1) %6)
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str3, i16 6))
  call cc 1005 addrspace(1) void @N$PN()
  %75 = add nuw nsw i16 %74, 4
  call cc 1005 addrspace(1) void @N$PU2(i16 4)
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str4, i16 6))
  call cc 1005 addrspace(1) void @N$PU2(i16 %75)
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str5, i16 6))
  call cc 1005 addrspace(1) void @N$PV(ptr addrspace(1) %69)
  call cc 1005 addrspace(1) void @N$PN()
  call cc 1005 addrspace(1) void @N$BDRP(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6))
  ret i16 0
}

declare cc 1005 void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PV(ptr addrspace(1)) addrspace(1)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PU2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$BDRP(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
