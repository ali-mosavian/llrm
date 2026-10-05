; RUN: llrm-mir %s
; CHECK: define {{.*}} @_sort(

target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-n8:16:32"


define internal fastcc void @_sort(ptr nocapture %0, i16 %1, i16 %2) memory(argmem: readwrite, inaccessiblemem: readwrite) {
b1:
  %3 = icmp sge i16 %1, %2
  br i1 %3, label %b4, label %b2

b2:
  %4 = mul i16 %2, 2
  %5 = getelementptr inbounds i8, ptr %0, i16 %4
  %6 = load i16, ptr %5, !tbaa !9
  %7 = shl i16 %1, 1
  %8 = shl i16 %2, 1
  %9 = sub i16 %7, %8
  %10 = getelementptr i8, ptr %0, i16 %8
  %11 = icmp sle i16 %2, %1
  br i1 %11, label %b6, label %30

b4:
  ret void

b6:
  %12 = phi i16 [ %1, %b2 ], [ %23, %31 ]
  %13 = mul i16 %12, 2
  %14 = getelementptr inbounds i8, ptr %0, i16 %13
  %15 = load i16, ptr %14, !tbaa !9
  %16 = load i16, ptr %5, !tbaa !9
  store i16 %16, ptr %14, !tbaa !9
  store i16 %15, ptr %5, !tbaa !9
  %17 = sub nsw i16 %12, 1
  call fastcc void @_sort(ptr %0, i16 %1, i16 %17)
  %18 = add nsw i16 %12, 1
  call fastcc void @_sort(ptr %0, i16 %18, i16 %2)
  br label %b4

b7:
  %19 = phi i16 [ %23, %b8 ], [ %1, %30 ]
  %lsr.iv1 = phi i16 [ %lsr.iv.next, %b8 ], [ %9, %30 ]
  %20 = getelementptr i8, ptr %10, i16 %lsr.iv1
  %21 = load i16, ptr %20, !tbaa !9
  %22 = icmp slt i16 %21, %6
  br i1 %22, label %b9, label %b8

b8:
  %23 = phi i16 [ %19, %b7 ], [ %29, %b9 ]
  %lsr.iv.next = add i16 %lsr.iv1, 2
  %24 = icmp ne i16 %lsr.iv.next, 0
  br i1 %24, label %b7, label %31

b9:
  %25 = mul i16 %19, 2
  %26 = getelementptr inbounds i8, ptr %0, i16 %25
  %27 = load i16, ptr %26, !tbaa !9
  store i16 %21, ptr %26, !tbaa !9
  %28 = getelementptr i8, ptr %10, i16 %lsr.iv1
  store i16 %27, ptr %28, !tbaa !9
  %29 = add nsw i16 %19, 1
  br label %b8

30:
  br label %b7

31:
  br label %b6
}

!5 = !{!"Simple C/C++ TBAA"}
!6 = !{!"omnipotent char", !5, i64 0}
!8 = !{!"int2", !6, i64 0}
!9 = !{!8, !8, i64 0}
