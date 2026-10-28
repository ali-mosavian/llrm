target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"
@$str2 = internal constant [14 x i8] c"\08\00\07\00\07\00closed \00"
@$str3 = internal constant [14 x i8] c"\08\00\07\00\07\00 after \00"
@$str4 = internal constant [13 x i8] c"\08\00\06\00\06\00 steps\00"
@$str5 = internal constant [14 x i8] c"\08\00\07\00\07\00printer\00"
@$str6 = internal constant [13 x i8] c"\08\00\06\00\06\00unplug\00"
@$str7 = internal constant [13 x i8] c"\08\00\06\00\06\00replug\00"
@$str8 = internal constant [12 x i8] c"\08\00\05\00\05\00login\00"
@$str9 = internal constant [12 x i8] c"\08\00\05\00\05\00reset\00"
@$str10 = internal constant [12 x i8] c"\08\00\05\00\05\00retry\00"
@$str11 = internal constant [11 x i8] c"\08\00\04\00\04\00wifi\00"
@$str12 = internal constant [13 x i8] c"\08\00\06\00\06\00reboot\00"
@$str13 = internal constant [15 x i8] c"\08\00\08\00\08\00call isp\00"
@$str14 = internal constant [22 x i8] c"\08\00\0F\00\0F\00no login ticket\00"
@$str15 = internal constant [12 x i8] c"\08\00\05\00\05\00email\00"
@$str16 = internal constant [22 x i8] c"\08\00\0F\00\0F\00no email ticket\00"
@$str17 = internal constant [17 x i8] c"\08\00\0A\00\0A\00 waiting, \00"
@$str18 = internal constant [14 x i8] c"\08\00\07\00\07\00 closed\00"
@$str19 = internal constant [9 x i8] c"\08\00\02\00\02\00  \00"

define internal cc 1005 void @Desk.claim(ptr addrspace(5) nocapture writeonly %0, ptr addrspace(5) nocapture %1, ptr addrspace(5) readonly %2) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %3 = addrspacecast ptr addrspace(5) %2 to ptr addrspace(1)
  %4 = alloca [8 x i8]
  %5 = load ptr, ptr addrspace(5) %1
  %6 = getelementptr i8, ptr %5, i16 -4
  %7 = load i16, ptr %6
  %8 = icmp eq i16 %7, 0
  br i1 %8, label %b4, label %9

9:
  %10 = addrspacecast ptr %4 to ptr addrspace(1)
  br label %b3

b3:
  %11 = phi ptr [ %5, %9 ], [ %67, %b15 ]
  %12 = phi i16 [ undef, %9 ], [ %66, %b15 ]
  %13 = phi i16 [ undef, %9 ], [ %65, %b15 ]
  %14 = phi i8 [ 1, %9 ], [ %64, %b15 ]
  %15 = phi ptr [ getelementptr inbounds nuw (i8, ptr @$str1, i16 6), %9 ], [ %63, %b15 ]
  %16 = call cc 1005 addrspace(1) i16 @N$BSHR(ptr nonnull %11, i16 1)
  %17 = shl i16 %16, 2
  %18 = getelementptr inbounds i8, ptr %11, i16 %17
  %19 = load ptr, ptr %18
  %20 = getelementptr i8, ptr %18, i16 2
  %21 = load ptr, ptr %20
  %22 = getelementptr i8, ptr %19, i16 -4
  %23 = load i16, ptr %22
  %24 = addrspacecast ptr %19 to ptr addrspace(1)
  store i16 %23, ptr %4, !tbaa !2
  %25 = getelementptr i8, ptr %4, i16 2
  store i16 %23, ptr %25, !tbaa !2
  %26 = getelementptr i8, ptr %4, i16 4
  store ptr addrspace(1) %24, ptr %26, !tbaa !2
  %27 = call cc 1005 addrspace(1) i8 @N$VCMP(ptr addrspace(1) %10, ptr addrspace(1) %3)
  %28 = icmp eq i8 %27, 0
  br i1 %28, label %b5, label %b6

