target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16-n8:16:32"

@_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a4 = global [600 x i8] zeroinitializer
@_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a5 = global [1200 x i8] zeroinitializer
@_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a6 = global [600 x i8] zeroinitializer
@_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a0 = global [600 x i8] zeroinitializer
@_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a1 = global [1200 x i8] zeroinitializer
@_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a2 = global [600 x i8] zeroinitializer
@_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a3 = global [1200 x i8] zeroinitializer

define i32 @_f_conc7_s24_xi_bln_index_n_st1_sum_as_end(i16 %0, i16 %1, i16 %2) addrspace(1) memory(readwrite, argmem: none) {
b1:
  %3 = alloca [600 x i8]
  %4 = alloca [1200 x i8]
  %5 = alloca [600 x i8]
  %6 = alloca [1200 x i8]
  %7 = alloca [600 x i8]
  %8 = alloca [1200 x i8]
  %9 = alloca [600 x i8]
  %10 = addrspacecast ptr @_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a0 to ptr addrspace(1)
  %11 = addrspacecast ptr %3 to ptr addrspace(1)
  %12 = call addrspace(1) i16 @_lcopy(ptr addrspace(1) %11, ptr addrspace(1) %10, i16 600)
  %13 = addrspacecast ptr @_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a1 to ptr addrspace(1)
  %14 = addrspacecast ptr %4 to ptr addrspace(1)
  %15 = call addrspace(1) i16 @_lcopy(ptr addrspace(1) %14, ptr addrspace(1) %13, i16 1200)
  %16 = addrspacecast ptr @_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a2 to ptr addrspace(1)
  %17 = addrspacecast ptr %5 to ptr addrspace(1)
  %18 = call addrspace(1) i16 @_lcopy(ptr addrspace(1) %17, ptr addrspace(1) %16, i16 600)
  %19 = addrspacecast ptr @_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a3 to ptr addrspace(1)
  %20 = addrspacecast ptr %6 to ptr addrspace(1)
  %21 = call addrspace(1) i16 @_lcopy(ptr addrspace(1) %20, ptr addrspace(1) %19, i16 1200)
  %22 = addrspacecast ptr @_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a4 to ptr addrspace(1)
  %23 = addrspacecast ptr %7 to ptr addrspace(1)
  %24 = call addrspace(1) i16 @_lcopy(ptr addrspace(1) %23, ptr addrspace(1) %22, i16 600)
  %25 = addrspacecast ptr @_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a5 to ptr addrspace(1)
  %26 = addrspacecast ptr %8 to ptr addrspace(1)
  %27 = call addrspace(1) i16 @_lcopy(ptr addrspace(1) %26, ptr addrspace(1) %25, i16 1200)
  %28 = addrspacecast ptr @_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a6 to ptr addrspace(1)
  %29 = addrspacecast ptr %9 to ptr addrspace(1)
  %30 = call addrspace(1) i16 @_lcopy(ptr addrspace(1) %29, ptr addrspace(1) %28, i16 600)
  %31 = add nsw i16 %0, 8
  %32 = mul nsw i16 %31, 2
  %33 = getelementptr inbounds i8, ptr %3, i16 %32
  %34 = getelementptr inbounds i8, ptr %3, i16 16
  %35 = getelementptr inbounds i8, ptr %4, i16 32
  %36 = getelementptr inbounds i8, ptr %5, i16 16
  %37 = getelementptr inbounds i8, ptr %6, i16 32
  %38 = getelementptr inbounds i8, ptr %7, i16 16
  %39 = getelementptr inbounds i8, ptr %8, i16 32
  %40 = getelementptr inbounds i8, ptr %9, i16 16
  br label %b2

b2:
  %41 = phi ptr [ %40, %b1 ], [ %76, %b4 ]
  %42 = phi ptr [ %39, %b1 ], [ %75, %b4 ]
  %43 = phi ptr [ %38, %b1 ], [ %74, %b4 ]
  %44 = phi ptr [ %37, %b1 ], [ %73, %b4 ]
  %45 = phi ptr [ %36, %b1 ], [ %72, %b4 ]
  %46 = phi ptr [ %35, %b1 ], [ %71, %b4 ]
  %47 = phi ptr [ %34, %b1 ], [ %70, %b4 ]
  %48 = phi i32 [ 0, %b1 ], [ %69, %b4 ]
  %49 = icmp ult ptr %47, %33
  br i1 %49, label %b4, label %b3

b3:
  %50 = phi i32 [ %48, %b2 ]
  %51 = add nsw i32 %50, 1
  ret i32 %51

b4:
  %52 = load i16, ptr %47, !tbaa !9
  %53 = sext i16 %52 to i32
  %54 = add nsw i32 %48, %53
  %55 = load i32, ptr %46, !tbaa !11
  %56 = add nsw i32 %54, %55
  %57 = load i16, ptr %45, !tbaa !9
  %58 = sext i16 %57 to i32
  %59 = add nsw i32 %56, %58
  %60 = load i32, ptr %44, !tbaa !11
  %61 = add nsw i32 %59, %60
  %62 = load i16, ptr %43, !tbaa !9
  %63 = sext i16 %62 to i32
  %64 = add nsw i32 %61, %63
  %65 = load i32, ptr %42, !tbaa !11
  %66 = add nsw i32 %64, %65
  %67 = load i16, ptr %41, !tbaa !9
  %68 = sext i16 %67 to i32
  %69 = add nsw i32 %66, %68
  %70 = getelementptr inbounds i8, ptr %47, i16 2
  %71 = getelementptr inbounds i8, ptr %46, i16 4
  %72 = getelementptr inbounds i8, ptr %45, i16 2
  %73 = getelementptr inbounds i8, ptr %44, i16 4
  %74 = getelementptr inbounds i8, ptr %43, i16 2
  %75 = getelementptr inbounds i8, ptr %42, i16 4
  %76 = getelementptr inbounds i8, ptr %41, i16 2
  br label %b2
}

