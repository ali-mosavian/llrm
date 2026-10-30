target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [13 x i8] c"\08\00\06\00\06\00nobody\00"
@$str2 = internal constant [10 x i8] c"\08\00\03\00\03\00ada\00"
@$str3 = internal constant [10 x i8] c"\08\00\03\00\03\00bob\00"
@$str4 = internal constant [14 x i8] c"\08\00\07\00\07\00leader \00"
@$str5 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"
@$str6 = internal constant [9 x i8] c"\08\00\02\00\02\00cy\00"
@$str7 = internal constant [9 x i8] c"\08\00\02\00\02\00di\00"
@$str8 = internal constant [9 x i8] c"\08\00\02\00\02\00ed\00"
@$str9 = internal constant [12 x i8] c"\08\00\05\00\05\00best \00"
@$str10 = internal constant [13 x i8] c"\08\00\06\00\06\00 with \00"
@$str11 = internal constant [14 x i8] c"\08\00\07\00\07\00no team\00"
@$str12 = internal constant [13 x i8] c"\08\00\06\00\06\00first \00"
@$str13 = internal constant [27 x i8] c"\08\00\14\00\14\00move north then east\00"
@$str14 = internal constant [8 x i8] c"\08\00\01\00\01\00[\00"
@$str15 = internal constant [8 x i8] c"\08\00\01\00\01\00]\00"

define cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
  %1 = alloca [8 x i8]
  %2 = alloca [8 x i8]
  %3 = alloca [8 x i8]
  store i32 196611, ptr %3
  %4 = getelementptr inbounds nuw i8, ptr %3, i16 4
  store ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str3, i16 6) to ptr addrspace(1)), ptr %4, !tbaa !2
  %5 = addrspacecast ptr %3 to ptr addrspace(1)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str4, i16 6))
  call cc 1005 addrspace(1) void @N$PV(ptr addrspace(1) %5)
  call cc 1005 addrspace(1) void @N$PN()
  %6 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr getelementptr inbounds nuw (i8, ptr @$str5, i16 6), i16 3, i16 4)
  store ptr getelementptr inbounds nuw (i8, ptr @$str6, i16 6), ptr %6
  %7 = getelementptr i8, ptr %6, i16 2
  store i16 12, ptr %7
  %8 = getelementptr inbounds nuw i8, ptr %6, i16 4
  store ptr getelementptr inbounds nuw (i8, ptr @$str7, i16 6), ptr %8
  %9 = getelementptr i8, ptr %6, i16 6
  store i16 58, ptr %9
  %10 = getelementptr inbounds nuw i8, ptr %6, i16 8
  store ptr getelementptr inbounds nuw (i8, ptr @$str8, i16 6), ptr %10
  %11 = getelementptr i8, ptr %6, i16 10
  store i16 40, ptr %11
  %12 = getelementptr i8, ptr %6, i16 -4
  %13 = load i16, ptr %12
  %14 = icmp eq i16 %13, 0
  br i1 %14, label %b3, label %15

15:
  %16 = icmp eq i16 %13, 1
  br i1 %16, label %17, label %20

17:
  %18 = phi i16 [ 0, %15 ], [ %34, %24 ]
  %19 = icmp ult i16 %18, %13
  br i1 %19, label %b4, label %37

20:
  %21 = phi i16 [ %34, %24 ], [ 0, %15 ]
  %22 = phi i16 [ %35, %24 ], [ 1, %15 ]
  %23 = icmp ult i16 %21, %13
  br i1 %23, label %24, label %36

24:
  %25 = shl i16 %22, 2
  %26 = getelementptr inbounds i8, ptr %6, i16 %25
  %27 = getelementptr i8, ptr %26, i16 2
  %28 = load i16, ptr %27
  %29 = shl i16 %21, 2
  %30 = getelementptr inbounds i8, ptr %6, i16 %29
  %31 = getelementptr i8, ptr %30, i16 2
  %32 = load i16, ptr %31
  %33 = icmp sgt i16 %28, %32
  %34 = select i1 %33, i16 %22, i16 %21
  %35 = add nuw i16 %22, 1
  %exitcond2 = icmp eq i16 %35, %13
  br i1 %exitcond2, label %17, label %20

36:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

37:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

38:
  call cc 1005 addrspace(1) void @N$PN()
  %39 = addrspacecast ptr %2 to ptr addrspace(5)
  %40 = addrspacecast ptr %2 to ptr addrspace(1)
  %41 = getelementptr i8, ptr %6, i16 -4
  %42 = load i16, ptr %41
  %43 = icmp sgt i16 %42, 0
  br i1 %43, label %44, label %49

44:
  %45 = load ptr, ptr %6
  %46 = getelementptr i8, ptr %45, i16 -4
  %47 = load i16, ptr %46
  %48 = addrspacecast ptr %45 to ptr addrspace(1)
  br label %49