29:
  %30 = zext nneg i8 %64 to i16
  br label %b4

b4:
  %31 = phi i16 [ %66, %29 ], [ undef, %b1 ]
  %32 = phi i16 [ %65, %29 ], [ undef, %b1 ]
  %33 = phi i16 [ %30, %29 ], [ 1, %b1 ]
  %34 = phi ptr [ %63, %29 ], [ getelementptr inbounds nuw (i8, ptr @$str1, i16 6), %b1 ]
  %35 = getelementptr i8, ptr %34, i16 -4
  %36 = load i16, ptr %35
  %37 = icmp eq i16 %36, 0
  br i1 %37, label %b24, label %38

38:
  br label %b21

b5:
  %39 = icmp eq i8 %14, 0
  br i1 %39, label %b9, label %b8

b6:
  %40 = getelementptr i8, ptr %15, i16 -4
  %41 = load i16, ptr %40
  %42 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %15, i16 1, i16 4)
  %43 = shl i16 %41, 2
  %44 = getelementptr inbounds i8, ptr %42, i16 %43
  store ptr %19, ptr %44
  %45 = getelementptr i8, ptr %44, i16 2
  store ptr %21, ptr %45
  br label %b15

b8:
  %46 = ptrtoint ptr %19 to i16
  %47 = ptrtoint ptr %21 to i16
  br label %b15

b9:
  %48 = inttoptr i16 %13 to ptr
  %49 = icmp eq ptr %48, null
  br i1 %49, label %b10, label %b11

b10:
  %50 = inttoptr i16 %13 to ptr
  call cc 1005 addrspace(1) void @N$BDRP(ptr %50)
  %51 = inttoptr i16 %12 to ptr
  call cc 1005 addrspace(1) void @N$BDRP(ptr %51)
  br label %b8

b11:
  %52 = inttoptr i16 %13 to ptr
  %53 = getelementptr i8, ptr %52, i16 -4
  %54 = load i16, ptr %53
  %55 = icmp eq i16 %54, 0
  br i1 %55, label %b10, label %56

56:
  %57 = inttoptr i16 %13 to ptr
  br label %b14

b14:
  %58 = phi ptr [ %57, %56 ], [ %62, %b14 ]
  %59 = phi i16 [ %54, %56 ], [ %61, %b14 ]
  %60 = load ptr, ptr %58
  call cc 1005 addrspace(1) void @N$BDRP(ptr %60)
  %61 = add i16 %59, -1
  %62 = getelementptr i8, ptr %58, i16 2
  %exitcond2 = icmp eq i16 %61, 0
  br i1 %exitcond2, label %b10, label %b14

b15:
  %63 = phi ptr [ %15, %b8 ], [ %42, %b6 ]
  %64 = phi i8 [ 0, %b8 ], [ %14, %b6 ]
  %65 = phi i16 [ %47, %b8 ], [ %13, %b6 ]
  %66 = phi i16 [ %46, %b8 ], [ %12, %b6 ]
  call cc 1005 addrspace(1) void @N$BDRP(ptr null)
  call cc 1005 addrspace(1) void @N$BDRP(ptr null)
  %67 = load ptr, ptr addrspace(5) %1
  %68 = getelementptr i8, ptr %67, i16 -4
  %69 = load i16, ptr %68
  %70 = icmp eq i16 %69, 0
  br i1 %70, label %29, label %b3