define internal i32 @_fillx(i32 %0, i32 %1) memory(none) willreturn nounwind {
b1:
  %2 = mul i32 %0, 7919
  %3 = add i32 %2, %1
  %4 = urem i32 %3, 65521
  ret i32 %4
}

define internal void @_fill_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a0(i16 %0, i32 %1, i32 %2, i32 %3) memory(write, argmem: none, inaccessiblemem: none) {
b1:
  %4 = addrspacecast ptr @_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a0 to ptr addrspace(1)
  %5 = zext i16 %0 to i32
  br label %b4

b3:
  ret void

b4:
  %6 = phi i32 [ 0, %b1 ], [ %14, %b4 ]
  %7 = call i32 @_fillx(i32 %6, i32 %5)
  %8 = urem i32 %7, 2001
  %9 = add nsw i32 -1000, %8
  %10 = trunc i32 %9 to i16
  %11 = trunc i32 %6 to i16
  %12 = mul nsw i16 %11, 2
  %13 = getelementptr inbounds i8, ptr addrspace(1) %4, i16 %12
  store i16 %10, ptr addrspace(1) %13, !tbaa !9
  %14 = add i32 %6, 1
  %15 = icmp ult i32 %14, 300
  br i1 %15, label %b4, label %b3
}

define internal void @_fill_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a1(i16 %0, i32 %1, i32 %2, i32 %3) memory(write, argmem: none, inaccessiblemem: none) {
b1:
  %4 = addrspacecast ptr @_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a1 to ptr addrspace(1)
  %5 = zext i16 %0 to i32
  br label %b4

b3:
  ret void

b4:
  %6 = phi i32 [ 0, %b1 ], [ %13, %b4 ]
  %7 = call i32 @_fillx(i32 %6, i32 %5)
  %8 = urem i32 %7, 2001
  %9 = add nsw i32 -1000, %8
  %10 = trunc i32 %6 to i16
  %11 = mul nsw i16 %10, 4
  %12 = getelementptr inbounds i8, ptr addrspace(1) %4, i16 %11
  store i32 %9, ptr addrspace(1) %12, !tbaa !11
  %13 = add i32 %6, 1
  %14 = icmp ult i32 %13, 300
  br i1 %14, label %b4, label %b3
}

