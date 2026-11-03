target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32-unknown-unknown"

define i32 @bench_x_switch(i32 %0) {
  %2 = icmp sgt i32 %0, 0
  br i1 %2, label %3, label %77

3:
  br label %72

4:
  %5 = mul i32 %37, 1103515245
  %6 = add i32 %5, 12345
  %7 = and i32 %6, 393216
  %8 = icmp eq i32 %7, 393216
  br i1 %8, label %52, label %44

9:
  %10 = mul i32 %45, 1103515245
  %11 = add i32 %10, 12345
  %12 = and i32 %11, 16711680
  %13 = icmp samesign ult i32 %12, 655360
  br i1 %13, label %72, label %36

14:
  %15 = mul i32 %53, 1103515245
  %16 = add i32 %15, 12345
  %17 = and i32 %16, 16711680
  %18 = icmp samesign ugt i32 %17, 13107200
  br i1 %18, label %60, label %65

19:
  %20 = mul i32 %16, 1103515245
  %21 = add i32 %20, 12345
  %22 = and i32 %21, 16711680
  %23 = icmp eq i32 %22, 196608
  br i1 %23, label %44, label %72

24:
  %25 = mul i32 %66, 1103515245
  %26 = add i32 %25, 12345
  %27 = and i32 %26, 65536
  %28 = icmp eq i32 %27, 0
  br i1 %28, label %72, label %36

29:
  %30 = mul i32 %73, 1103515245
  %31 = add i32 %30, 12345
  %32 = lshr i32 %31, 16
  %33 = and i32 %32, 255
  %34 = icmp samesign ult i32 %33, 64
  br i1 %34, label %65, label %.si.unfold.true.jt2

.si.unfold.true.jt2:
  %35 = icmp samesign ult i32 %33, 128
  br i1 %35, label %52, label %72

36:
  %37 = phi i32 [ %26, %24 ], [ %11, %9 ]
  %38 = phi i32 [ %71, %24 ], [ %51, %9 ]
  %39 = phi i32 [ %70, %24 ], [ %50, %9 ]
  %40 = and i32 %38, 3
  %41 = mul nuw nsw i32 %40, 3
  %42 = add nsw i32 %41, %39
  %43 = add nuw nsw i32 %38, 1
  %exitcond.jt3 = icmp eq i32 %43, %0
  br i1 %exitcond.jt3, label %77, label %4

44:
  %45 = phi i32 [ %21, %19 ], [ %6, %4 ]
  %46 = phi i32 [ %64, %19 ], [ %43, %4 ]
  %47 = phi i32 [ %63, %19 ], [ %42, %4 ]
  %48 = and i32 %46, 3
  %49 = mul nuw nsw i32 %48, 5
  %50 = add nsw i32 %49, %47
  %51 = add nuw nsw i32 %46, 1
  %exitcond.jt5 = icmp eq i32 %51, %0
  br i1 %exitcond.jt5, label %77, label %9

52:
  %53 = phi i32 [ %6, %4 ], [ %31, %.si.unfold.true.jt2 ]
  %54 = phi i32 [ %43, %4 ], [ %76, %.si.unfold.true.jt2 ]
  %55 = phi i32 [ %42, %4 ], [ %75, %.si.unfold.true.jt2 ]
  %56 = shl i32 %54, 1
  %57 = and i32 %56, 6
  %58 = add nsw i32 %57, %55
  %59 = add nuw nsw i32 %54, 1
  %exitcond.jt2 = icmp eq i32 %59, %0
  br i1 %exitcond.jt2, label %77, label %14

60:
  %61 = shl i32 %59, 2
  %62 = and i32 %61, 12
  %63 = add nsw i32 %62, %58
  %64 = add nuw nsw i32 %54, 2
  %exitcond.jt4 = icmp eq i32 %64, %0
  br i1 %exitcond.jt4, label %77, label %19

65:
  %66 = phi i32 [ %31, %29 ], [ %16, %14 ]
  %67 = phi i32 [ %76, %29 ], [ %59, %14 ]
  %68 = phi i32 [ %75, %29 ], [ %58, %14 ]
  %69 = and i32 %67, 3
  %70 = add nsw i32 %69, %68
  %71 = add nuw nsw i32 %67, 1
  %exitcond.jt1 = icmp eq i32 %71, %0
  br i1 %exitcond.jt1, label %77, label %24

72:
  %73 = phi i32 [ 1341714958, %3 ], [ %26, %24 ], [ %11, %9 ], [ %21, %19 ], [ %31, %.si.unfold.true.jt2 ]
  %74 = phi i32 [ 0, %3 ], [ %71, %24 ], [ %51, %9 ], [ %64, %19 ], [ %76, %.si.unfold.true.jt2 ]
  %75 = phi i32 [ 0, %3 ], [ %70, %24 ], [ %50, %9 ], [ %63, %19 ], [ %75, %.si.unfold.true.jt2 ]
  %76 = add nuw nsw i32 %74, 1
  %exitcond.jt0 = icmp eq i32 %76, %0
  br i1 %exitcond.jt0, label %77, label %29

77:
  %78 = phi i32 [ 0, %1 ], [ %42, %36 ], [ %50, %44 ], [ %58, %52 ], [ %63, %60 ], [ %70, %65 ], [ %75, %72 ]
  ret i32 %78
}