b21:
  %71 = load ptr, ptr addrspace(5) %1
  %72 = getelementptr i8, ptr %71, i16 -4
  %73 = load i16, ptr %72
  %74 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %71, i16 1, i16 4)
  store ptr %74, ptr addrspace(5) %1
  %75 = shl i16 %73, 2
  %76 = getelementptr inbounds i8, ptr %74, i16 %75
  %77 = call cc 1005 addrspace(1) i16 @N$BSHR(ptr nonnull %34, i16 1)
  %78 = shl i16 %77, 2
  %79 = getelementptr inbounds i8, ptr %34, i16 %78
  %80 = load ptr, ptr %79
  %81 = getelementptr i8, ptr %79, i16 2
  %82 = load ptr, ptr %81
  store ptr %80, ptr %76
  %83 = getelementptr i8, ptr %76, i16 2
  store ptr %82, ptr %83
  %84 = getelementptr i8, ptr %34, i16 -4
  %85 = load i16, ptr %84
  %86 = icmp eq i16 %85, 0
  br i1 %86, label %b24, label %b21

b23:
  call cc 1005 addrspace(1) void @N$BDRP(ptr nonnull %34)
  call cc 1005 addrspace(1) void @N$BDRP(ptr null)
  call cc 1005 addrspace(1) void @N$BDRP(ptr null)
  ret void

b24:
  store i16 %33, ptr addrspace(5) %0, align 1
  %87 = getelementptr inbounds nuw i8, ptr addrspace(5) %0, i16 2
  store i16 %31, ptr addrspace(5) %87, align 1
  %88 = getelementptr inbounds nuw i8, ptr addrspace(5) %0, i16 4
  store i16 %32, ptr addrspace(5) %88, align 1
  %89 = getelementptr i8, ptr %34, i16 -4
  %90 = load i16, ptr %89
  %91 = icmp eq i16 %90, 0
  br i1 %91, label %b23, label %92

92:
  br label %b27

b27:
  %93 = phi i16 [ %101, %b28 ], [ 0, %92 ]
  %94 = shl i16 %93, 2
  %95 = getelementptr inbounds i8, ptr %34, i16 %94
  %96 = getelementptr i8, ptr %95, i16 2
  %97 = load ptr, ptr %96
  %98 = icmp eq ptr %97, null
  br i1 %98, label %b28, label %b29

b28:
  call cc 1005 addrspace(1) void @N$BDRP(ptr %97)
  %99 = getelementptr i8, ptr %34, i16 %94
  %100 = load ptr, ptr %99
  call cc 1005 addrspace(1) void @N$BDRP(ptr %100)
  %101 = add nuw i16 %93, 1
  %exitcond1 = icmp eq i16 %101, %90
  br i1 %exitcond1, label %b23, label %b27

b29:
  %102 = getelementptr i8, ptr %97, i16 -4
  %103 = load i16, ptr %102
  %104 = icmp eq i16 %103, 0
  br i1 %104, label %b28, label %105

105:
  br label %b32

b32:
  %106 = phi ptr [ %97, %105 ], [ %110, %b32 ]
  %107 = phi i16 [ %103, %105 ], [ %109, %b32 ]
  %108 = load ptr, ptr %106
  call cc 1005 addrspace(1) void @N$BDRP(ptr %108)
  %109 = add i16 %107, -1
  %110 = getelementptr i8, ptr %106, i16 2
  %exitcond = icmp eq i16 %109, 0
  br i1 %exitcond, label %b28, label %b32
}

define internal cc 1005 void @Desk.close(ptr addrspace(5) nocapture %0, ptr addrspace(5) nocapture %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str2, i16 6))
  %2 = load ptr, ptr addrspace(5) %1
  tail call cc 1005 addrspace(1) void @N$PS(ptr %2)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str3, i16 6))
  %3 = getelementptr i8, ptr addrspace(5) %1, i16 2
  %4 = load ptr, ptr addrspace(5) %3
  %5 = getelementptr i8, ptr %4, i16 -4
  %6 = load i16, ptr %5
  tail call cc 1005 addrspace(1) void @N$PU2(i16 %6)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str4, i16 6))
  tail call cc 1005 addrspace(1) void @N$PN()
  %7 = getelementptr inbounds nuw i8, ptr addrspace(5) %0, i16 2
  %8 = load ptr, ptr addrspace(5) %7
  %9 = getelementptr i8, ptr %8, i16 -4
  %10 = load i16, ptr %9
  %11 = tail call cc 1005 addrspace(1) ptr @N$BGRW(ptr %8, i16 1, i16 4)
  store ptr %11, ptr addrspace(5) %7
  %12 = shl i16 %10, 2
  %13 = getelementptr inbounds i8, ptr %11, i16 %12
  store ptr %2, ptr %13
  %14 = getelementptr i8, ptr %13, i16 2
  store ptr %4, ptr %14
  store ptr null, ptr addrspace(5) %1
  store ptr null, ptr addrspace(5) %3
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr null)
  %15 = load ptr, ptr addrspace(5) %1
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr %15)
  ret void
}

define cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
  %1 = alloca [4 x i8]
  %2 = alloca [4 x i8]
  %3 = alloca [8 x i8]
  %4 = alloca [6 x i8]
  %5 = alloca [4 x i8]
  %6 = alloca [8 x i8]
  %7 = alloca [6 x i8]
  %8 = alloca [8 x i8]
  %9 = alloca [8 x i8]
  %10 = alloca [8 x i8]
  %11 = alloca [8 x i8]
  %12 = alloca [8 x i8]
  %13 = alloca [8 x i8]
  %14 = alloca [8 x i8]
  %15 = alloca [8 x i8]
  %16 = alloca [8 x i8]
  %17 = alloca [4 x i8]
  store ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6), ptr %17, !tbaa !2
  %18 = getelementptr inbounds nuw i8, ptr %17, i16 2
  store ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6), ptr %18, !tbaa !2
  %19 = addrspacecast ptr %17 to ptr addrspace(5)
  store i32 458759, ptr %16
  %20 = getelementptr inbounds nuw i8, ptr %16, i16 4
  store ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str5, i16 6) to ptr addrspace(1)), ptr %20, !tbaa !2
  %21 = addrspacecast ptr %16 to ptr addrspace(1)
  store i32 393222, ptr %15
  %22 = getelementptr inbounds nuw i8, ptr %15, i16 4
  store ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str6, i16 6) to ptr addrspace(1)), ptr %22, !tbaa !2
  %23 = addrspacecast ptr %15 to ptr addrspace(1)
  store i32 393222, ptr %14
  %24 = getelementptr inbounds nuw i8, ptr %14, i16 4
  store ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str7, i16 6) to ptr addrspace(1)), ptr %24, !tbaa !2
  %25 = addrspacecast ptr %14 to ptr addrspace(1)
  %26 = call cc 1005 addrspace(1) ptr @N$VCPY(ptr addrspace(1) %21)
  %27 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6), i16 1, i16 2)
  %28 = call cc 1005 addrspace(1) ptr @N$VCPY(ptr addrspace(1) %23)
  store ptr %28, ptr %27
  %29 = getelementptr i8, ptr %27, i16 -4
  %30 = load i16, ptr %29
  %31 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %27, i16 1, i16 2)
  %32 = shl i16 %30, 1
  %33 = getelementptr inbounds i8, ptr %31, i16 %32
  %34 = call cc 1005 addrspace(1) ptr @N$VCPY(ptr addrspace(1) %25)
  store ptr %34, ptr %33
  call cc 1005 addrspace(1) void @N$BDRP(ptr null)
  call cc 1005 addrspace(1) void @N$BDRP(ptr null)
  %35 = load ptr, ptr addrspace(5) %19
  %36 = getelementptr i8, ptr %35, i16 -4
  %37 = load i16, ptr %36
  %38 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %35, i16 1, i16 4)
  store ptr %38, ptr addrspace(5) %19
  %39 = shl i16 %37, 2
  %40 = getelementptr inbounds i8, ptr %38, i16 %39
  store ptr %26, ptr %40
  %41 = getelementptr i8, ptr %40, i16 2
  store ptr %31, ptr %41
  call cc 1005 addrspace(1) void @N$BDRP(ptr null)
  call cc 1005 addrspace(1) void @N$BDRP(ptr null)
  store i32 327685, ptr %13
  %42 = getelementptr inbounds nuw i8, ptr %13, i16 4
  store ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str8, i16 6) to ptr addrspace(1)), ptr %42, !tbaa !2
  %43 = addrspacecast ptr %13 to ptr addrspace(1)
  store i32 327685, ptr %12
  %44 = getelementptr inbounds nuw i8, ptr %12, i16 4
  store ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str9, i16 6) to ptr addrspace(1)), ptr %44, !tbaa !2
  %45 = addrspacecast ptr %12 to ptr addrspace(1)
  store i32 327685, ptr %11
  %46 = getelementptr inbounds nuw i8, ptr %11, i16 4
  store ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str10, i16 6) to ptr addrspace(1)), ptr %46, !tbaa !2
  %47 = addrspacecast ptr %11 to ptr addrspace(1)
  %48 = call cc 1005 addrspace(1) ptr @N$VCPY(ptr addrspace(1) %43)
  %49 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6), i16 1, i16 2)
  %50 = call cc 1005 addrspace(1) ptr @N$VCPY(ptr addrspace(1) %45)
  store ptr %50, ptr %49
  %51 = getelementptr i8, ptr %49, i16 -4
  %52 = load i16, ptr %51
  %53 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %49, i16 1, i16 2)
  %54 = shl i16 %52, 1
  %55 = getelementptr inbounds i8, ptr %53, i16 %54
  %56 = call cc 1005 addrspace(1) ptr @N$VCPY(ptr addrspace(1) %47)
  store ptr %56, ptr %55
  call cc 1005 addrspace(1) void @N$BDRP(ptr null)
  call cc 1005 addrspace(1) void @N$BDRP(ptr null)
  %57 = load ptr, ptr addrspace(5) %19
  %58 = getelementptr i8, ptr %57, i16 -4
  %59 = load i16, ptr %58
  %60 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %57, i16 1, i16 4)
  store ptr %60, ptr addrspace(5) %19
  %61 = shl i16 %59, 2
  %62 = getelementptr inbounds i8, ptr %60, i16 %61
  store ptr %48, ptr %62
  %63 = getelementptr i8, ptr %62, i16 2
  store ptr %53, ptr %63
  call cc 1005 addrspace(1) void @N$BDRP(ptr null)
  call cc 1005 addrspace(1) void @N$BDRP(ptr null)
  store i32 262148, ptr %10
  %64 = getelementptr inbounds nuw i8, ptr %10, i16 4
  store ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str11, i16 6) to ptr addrspace(1)), ptr %64, !tbaa !2
  %65 = addrspacecast ptr %10 to ptr addrspace(1)
  store i32 393222, ptr %9
  %66 = getelementptr inbounds nuw i8, ptr %9, i16 4
  store ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str12, i16 6) to ptr addrspace(1)), ptr %66, !tbaa !2
  %67 = addrspacecast ptr %9 to ptr addrspace(1)
  store i32 524296, ptr %8
  %68 = getelementptr inbounds nuw i8, ptr %8, i16 4
  store ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str13, i16 6) to ptr addrspace(1)), ptr %68, !tbaa !2
  %69 = addrspacecast ptr %8 to ptr addrspace(1)
  %70 = call cc 1005 addrspace(1) ptr @N$VCPY(ptr addrspace(1) %65)
  %71 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6), i16 1, i16 2)
  %72 = call cc 1005 addrspace(1) ptr @N$VCPY(ptr addrspace(1) %67)
  store ptr %72, ptr %71
  %73 = getelementptr i8, ptr %71, i16 -4
  %74 = load i16, ptr %73
  %75 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %71, i16 1, i16 2)
  %76 = shl i16 %74, 1
  %77 = getelementptr inbounds i8, ptr %75, i16 %76
  %78 = call cc 1005 addrspace(1) ptr @N$VCPY(ptr addrspace(1) %69)
  store ptr %78, ptr %77
  call cc 1005 addrspace(1) void @N$BDRP(ptr null)
  call cc 1005 addrspace(1) void @N$BDRP(ptr null)
  %79 = load ptr, ptr addrspace(5) %19
  %80 = getelementptr i8, ptr %79, i16 -4
  %81 = load i16, ptr %80
  %82 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %79, i16 1, i16 4)
  store ptr %82, ptr addrspace(5) %19
  %83 = shl i16 %81, 2
  %84 = getelementptr inbounds i8, ptr %82, i16 %83
  store ptr %70, ptr %84
  %85 = getelementptr i8, ptr %84, i16 2
  store ptr %75, ptr %85
  call cc 1005 addrspace(1) void @N$BDRP(ptr null)
  call cc 1005 addrspace(1) void @N$BDRP(ptr null)
  %86 = addrspacecast ptr %7 to ptr addrspace(5)
  store i32 327685, ptr %6
  %87 = getelementptr inbounds nuw i8, ptr %6, i16 4
  store ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str8, i16 6) to ptr addrspace(1)), ptr %87, !tbaa !2
  %88 = addrspacecast ptr %6 to ptr addrspace(5)
  call cc 1005 addrspace(1) void @Desk.claim(ptr addrspace(5) %86, ptr addrspace(5) %19, ptr addrspace(5) %88)
  %89 = load i8, ptr %7, !tbaa !2, !range !6
  %90 = icmp eq i8 %89, 0
  br i1 %90, label %b4, label %b3