define internal void @_fill_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a2(i16 %0, i32 %1, i32 %2, i32 %3) memory(write, argmem: none, inaccessiblemem: none) {
b1:
  %4 = addrspacecast ptr @_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a2 to ptr addrspace(1)
  %5 = zext i16 %0 to i32
  br label %b4

b3:
  ret void

b4:
  %6 = phi i32 [ 0, %b1 ], [ %14, %b4 ]
  %7 = call i32 @_fillx(i32 %6, i32 %5)
  %8 = urem i32 %7, 2001
  %9 = add nsw i32 -1000, %8
  %10 = trunc i32 %9 to i16
  %11 = trunc i32 %6 to i16
  %12 = mul nsw i16 %11, 2
  %13 = getelementptr inbounds i8, ptr addrspace(1) %4, i16 %12
  store i16 %10, ptr addrspace(1) %13, !tbaa !9
  %14 = add i32 %6, 1
  %15 = icmp ult i32 %14, 300
  br i1 %15, label %b4, label %b3
}

define internal void @_fill_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a3(i16 %0, i32 %1, i32 %2, i32 %3) memory(write, argmem: none, inaccessiblemem: none) {
b1:
  %4 = addrspacecast ptr @_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a3 to ptr addrspace(1)
  %5 = zext i16 %0 to i32
  br label %b4

b3:
  ret void

b4:
  %6 = phi i32 [ 0, %b1 ], [ %13, %b4 ]
  %7 = call i32 @_fillx(i32 %6, i32 %5)
  %8 = urem i32 %7, 2001
  %9 = add nsw i32 -1000, %8
  %10 = trunc i32 %6 to i16
  %11 = mul nsw i16 %10, 4
  %12 = getelementptr inbounds i8, ptr addrspace(1) %4, i16 %11
  store i32 %9, ptr addrspace(1) %12, !tbaa !11
  %13 = add i32 %6, 1
  %14 = icmp ult i32 %13, 300
  br i1 %14, label %b4, label %b3
}

define internal void @_fill_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a4(i16 %0, i32 %1, i32 %2, i32 %3) memory(write, argmem: none, inaccessiblemem: none) {
b1:
  %4 = addrspacecast ptr @_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a4 to ptr addrspace(1)
  %5 = zext i16 %0 to i32
  br label %b4

b3:
  ret void

b4:
  %6 = phi i32 [ 0, %b1 ], [ %14, %b4 ]
  %7 = call i32 @_fillx(i32 %6, i32 %5)
  %8 = urem i32 %7, 2001
  %9 = add nsw i32 -1000, %8
  %10 = trunc i32 %9 to i16
  %11 = trunc i32 %6 to i16
  %12 = mul nsw i16 %11, 2
  %13 = getelementptr inbounds i8, ptr addrspace(1) %4, i16 %12
  store i16 %10, ptr addrspace(1) %13, !tbaa !9
  %14 = add i32 %6, 1
  %15 = icmp ult i32 %14, 300
  br i1 %15, label %b4, label %b3
}

define internal void @_fill_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a5(i16 %0, i32 %1, i32 %2, i32 %3) memory(write, argmem: none, inaccessiblemem: none) {
b1:
  %4 = addrspacecast ptr @_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a5 to ptr addrspace(1)
  %5 = zext i16 %0 to i32
  br label %b4

b3:
  ret void

b4:
  %6 = phi i32 [ 0, %b1 ], [ %13, %b4 ]
  %7 = call i32 @_fillx(i32 %6, i32 %5)
  %8 = urem i32 %7, 2001
  %9 = add nsw i32 -1000, %8
  %10 = trunc i32 %6 to i16
  %11 = mul nsw i16 %10, 4
  %12 = getelementptr inbounds i8, ptr addrspace(1) %4, i16 %11
  store i32 %9, ptr addrspace(1) %12, !tbaa !11
  %13 = add i32 %6, 1
  %14 = icmp ult i32 %13, 300
  br i1 %14, label %b4, label %b3
}

define internal void @_fill_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a6(i16 %0, i32 %1, i32 %2, i32 %3) memory(write, argmem: none, inaccessiblemem: none) {
b1:
  %4 = addrspacecast ptr @_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a6 to ptr addrspace(1)
  %5 = zext i16 %0 to i32
  br label %b4

b3:
  ret void

b4:
  %6 = phi i32 [ 0, %b1 ], [ %14, %b4 ]
  %7 = call i32 @_fillx(i32 %6, i32 %5)
  %8 = urem i32 %7, 2001
  %9 = add nsw i32 -1000, %8
  %10 = trunc i32 %9 to i16
  %11 = trunc i32 %6 to i16
  %12 = mul nsw i16 %11, 2
  %13 = getelementptr inbounds i8, ptr addrspace(1) %4, i16 %12
  store i16 %10, ptr addrspace(1) %13, !tbaa !9
  %14 = add i32 %6, 1
  %15 = icmp ult i32 %14, 300
  br i1 %15, label %b4, label %b3
}

