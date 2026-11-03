
; Function Attrs: nofree norecurse nosync nounwind memory(read, argmem: readwrite, inaccessiblemem: none)
define i16 @lz4(ptr nocapture readonly %0, ptr %1, i16 %2) {
  %4 = getelementptr i8, ptr %1, i16 %2
  %.not83 = icmp eq i16 %2, 0
  br i1 %.not83, label %._crit_edge79, label %.lr.ph78

.loopexit:                                        ; preds = %.lr.ph73, %.loopexit57
  %.495 = phi ptr [ %30, %.loopexit57 ], [ %.494, %.lr.ph73 ]
  %.341.lcssa = phi ptr [ %.240.lcssa, %.loopexit57 ], [ %37, %.lr.ph73 ]
  %5 = icmp ult ptr %.341.lcssa, %4
  br i1 %5, label %.lr.ph78, label %._crit_edge79

.lr.ph78:                                         ; preds = %3, %.loopexit
  %.03776 = phi ptr [ %.495, %.loopexit ], [ %0, %3 ]
  %.03875 = phi ptr [ %.341.lcssa, %.loopexit ], [ %1, %3 ]
  %6 = getelementptr i8, ptr %.03776, i16 1
  %7 = load i8, ptr %.03776, align 1
  %8 = lshr i8 %7, 4
  %9 = zext nneg i8 %8 to i16
  %10 = and i8 %7, 15
  %narrow = add i8 %10, 4
  %11 = icmp eq i8 %8, 15
  br i1 %11, label %.preheader58, label %.loopexit59

.preheader58:                                     ; preds = %.lr.ph78, %.preheader58
  %.143 = phi i16 [ %15, %.preheader58 ], [ 15, %.lr.ph78 ]
  %.2 = phi ptr [ %12, %.preheader58 ], [ %6, %.lr.ph78 ]
  %12 = getelementptr i8, ptr %.2, i16 1
  %13 = load i8, ptr %.2, align 1
  %14 = zext i8 %13 to i16
  %15 = add i16 %.143, %14
  %16 = icmp eq i8 %13, -1
  br i1 %16, label %.preheader58, label %.loopexit59

.loopexit59:                                      ; preds = %.preheader58, %.lr.ph78
  %.042 = phi i16 [ %9, %.lr.ph78 ], [ %15, %.preheader58 ]
  %.1 = phi ptr [ %6, %.lr.ph78 ], [ %12, %.preheader58 ]
  %.not63 = icmp eq i16 %.042, 0
  br i1 %.not63, label %._crit_edge, label %.lr.ph

.lr.ph:                                           ; preds = %.loopexit59, %.lr.ph
  %.366 = phi ptr [ %17, %.lr.ph ], [ %.1, %.loopexit59 ]
  %.24065 = phi ptr [ %19, %.lr.ph ], [ %.03875, %.loopexit59 ]
  %.24464 = phi i16 [ %20, %.lr.ph ], [ %.042, %.loopexit59 ]
  %17 = getelementptr i8, ptr %.366, i16 1
  %18 = load i8, ptr %.366, align 1
  %19 = getelementptr i8, ptr %.24065, i16 1
  store i8 %18, ptr %.24065, align 1
  %20 = add i16 %.24464, -1
  %.not = icmp eq i16 %20, 0
  br i1 %.not, label %._crit_edge, label %.lr.ph

._crit_edge:                                      ; preds = %.lr.ph, %.loopexit59
  %.240.lcssa = phi ptr [ %.03875, %.loopexit59 ], [ %19, %.lr.ph ]
  %.3.lcssa = phi ptr [ %.1, %.loopexit59 ], [ %17, %.lr.ph ]
  %.not53 = icmp ult ptr %.240.lcssa, %4
  br i1 %.not53, label %21, label %._crit_edge79

21:                                               ; preds = %._crit_edge
  %22 = getelementptr i8, ptr %.3.lcssa, i16 1
  %23 = load i8, ptr %.3.lcssa, align 1
  %24 = zext i8 %23 to i16
  %25 = getelementptr i8, ptr %.3.lcssa, i16 2
  %26 = load i8, ptr %22, align 1
  %.neg55 = sub i8 0, %26
  %.neg55.z = zext i8 %.neg55 to i16
  %.neg = shl i16 %.neg55.z, 8
  %.neg54 = sub i16 %.neg, %24
  %27 = getelementptr i8, ptr %.240.lcssa, i16 %.neg54
  %28 = zext nneg i8 %narrow to i16
  %29 = icmp eq i8 %narrow, 19
  br i1 %29, label %.preheader, label %.lr.ph73.preheader

.preheader:                                       ; preds = %21, %.preheader
  %.446 = phi i16 [ %33, %.preheader ], [ 19, %21 ]
  %.5 = phi ptr [ %30, %.preheader ], [ %25, %21 ]
  %30 = getelementptr i8, ptr %.5, i16 1
  %31 = load i8, ptr %.5, align 1
  %32 = zext i8 %31 to i16
  %33 = add i16 %.446, %32
  %34 = icmp eq i8 %31, -1
  br i1 %34, label %.preheader, label %.loopexit57

.loopexit57:                                      ; preds = %.preheader
  %.not5668 = icmp eq i16 %33, 0
  br i1 %.not5668, label %.loopexit, label %.lr.ph73.preheader

.lr.ph73.preheader:                               ; preds = %21, %.loopexit57
  %.494 = phi ptr [ %30, %.loopexit57 ], [ %25, %21 ]
  %.34593 = phi i16 [ %33, %.loopexit57 ], [ %28, %21 ]
  br label %.lr.ph73

.lr.ph73:                                         ; preds = %.lr.ph73.preheader, %.lr.ph73
  %.071 = phi ptr [ %35, %.lr.ph73 ], [ %27, %.lr.ph73.preheader ]
  %.34170 = phi ptr [ %37, %.lr.ph73 ], [ %.240.lcssa, %.lr.ph73.preheader ]
  %.54769 = phi i16 [ %38, %.lr.ph73 ], [ %.34593, %.lr.ph73.preheader ]
  %35 = getelementptr i8, ptr %.071, i16 1
  %36 = load i8, ptr %.071, align 1
  %37 = getelementptr i8, ptr %.34170, i16 1
  store i8 %36, ptr %.34170, align 1
  %38 = add i16 %.54769, -1
  %.not56 = icmp eq i16 %38, 0
  br i1 %.not56, label %.loopexit, label %.lr.ph73

._crit_edge79:                                    ; preds = %.loopexit, %._crit_edge, %3
  %.139 = phi ptr [ %1, %3 ], [ %.240.lcssa, %._crit_edge ], [ %.341.lcssa, %.loopexit ]
  %39 = ptrtoint ptr %.139 to i16
  %40 = ptrtoint ptr %1 to i16
  %41 = sub i16 %39, %40
  ret i16 %41
}