b2:
  %91 = addrspacecast ptr %4 to ptr addrspace(5)
  store i32 327685, ptr %3
  %92 = getelementptr inbounds nuw i8, ptr %3, i16 4
  store ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str15, i16 6) to ptr addrspace(1)), ptr %92, !tbaa !2
  %93 = addrspacecast ptr %3 to ptr addrspace(5)
  call cc 1005 addrspace(1) void @Desk.claim(ptr addrspace(5) %91, ptr addrspace(5) %19, ptr addrspace(5) %93)
  %94 = load i8, ptr %4, !tbaa !2, !range !6
  %95 = icmp eq i8 %94, 0
  br i1 %95, label %b20, label %b19

b3:
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str14, i16 6))
  call cc 1005 addrspace(1) void @N$PN()
  br label %b2

b4:
  %96 = getelementptr inbounds nuw i8, ptr %7, i16 2
  %97 = load ptr, ptr %96, !tbaa !2
  %98 = getelementptr inbounds nuw i8, ptr %7, i16 4
  %99 = load ptr, ptr %98, !tbaa !2
  store ptr %97, ptr %5, !tbaa !2
  %100 = getelementptr inbounds nuw i8, ptr %5, i16 2
  store ptr %99, ptr %100, !tbaa !2
  %101 = addrspacecast ptr %5 to ptr addrspace(5)
  call cc 1005 addrspace(1) void @Desk.close(ptr addrspace(5) %19, ptr addrspace(5) %101)
  call cc 1005 addrspace(1) void @N$BDRP(ptr null)
  call cc 1005 addrspace(1) void @N$BDRP(ptr null)
  br label %b2