define internal void @_run_f_conc7_s24_xi_bln_index_n_st1_sum_as_end() memory(readwrite, argmem: none) {
b1:
  call void @_fill_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a0(i16 3, i32 -1000, i32 2001, i32 0)
  call void @_fill_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a1(i16 10, i32 -1000, i32 2001, i32 0)
  call void @_fill_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a2(i16 17, i32 -1000, i32 2001, i32 0)
  call void @_fill_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a3(i16 24, i32 -1000, i32 2001, i32 0)
  call void @_fill_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a4(i16 31, i32 -1000, i32 2001, i32 0)
  call void @_fill_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a5(i16 38, i32 -1000, i32 2001, i32 0)
  call void @_fill_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a6(i16 45, i32 -1000, i32 2001, i32 0)
  %0 = call addrspace(1) i32 @_f_conc7_s24_xi_bln_index_n_st1_sum_as_end(i16 0, i16 5, i16 2)
  %1 = call addrspace(1) i16 @_report(i32 %0)
  call void @_fill_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a0(i16 34, i32 -1000, i32 2001, i32 0)
  call void @_fill_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a1(i16 41, i32 -1000, i32 2001, i32 0)
  call void @_fill_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a2(i16 48, i32 -1000, i32 2001, i32 0)
  call void @_fill_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a3(i16 55, i32 -1000, i32 2001, i32 0)
  call void @_fill_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a4(i16 62, i32 -1000, i32 2001, i32 0)
  call void @_fill_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a5(i16 69, i32 -1000, i32 2001, i32 0)
  call void @_fill_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a6(i16 76, i32 -1000, i32 2001, i32 0)
  %2 = call addrspace(1) i32 @_f_conc7_s24_xi_bln_index_n_st1_sum_as_end(i16 1, i16 5, i16 2)
  %3 = call addrspace(1) i16 @_report(i32 %2)
  call void @_fill_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a0(i16 65, i32 -1000, i32 2001, i32 0)
  call void @_fill_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a1(i16 72, i32 -1000, i32 2001, i32 0)
  call void @_fill_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a2(i16 79, i32 -1000, i32 2001, i32 0)
  call void @_fill_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a3(i16 86, i32 -1000, i32 2001, i32 0)
  call void @_fill_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a4(i16 93, i32 -1000, i32 2001, i32 0)
  call void @_fill_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a5(i16 100, i32 -1000, i32 2001, i32 0)
  call void @_fill_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a6(i16 107, i32 -1000, i32 2001, i32 0)
  %4 = call addrspace(1) i32 @_f_conc7_s24_xi_bln_index_n_st1_sum_as_end(i16 2, i16 5, i16 2)
  %5 = call addrspace(1) i16 @_report(i32 %4)
  call void @_fill_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a0(i16 96, i32 -1000, i32 2001, i32 0)
  call void @_fill_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a1(i16 103, i32 -1000, i32 2001, i32 0)
  call void @_fill_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a2(i16 110, i32 -1000, i32 2001, i32 0)
  call void @_fill_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a3(i16 117, i32 -1000, i32 2001, i32 0)
  call void @_fill_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a4(i16 124, i32 -1000, i32 2001, i32 0)
  call void @_fill_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a5(i16 131, i32 -1000, i32 2001, i32 0)
  call void @_fill_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a6(i16 138, i32 -1000, i32 2001, i32 0)
  %6 = call addrspace(1) i32 @_f_conc7_s24_xi_bln_index_n_st1_sum_as_end(i16 3, i16 5, i16 2)
  %7 = call addrspace(1) i16 @_report(i32 %6)
  call void @_fill_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a0(i16 127, i32 -1000, i32 2001, i32 0)
  call void @_fill_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a1(i16 134, i32 -1000, i32 2001, i32 0)
  call void @_fill_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a2(i16 141, i32 -1000, i32 2001, i32 0)
  call void @_fill_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a3(i16 148, i32 -1000, i32 2001, i32 0)
  call void @_fill_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a4(i16 155, i32 -1000, i32 2001, i32 0)
  call void @_fill_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a5(i16 162, i32 -1000, i32 2001, i32 0)
  call void @_fill_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a6(i16 169, i32 -1000, i32 2001, i32 0)
  %8 = call addrspace(1) i32 @_f_conc7_s24_xi_bln_index_n_st1_sum_as_end(i16 15, i16 5, i16 2)
  %9 = call addrspace(1) i16 @_report(i32 %8)
  call void @_fill_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a0(i16 158, i32 -1000, i32 2001, i32 0)
  call void @_fill_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a1(i16 165, i32 -1000, i32 2001, i32 0)
  call void @_fill_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a2(i16 172, i32 -1000, i32 2001, i32 0)
  call void @_fill_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a3(i16 179, i32 -1000, i32 2001, i32 0)
  call void @_fill_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a4(i16 186, i32 -1000, i32 2001, i32 0)
  call void @_fill_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a5(i16 193, i32 -1000, i32 2001, i32 0)
  call void @_fill_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a6(i16 200, i32 -1000, i32 2001, i32 0)
  %10 = call addrspace(1) i32 @_f_conc7_s24_xi_bln_index_n_st1_sum_as_end(i16 16, i16 5, i16 2)
  %11 = call addrspace(1) i16 @_report(i32 %10)
  call void @_fill_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a0(i16 189, i32 -1000, i32 2001, i32 0)
  call void @_fill_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a1(i16 196, i32 -1000, i32 2001, i32 0)
  call void @_fill_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a2(i16 203, i32 -1000, i32 2001, i32 0)
  call void @_fill_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a3(i16 210, i32 -1000, i32 2001, i32 0)
  call void @_fill_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a4(i16 217, i32 -1000, i32 2001, i32 0)
  call void @_fill_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a5(i16 224, i32 -1000, i32 2001, i32 0)
  call void @_fill_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a6(i16 231, i32 -1000, i32 2001, i32 0)
  %12 = call addrspace(1) i32 @_f_conc7_s24_xi_bln_index_n_st1_sum_as_end(i16 17, i16 5, i16 2)
  %13 = call addrspace(1) i16 @_report(i32 %12)
  call void @_fill_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a0(i16 220, i32 -1000, i32 2001, i32 0)
  call void @_fill_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a1(i16 227, i32 -1000, i32 2001, i32 0)
  call void @_fill_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a2(i16 234, i32 -1000, i32 2001, i32 0)
  call void @_fill_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a3(i16 241, i32 -1000, i32 2001, i32 0)
  call void @_fill_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a4(i16 248, i32 -1000, i32 2001, i32 0)
  call void @_fill_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a5(i16 255, i32 -1000, i32 2001, i32 0)
  call void @_fill_f_conc7_s24_xi_bln_index_n_st1_sum_as_end_a6(i16 262, i32 -1000, i32 2001, i32 0)
  %14 = call addrspace(1) i32 @_f_conc7_s24_xi_bln_index_n_st1_sum_as_end(i16 255, i16 5, i16 2)
  %15 = call addrspace(1) i16 @_report(i32 %14)
  ret void
}

define i16 @_main() addrspace(1) memory(readwrite, argmem: none) {
b1:
  call void @_run_f_conc7_s24_xi_bln_index_n_st1_sum_as_end()
  ret i16 0
}

declare i16 @_lcopy(ptr addrspace(1), ptr addrspace(1), i16) addrspace(1)

declare i16 @_report(i32) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{!"Simple C/C++ TBAA"}
!6 = !{!"omnipotent char", !5, i64 0}
!7 = !{!6, !6, i64 0}
!8 = !{!"int2", !6, i64 0}
!9 = !{!8, !8, i64 0}
!10 = !{!"int4", !6, i64 0}
!11 = !{!10, !10, i64 0}
!12 = !{!"int8", !6, i64 0}
!13 = !{!12, !12, i64 0}
!14 = !{!"float4", !6, i64 0}
!15 = !{!14, !14, i64 0}
!16 = !{!"float8", !6, i64 0}
!17 = !{!16, !16, i64 0}
!18 = !{!"float10", !6, i64 0}
!19 = !{!18, !18, i64 0}
!20 = !{!"pointer2", !6, i64 0}
!21 = !{!20, !20, i64 0}
!22 = !{!"pointer4", !6, i64 0}
!23 = !{!22, !22, i64 0}