49:
  %sink4 = phi i16 [ %47, %44 ], [ 6, %38 ]
  %sink = phi ptr addrspace(1) [ %48, %44 ], [ addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6) to ptr addrspace(1)), %38 ]
  store i16 %sink4, ptr addrspace(5) %39
  %50 = getelementptr inbounds nuw i8, ptr addrspace(5) %39, i16 2
  store i16 %sink4, ptr addrspace(5) %50
  %51 = getelementptr inbounds nuw i8, ptr addrspace(5) %39, i16 4
  store ptr addrspace(1) %sink, ptr addrspace(5) %51
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str12, i16 6))
  call cc 1005 addrspace(1) void @N$PV(ptr addrspace(1) %40)
  call cc 1005 addrspace(1) void @N$PN()
  %52 = addrspacecast ptr %1 to ptr addrspace(5)
  %53 = addrspacecast ptr %1 to ptr addrspace(1)
  store i32 262148, ptr addrspace(5) %52
  %54 = getelementptr inbounds nuw i8, ptr addrspace(5) %52, i16 4
  store ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str13, i16 6) to ptr addrspace(1)), ptr addrspace(5) %54, !tbaa !2
  br label %b7

b3:
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str11, i16 6))
  br label %38

b4:
  %55 = shl nuw i16 %18, 2
  %56 = getelementptr inbounds i8, ptr %6, i16 %55
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str9, i16 6))
  %57 = load ptr, ptr %56
  call cc 1005 addrspace(1) void @N$PS(ptr %57)
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str10, i16 6))
  %58 = getelementptr i8, ptr %56, i16 2
  %59 = load i16, ptr %58
  call cc 1005 addrspace(1) void @N$PI2(i16 %59)
  br label %38

b7:
  %60 = phi i16 [ 0, %49 ], [ %63, %79 ]
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str14, i16 6))
  call cc 1005 addrspace(1) void @N$PV(ptr addrspace(1) %53)
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str15, i16 6))
  call cc 1005 addrspace(1) void @N$PN()
  %61 = load i16, ptr addrspace(5) %52
  %62 = add nuw nsw i16 %60, 1
  %63 = add i16 %62, %61
  %64 = icmp ugt i16 %63, 19
  br i1 %64, label %b8, label %65

65:
  %66 = add i16 %60, -19
  %67 = add i16 %66, %61
  br label %71

b8:
  call cc 1005 addrspace(1) void @N$BDRP(ptr getelementptr inbounds nuw (i8, ptr @$str13, i16 6))
  %68 = icmp eq ptr %6, null
  br i1 %68, label %b12, label %b13

69:
  %70 = add i16 %72, 1
  %exitcond = icmp eq i16 %70, 0
  br i1 %exitcond, label %79, label %71

71:
  %72 = phi i16 [ %67, %65 ], [ %70, %69 ]
  %73 = getelementptr i8, ptr @$str13, i16 %72
  %74 = getelementptr i8, ptr %73, i16 26
  %75 = load i8, ptr %74
  %76 = icmp eq i8 %75, 32
  br i1 %76, label %77, label %69

77:
  %78 = add i16 %72, 20
  br label %79

79:
  %80 = phi i16 [ %78, %77 ], [ 20, %69 ]
  %81 = getelementptr inbounds nuw i8, ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str13, i16 6) to ptr addrspace(1)), i16 %63
  %82 = sub nuw nsw i16 %80, %63
  store i16 %82, ptr addrspace(5) %52, !tbaa !2
  %83 = getelementptr i8, ptr addrspace(5) %52, i16 2
  store i16 %82, ptr addrspace(5) %83, !tbaa !2
  %84 = getelementptr i8, ptr addrspace(5) %52, i16 4
  store ptr addrspace(1) %81, ptr addrspace(5) %84, !tbaa !2
  %85 = icmp eq i16 %82, 0
  br i1 %85, label %b8, label %b7

b12:
  call cc 1005 addrspace(1) void @N$BDRP(ptr %6)
  call cc 1005 addrspace(1) void @N$BDRP(ptr getelementptr inbounds nuw (i8, ptr @$str3, i16 6))
  call cc 1005 addrspace(1) void @N$BDRP(ptr getelementptr inbounds nuw (i8, ptr @$str2, i16 6))
  ret i16 0

b13:
  %86 = getelementptr i8, ptr %6, i16 -4
  %87 = load i16, ptr %86
  %88 = icmp eq i16 %87, 0
  br i1 %88, label %b12, label %89

89:
  br label %b16

b16:
  %90 = phi ptr [ %6, %89 ], [ %94, %b16 ]
  %91 = phi i16 [ %87, %89 ], [ %93, %b16 ]
  %92 = load ptr, ptr %90
  call cc 1005 addrspace(1) void @N$BDRP(ptr %92)
  %93 = add i16 %91, -1
  %94 = getelementptr i8, ptr %90, i16 4
  %exitcond3 = icmp eq i16 %93, 0
  br i1 %exitcond3, label %b12, label %b16
}

declare cc 1005 void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PV(ptr addrspace(1)) addrspace(1)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 ptr @N$BGRW(ptr, i16, i16) addrspace(1)

declare cc 1005 void @N$PI2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$BDRP(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{i16 0, i16 16384}
!6 = !{i16 0, i16 16384}
!7 = !{i16 0, i16 16384}
!8 = !{i8 0, i8 2}