b18:
  %102 = load ptr, ptr addrspace(5) %19
  %103 = getelementptr i8, ptr %102, i16 -4
  %104 = load i16, ptr %103
  %105 = icmp eq i16 %104, 0
  br i1 %105, label %b38, label %106

106:
  %107 = addrspacecast ptr %1 to ptr addrspace(5)
  br label %b39

b19:
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str16, i16 6))
  call cc 1005 addrspace(1) void @N$PN()
  br label %b18

b20:
  %108 = getelementptr inbounds nuw i8, ptr %4, i16 2
  %109 = load ptr, ptr %108, !tbaa !2
  %110 = getelementptr inbounds nuw i8, ptr %4, i16 4
  %111 = load ptr, ptr %110, !tbaa !2
  store ptr %109, ptr %2, !tbaa !2
  %112 = getelementptr inbounds nuw i8, ptr %2, i16 2
  store ptr %111, ptr %112, !tbaa !2
  %113 = addrspacecast ptr %2 to ptr addrspace(5)
  call cc 1005 addrspace(1) void @Desk.close(ptr addrspace(5) %19, ptr addrspace(5) %113)
  call cc 1005 addrspace(1) void @N$BDRP(ptr null)
  call cc 1005 addrspace(1) void @N$BDRP(ptr null)
  br label %b18

