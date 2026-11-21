target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"

; Function Attrs: noinline nounwind
define zeroext i8 @f(i8 noundef zeroext %0, i8 noundef zeroext %1) #0 {
  br label %3

3:                                                ; preds = %11, %2
  %.094 = phi i8 [ 0, %2 ], [ %.195, %11 ]
  %.093 = phi i8 [ 0, %2 ], [ %12, %11 ]
  %.091 = phi i8 [ undef, %2 ], [ %.192, %11 ]
  %.0 = phi i8 [ undef, %2 ], [ %.1, %11 ]
  %4 = icmp ult i8 %.093, %1
  br i1 %4, label %5, label %18

5:                                                ; preds = %3, %7
  %.192 = phi i8 [ %8, %7 ], [ %.091, %3 ]
  %.1 = phi i8 [ %0, %7 ], [ %.0, %3 ]
  %6 = icmp ult i8 %.192, %1
  br i1 %6, label %7, label %9

7:                                                ; preds = %5
  %8 = add i8 %.192, 1
  br label %5

9:                                                ; preds = %5, %16
  %.195 = phi i8 [ %.296, %16 ], [ %.094, %5 ]
  %.090 = phi i8 [ %17, %16 ], [ 0, %5 ]
  %10 = icmp ult i8 %.090, %1
  br i1 %10, label %13, label %11

11:                                               ; preds = %9
  %12 = add i8 %.093, 1
  br label %3

13:                                               ; preds = %9
  %14 = icmp ugt i8 %0, %.195
  br i1 %14, label %16, label %15

15:                                               ; preds = %13
  %.not120 = icmp eq i8 %1, -1
  br i1 %.not120, label %16, label %common.ret

common.ret:                                       ; preds = %15, %60
  %common.ret.op = phi i8 [ %61, %60 ], [ 0, %15 ]
  ret i8 %common.ret.op

16:                                               ; preds = %15, %13
  %.296 = phi i8 [ -1, %13 ], [ %.195, %15 ]
  %17 = add i8 %.090, 1
  br label %9

18:                                               ; preds = %3, %23
  %.089 = phi i8 [ %24, %23 ], [ 0, %3 ]
  %.087 = phi i8 [ %.188, %23 ], [ undef, %3 ]
  %19 = icmp ult i8 %.089, %1
  br i1 %19, label %20, label %27

20:                                               ; preds = %18, %25
  %.188 = phi i8 [ %26, %25 ], [ %.087, %18 ]
  %21 = add i8 %1, 1
  %22 = icmp ult i8 %.188, %21
  br i1 %22, label %25, label %23

23:                                               ; preds = %20
  %24 = add i8 %.089, 1
  br label %18

25:                                               ; preds = %20
  %26 = add i8 %.188, 1
  br label %20

27:                                               ; preds = %18, %47
  %.0108 = phi i8 [ %.1109, %47 ], [ undef, %18 ]
  %.098 = phi i8 [ %.2100, %47 ], [ undef, %18 ]
  %.397 = phi i8 [ %.7, %47 ], [ %.094, %18 ]
  %.086 = phi i8 [ %48, %47 ], [ 0, %18 ]
  %.084 = phi i8 [ %.185, %47 ], [ undef, %18 ]
  %.082 = phi i8 [ %.183, %47 ], [ undef, %18 ]
  %.079 = phi i8 [ %.180, %47 ], [ undef, %18 ]
  %.075 = phi i8 [ %.176, %47 ], [ undef, %18 ]
  %28 = add i8 %1, 1
  %29 = icmp ult i8 %.086, %28
  br i1 %29, label %30, label %58

30:                                               ; preds = %27, %32
  %.199 = phi i8 [ 0, %32 ], [ %.098, %27 ]
  %.4 = phi i8 [ %..4, %32 ], [ %.397, %27 ]
  %.185 = phi i8 [ %34, %32 ], [ %.084, %27 ]
  %31 = icmp ult i8 %.185, %1
  br i1 %31, label %32, label %35

32:                                               ; preds = %30
  %33 = icmp ult i8 %.0, 3
  %..4 = select i1 %33, i8 0, i8 %.4
  %34 = add i8 %.185, 1
  br label %30

35:                                               ; preds = %30, %43
  %.1109 = phi i8 [ %.4112, %43 ], [ %.0108, %30 ]
  %.6 = phi i8 [ 0, %43 ], [ %.4, %30 ]
  %.183 = phi i8 [ %44, %43 ], [ %.082, %30 ]
  %36 = icmp ult i8 %.183, %28
  br i1 %36, label %37, label %45

