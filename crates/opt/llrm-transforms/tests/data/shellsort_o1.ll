target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"

@opq0 = internal global i32 0, align 4

define dso_local i64 @bench_shellsort(i16 %0) {
  %2 = alloca [64 x i16], align 2
  br label %3

3:                                                ; preds = %1, %3
  %indvars.iv = phi i64 [ 0, %1 ], [ %indvars.iv.next, %3 ]
  %indvars.iv.tr = trunc nuw nsw i64 %indvars.iv to i16
  %4 = shl nuw nsw i16 %indvars.iv.tr, 7
  %5 = xor i16 %4, %0
  %6 = trunc i64 %indvars.iv to i16
  %7 = mul i16 %6, 109
  %8 = add i16 %7, 37
  %9 = xor i16 %5, %8
  %10 = getelementptr inbounds nuw [2 x i8], ptr %2, i64 %indvars.iv
  store i16 %9, ptr %10, align 2
  %indvars.iv.next = add nuw nsw i64 %indvars.iv, 1
  %exitcond.not = icmp eq i64 %indvars.iv.next, 64
  br i1 %exitcond.not, label %.preheader40, label %3

.preheader40:                                     ; preds = %3, %._crit_edge
  %.03249 = phi i16 [ %24, %._crit_edge ], [ 32, %3 ]
  %11 = zext nneg i16 %.03249 to i64
  br label %.lr.ph48

.lr.ph48:                                         ; preds = %.preheader40, %.critedge
  %indvars.iv53 = phi i64 [ %11, %.preheader40 ], [ %indvars.iv.next54, %.critedge ]
  %12 = getelementptr inbounds nuw [2 x i8], ptr %2, i64 %indvars.iv53
  %13 = load i16, ptr %12, align 2
  %14 = trunc nuw nsw i64 %indvars.iv53 to i16
  br label %.lr.ph

.lr.ph:                                           ; preds = %.lr.ph48, %19
  %.044 = phi i16 [ %narrow39, %19 ], [ %14, %.lr.ph48 ]
  %narrow39 = sub nuw i16 %.044, %.03249
  %15 = zext i16 %narrow39 to i64
  %16 = getelementptr inbounds nuw [2 x i8], ptr %2, i64 %15
  %17 = load i16, ptr %16, align 2
  %18 = icmp ugt i16 %17, %13
  br i1 %18, label %19, label %.critedge

19:                                               ; preds = %.lr.ph
  %20 = zext i16 %.044 to i64
  %21 = getelementptr inbounds nuw [2 x i8], ptr %2, i64 %20
  store i16 %17, ptr %21, align 2
  %.not38 = icmp ult i16 %narrow39, %.03249
  br i1 %.not38, label %.critedge, label %.lr.ph

.critedge:                                        ; preds = %.lr.ph, %19
  %.0.lcssa.ph = phi i16 [ %.044, %.lr.ph ], [ %narrow39, %19 ]
  %22 = zext i16 %.0.lcssa.ph to i64
  %23 = getelementptr inbounds nuw [2 x i8], ptr %2, i64 %22
  store i16 %13, ptr %23, align 2
  %indvars.iv.next54 = add nuw nsw i64 %indvars.iv53, 1
  %exitcond56.not = icmp eq i64 %indvars.iv.next54, 64
  br i1 %exitcond56.not, label %._crit_edge, label %.lr.ph48

._crit_edge:                                      ; preds = %.critedge
  %24 = lshr i16 %.03249, 1
  %.not = icmp eq i16 %24, 0
  br i1 %.not, label %.preheader, label %.preheader40

.preheader:                                       ; preds = %._crit_edge, %.preheader
  %indvars.iv57 = phi i64 [ %indvars.iv.next58, %.preheader ], [ 0, %._crit_edge ]
  %.03151 = phi i64 [ %29, %.preheader ], [ 0, %._crit_edge ]
  %25 = getelementptr inbounds nuw [2 x i8], ptr %2, i64 %indvars.iv57
  %26 = load i16, ptr %25, align 2
  %27 = zext i16 %26 to i64
  %indvars.iv.next58 = add nuw nsw i64 %indvars.iv57, 1
  %28 = mul nuw nsw i64 %indvars.iv.next58, %27
  %29 = add i64 %28, %.03151
  %exitcond60.not = icmp eq i64 %indvars.iv.next58, 64
  br i1 %exitcond60.not, label %30, label %.preheader

30:                                               ; preds = %.preheader
  ret i64 %29
}