b38:
  %114 = phi ptr [ %102, %b18 ], [ %130, %b39 ]
  call cc 1005 addrspace(1) void @N$PU2(i16 0)
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str17, i16 6))
  %115 = getelementptr i8, ptr %17, i16 2
  %116 = load ptr, ptr %115, !tbaa !2
  %117 = getelementptr i8, ptr %116, i16 -4
  %118 = load i16, ptr %117
  call cc 1005 addrspace(1) void @N$PU2(i16 %118)
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str18, i16 6))
  call cc 1005 addrspace(1) void @N$PN()
  %119 = load i16, ptr %117
  %120 = icmp eq i16 %119, 0
  br i1 %120, label %b57.thread, label %121

121:
  br label %b54

b39:
  %122 = phi ptr [ %102, %106 ], [ %130, %b39 ]
  %123 = call cc 1005 addrspace(1) i16 @N$BSHR(ptr nonnull %122, i16 1)
  %124 = shl i16 %123, 2
  %125 = getelementptr inbounds i8, ptr %122, i16 %124
  %126 = load ptr, ptr %125
  %127 = getelementptr i8, ptr %125, i16 2
  %128 = load ptr, ptr %127
  store ptr %126, ptr %1, !tbaa !2
  %129 = getelementptr i8, ptr %1, i16 2
  store ptr %128, ptr %129, !tbaa !2
  call cc 1005 addrspace(1) void @Desk.close(ptr addrspace(5) %19, ptr addrspace(5) %107)
  call cc 1005 addrspace(1) void @N$BDRP(ptr null)
  call cc 1005 addrspace(1) void @N$BDRP(ptr null)
  %130 = load ptr, ptr addrspace(5) %19
  %131 = getelementptr i8, ptr %130, i16 -4
  %132 = load i16, ptr %131
  %133 = icmp eq i16 %132, 0
  br i1 %133, label %b38, label %b39

b54:
  %134 = phi i16 [ 0, %121 ], [ %139, %b54 ]
  %135 = phi i16 [ %119, %121 ], [ %138, %b54 ]
  %136 = getelementptr i8, ptr %116, i16 %134
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str19, i16 6))
  %137 = load ptr, ptr %136
  call cc 1005 addrspace(1) void @N$PS(ptr %137)
  call cc 1005 addrspace(1) void @N$PN()
  %138 = add i16 %135, -1
  %139 = add i16 %134, 4
  %exitcond4 = icmp eq i16 %138, 0
  br i1 %exitcond4, label %b58, label %b54

b57:
  call cc 1005 addrspace(1) void @N$BDRP(ptr nonnull %116)
  %140 = icmp eq ptr %114, null
  br i1 %140, label %b67, label %b68

b58:
  %141 = getelementptr i8, ptr %116, i16 -4
  %142 = load i16, ptr %141
  %143 = icmp eq i16 %142, 0
  br i1 %143, label %b57.thread, label %144

144:
  br label %b61

b57.thread:
  call cc 1005 addrspace(1) void @N$BDRP(ptr nonnull %116)
  br label %b68

b61:
  %145 = phi i16 [ %153, %b62 ], [ 0, %144 ]
  %146 = shl i16 %145, 2
  %147 = getelementptr inbounds i8, ptr %116, i16 %146
  %148 = getelementptr i8, ptr %147, i16 2
  %149 = load ptr, ptr %148
  %150 = icmp eq ptr %149, null
  br i1 %150, label %b62, label %b63