37:                                               ; preds = %35
  %.not118 = icmp eq i8 %.1109, 100
  br i1 %.not118, label %42, label %38

38:                                               ; preds = %37, %40
  %.2110 = phi i8 [ %.0, %40 ], [ %.1109, %37 ]
  %.081 = phi i8 [ %41, %40 ], [ 0, %37 ]
  %39 = icmp ult i8 %.081, %28
  br i1 %39, label %40, label %43

40:                                               ; preds = %38
  %41 = add i8 %.081, 1
  br label %38

42:                                               ; preds = %37
  %.not119 = icmp eq i8 %.6, 0
  %. = select i1 %.not119, i8 0, i8 100
  br label %43

43:                                               ; preds = %38, %42
  %.4112 = phi i8 [ %., %42 ], [ %.2110, %38 ]
  %44 = add i8 %.183, 1
  br label %35

45:                                               ; preds = %35, %52
  %.2100 = phi i8 [ %.3101, %52 ], [ %.199, %35 ]
  %.7 = phi i8 [ %.8, %52 ], [ %.6, %35 ]
  %.180 = phi i8 [ %53, %52 ], [ %.079, %35 ]
  %.176 = phi i8 [ %.277, %52 ], [ %.075, %35 ]
  %46 = icmp ult i8 %.180, %28
  br i1 %46, label %49, label %47

47:                                               ; preds = %45
  %48 = add i8 %.086, 1
  br label %27

49:                                               ; preds = %54, %45
  %.3101 = phi i8 [ %.2100, %45 ], [ %.4102, %54 ]
  %.8 = phi i8 [ %.7, %45 ], [ 0, %54 ]
  %.078 = phi i16 [ 0, %45 ], [ 1, %54 ]
  %.277 = phi i8 [ %.176, %45 ], [ %.3, %54 ]
  %50 = zext i8 %1 to i16
  %51 = icmp samesign ult i16 %.078, %50
  br i1 %51, label %54, label %52

52:                                               ; preds = %49
  %53 = add i8 %.180, 1
  br label %45

54:                                               ; preds = %49, %56
  %.4102 = phi i8 [ -1, %56 ], [ %.3101, %49 ]
  %.3 = phi i8 [ %57, %56 ], [ %.277, %49 ]
  %55 = icmp ult i8 %.3, %1
  br i1 %55, label %56, label %49

56:                                               ; preds = %54
  %57 = add i8 %.3, 1
  br label %54

58:                                               ; preds = %27, %77
  %.5113 = phi i8 [ %.6114, %77 ], [ %.0108, %27 ]
  %.5103 = phi i8 [ %.6104, %77 ], [ %.098, %27 ]
  %.9 = phi i8 [ %.6104, %77 ], [ %.397, %27 ]
  %.074 = phi i8 [ %78, %77 ], [ 0, %27 ]
  %.069 = phi i8 [ %.170, %77 ], [ undef, %27 ]
  %59 = icmp ult i8 %.074, %28
  br i1 %59, label %62, label %60

60:                                               ; preds = %58
  %61 = mul i8 %.9, 11
  br label %common.ret

62:                                               ; preds = %58, %66
  %.6114 = phi i8 [ %.7115, %66 ], [ %.5113, %58 ]
  %.6104 = phi i8 [ %.7105, %66 ], [ %.5103, %58 ]
  %.10 = phi i8 [ 1, %66 ], [ %.9, %58 ]
  %.073 = phi i8 [ %67, %66 ], [ 0, %58 ]
  %63 = icmp ult i8 %.073, %28
  br i1 %63, label %64, label %76

64:                                               ; preds = %62, %70
  %.7115 = phi i8 [ %..7115, %70 ], [ %.6114, %62 ]
  %.7105 = phi i8 [ %.7115..8106, %70 ], [ %.6104, %62 ]
  %.11 = phi i8 [ %.12, %70 ], [ %.10, %62 ]
  %.072 = phi i8 [ %72, %70 ], [ 0, %62 ]
  %65 = icmp ult i8 %.072, %28
  br i1 %65, label %68, label %66

66:                                               ; preds = %64
  %67 = add i8 %.073, 1
  br label %62

