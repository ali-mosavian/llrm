target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"

define i16 @f(i16 %0, i1 %1, i1 %2, i1 %3) {
  br label %5

5:                                                ; preds = %7, %4
  %6 = phi i1 [ false, %4 ], [ true, %7 ]
  br label %8

7:                                                ; preds = %8
  br i1 %6, label %10, label %5

8:                                                ; preds = %8, %5
  %9 = load volatile float, ptr null, align 2
  br i1 %1, label %7, label %8

10:                                               ; preds = %11, %7
  br label %12

11:                                               ; preds = %12
  br i1 %2, label %16, label %10

12:                                               ; preds = %12, %10
  %13 = phi i16 [ 0, %10 ], [ %0, %12 ]
  %14 = load volatile float, ptr null, align 2
  %15 = icmp eq i16 %13, 1
  br i1 %15, label %11, label %12

16:                                               ; preds = %18, %11
  %17 = phi i1 [ true, %18 ], [ false, %11 ]
  br label %19

18:                                               ; preds = %19
  br i1 %17, label %._crit_edge, label %16

19:                                               ; preds = %19, %16
  %20 = load volatile float, ptr null, align 2
  br i1 %3, label %18, label %19

._crit_edge:                                      ; preds = %._crit_edge, %18
  br label %._crit_edge
}