b62:
  call cc 1005 addrspace(1) void @N$BDRP(ptr %149)
  %151 = getelementptr i8, ptr %116, i16 %146
  %152 = load ptr, ptr %151
  call cc 1005 addrspace(1) void @N$BDRP(ptr %152)
  %153 = add nuw i16 %145, 1
  %exitcond3 = icmp eq i16 %153, %142
  br i1 %exitcond3, label %b57, label %b61

b63:
  %154 = getelementptr i8, ptr %149, i16 -4
  %155 = load i16, ptr %154
  %156 = icmp eq i16 %155, 0
  br i1 %156, label %b62, label %157

157:
  br label %b66

b66:
  %158 = phi ptr [ %149, %157 ], [ %162, %b66 ]
  %159 = phi i16 [ %155, %157 ], [ %161, %b66 ]
  %160 = load ptr, ptr %158
  call cc 1005 addrspace(1) void @N$BDRP(ptr %160)
  %161 = add i16 %159, -1
  %162 = getelementptr i8, ptr %158, i16 2
  %exitcond2 = icmp eq i16 %161, 0
  br i1 %exitcond2, label %b62, label %b66

b67:
  call cc 1005 addrspace(1) void @N$BDRP(ptr %114)
  ret i16 0

b68:
  %163 = getelementptr i8, ptr %114, i16 -4
  %164 = load i16, ptr %163
  %165 = icmp eq i16 %164, 0
  br i1 %165, label %b67, label %166

166:
  br label %b71

b71:
  %167 = phi i16 [ %175, %b72 ], [ 0, %166 ]
  %168 = shl i16 %167, 2
  %169 = getelementptr inbounds i8, ptr %114, i16 %168
  %170 = getelementptr i8, ptr %169, i16 2
  %171 = load ptr, ptr %170
  %172 = icmp eq ptr %171, null
  br i1 %172, label %b72, label %b73

b72:
  call cc 1005 addrspace(1) void @N$BDRP(ptr %171)
  %173 = getelementptr i8, ptr %114, i16 %168
  %174 = load ptr, ptr %173
  call cc 1005 addrspace(1) void @N$BDRP(ptr %174)
  %175 = add nuw i16 %167, 1
  %exitcond1 = icmp eq i16 %175, %164
  br i1 %exitcond1, label %b67, label %b71

b73:
  %176 = getelementptr i8, ptr %171, i16 -4
  %177 = load i16, ptr %176
  %178 = icmp eq i16 %177, 0
  br i1 %178, label %b72, label %179

179:
  br label %b76

b76:
  %180 = phi ptr [ %171, %179 ], [ %184, %b76 ]
  %181 = phi i16 [ %177, %179 ], [ %183, %b76 ]
  %182 = load ptr, ptr %180
  call cc 1005 addrspace(1) void @N$BDRP(ptr %182)
  %183 = add i16 %181, -1
  %184 = getelementptr i8, ptr %180, i16 2
  %exitcond = icmp eq i16 %183, 0
  br i1 %exitcond, label %b72, label %b76
}

declare cc 1005 ptr @N$VCPY(ptr addrspace(1)) addrspace(1)

declare cc 1005 ptr @N$BGRW(ptr, i16, i16) addrspace(1)

declare cc 1005 void @N$BDRP(ptr) addrspace(1)

declare cc 1005 i16 @N$BSHR(ptr, i16) addrspace(1)

declare void @llvm.memcpy.p1.p0.i16(ptr addrspace(1) noalias nocapture writeonly, ptr noalias nocapture readonly, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)

declare cc 1005 i8 @N$VCMP(ptr addrspace(1), ptr addrspace(1)) addrspace(1) memory(read)

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PU2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{i8 0, i8 2}
!6 = !{i8 0, i8 2}