68:                                               ; preds = %64, %73
  %.8106 = phi i8 [ %0, %73 ], [ %.7105, %64 ]
  %.12 = phi i8 [ %74, %73 ], [ %.11, %64 ]
  %.071 = phi i8 [ %75, %73 ], [ 0, %64 ]
  %69 = icmp ult i8 %.071, %1
  br i1 %69, label %73, label %70

70:                                               ; preds = %68
  %71 = icmp eq i8 %0, %.12
  %..7115 = select i1 %71, i8 0, i8 %.7115
  %.7115..8106 = select i1 %71, i8 %.7115, i8 %.8106
  %72 = add i8 %.072, 1
  br label %64

73:                                               ; preds = %68
  %74 = xor i8 %.8106, %0
  %75 = add i8 %.071, 1
  br label %68

76:                                               ; preds = %79, %62
  %.170 = phi i8 [ %.069, %62 ], [ %.2, %79 ]
  %.not = icmp eq i8 %1, 0
  br i1 %.not, label %77, label %79

77:                                               ; preds = %76
  %78 = add i8 %.074, 1
  br label %58

79:                                               ; preds = %76, %82
  %.2 = phi i8 [ %83, %82 ], [ %.170, %76 ]
  %80 = icmp ult i8 %.2, %1
  br i1 %80, label %81, label %76

81:                                               ; preds = %81, %79
  %.not117 = icmp eq i8 %1, 1
  br i1 %.not117, label %82, label %81

82:                                               ; preds = %81
  %83 = add i8 %.2, 1
  br label %79

; uselistorder directives
  uselistorder i8 %0, { 3, 4, 0, 2, 1 }
  uselistorder i8 %1, { 7, 8, 9, 6, 4, 13, 5, 10, 3, 11, 0, 1, 2, 12 }
  uselistorder i8 %.093, { 1, 0 }
  uselistorder i8 %.0, { 0, 2, 1 }
  uselistorder label %5, { 1, 0 }
  uselistorder i8 %.192, { 2, 0, 1 }
  uselistorder label %9, { 1, 0 }
  uselistorder i8 %.090, { 1, 0 }
  uselistorder label %common.ret, { 1, 0 }
  uselistorder label %18, { 1, 0 }
  uselistorder i8 %.089, { 1, 0 }
  uselistorder label %20, { 1, 0 }
  uselistorder i8 %.188, { 2, 1, 0 }
  uselistorder label %27, { 1, 0 }
  uselistorder i8 %.086, { 1, 0 }
  uselistorder i8 %28, { 3, 4, 5, 1, 0, 2, 6 }
  uselistorder label %30, { 1, 0 }
  uselistorder i8 %.4, { 1, 0 }
  uselistorder i8 %.185, { 2, 1, 0 }
  uselistorder label %35, { 1, 0 }
  uselistorder i8 %.1109, { 0, 2, 1 }
  uselistorder i8 %.183, { 2, 1, 0 }
  uselistorder label %38, { 1, 0 }
  uselistorder i8 %.081, { 1, 0 }
  uselistorder label %43, { 1, 0 }
  uselistorder label %45, { 1, 0 }
  uselistorder i8 %.180, { 2, 1, 0 }
  uselistorder label %54, { 1, 0 }
  uselistorder i8 %.3, { 2, 1, 0 }
  uselistorder label %58, { 1, 0 }
  uselistorder i8 %.074, { 1, 0 }
  uselistorder label %62, { 1, 0 }
  uselistorder i8 %.6104, { 0, 2, 1 }
  uselistorder i8 %.073, { 1, 0 }
  uselistorder label %64, { 1, 0 }
  uselistorder i8 %.072, { 1, 0 }
  uselistorder label %68, { 1, 0 }
  uselistorder i8 %.8106, { 1, 0 }
  uselistorder i8 %.12, { 1, 0 }
  uselistorder i8 %.071, { 1, 0 }
  uselistorder label %79, { 1, 0 }
  uselistorder i8 %.2, { 2, 1, 0 }
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr nocapture) #1

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr nocapture) #1

; uselistorder directives
uselistorder i8 0, { 16, 0, 1, 2, 3, 4, 5, 6, 15, 7, 9, 8, 10, 11, 14, 12, 13, 17, 18 }
uselistorder i8 1, { 4, 2, 5, 7, 6, 8, 0, 9, 10, 11, 12, 13, 14, 3, 15, 16, 1, 17, 18, 19 }

attributes #0 = { noinline nounwind "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
