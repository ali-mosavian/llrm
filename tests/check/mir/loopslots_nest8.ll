; RUN: llrm-mir %s
; CHECK: define {{.*}} @_fn(

; scaling.py's nest(8) (loops 8 deep, a statement at every depth) as the pipeline hands it to isel (llrm-c -m16 -O2, 21-homes.ll),
; its function alone, in the plain calling convention. Pinned at loopslots' input: each loop of the nest is ranked.
target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-n8:16:32"

define i16 @_fn(i16 %0, i16 %1, i16 %2, i16 %3) addrspace(1) memory(none) nounwind norecurse {
b1:
  br label %b2

b2:
  %4 = phi i16 [ 0, %b1 ], [ %35, %b6 ]
  %5 = phi i16 [ %3, %b1 ], [ %34, %b6 ]
  %6 = phi i16 [ %2, %b1 ], [ %33, %b6 ]
  %7 = phi i16 [ %1, %b1 ], [ %32, %b6 ]
  %8 = phi i16 [ %0, %b1 ], [ %31, %b6 ]
  %9 = and i16 %8, 1
  %10 = add i16 %9, 1
  %11 = icmp ult i16 %4, %10
  br i1 %11, label %b4, label %b3

b3:
  %12 = phi i16 [ %8, %b2 ]
  %13 = phi i16 [ %7, %b2 ]
  %14 = phi i16 [ %6, %b2 ]
  %15 = phi i16 [ %5, %b2 ]
  %16 = xor i16 %12, %13
  %17 = xor i16 %16, %14
  %18 = xor i16 %17, %15
  ret i16 %18

b4:
  %19 = mul i16 %8, 49
  %20 = xor i16 %7, %4
  %21 = add i16 %19, %20
  br label %b5

b5:
  %22 = phi i16 [ 0, %b4 ], [ %52, %b9 ]
  %23 = phi i16 [ %5, %b4 ], [ %51, %b9 ]
  %24 = phi i16 [ %6, %b4 ], [ %50, %b9 ]
  %25 = phi i16 [ %7, %b4 ], [ %49, %b9 ]
  %26 = phi i16 [ %21, %b4 ], [ %48, %b9 ]
  %27 = lshr i16 %25, 1
  %28 = and i16 %27, 1
  %29 = add i16 %28, 1
  %30 = icmp ult i16 %22, %29
  br i1 %30, label %b7, label %b6

b6:
  %31 = phi i16 [ %26, %b5 ]
  %32 = phi i16 [ %25, %b5 ]
  %33 = phi i16 [ %24, %b5 ]
  %34 = phi i16 [ %23, %b5 ]
  %35 = add i16 %4, 1
  br label %b2

b7:
  %36 = mul i16 %25, 91
  %37 = xor i16 %24, %22
  %38 = add i16 %36, %37
  br label %b8

b8:
  %39 = phi i16 [ 0, %b7 ], [ %69, %b12 ]
  %40 = phi i16 [ %23, %b7 ], [ %68, %b12 ]
  %41 = phi i16 [ %24, %b7 ], [ %67, %b12 ]
  %42 = phi i16 [ %38, %b7 ], [ %66, %b12 ]
  %43 = phi i16 [ %26, %b7 ], [ %65, %b12 ]
  %44 = lshr i16 %41, 2
  %45 = and i16 %44, 1
  %46 = add i16 %45, 1
  %47 = icmp ult i16 %39, %46
  br i1 %47, label %b10, label %b9

b9:
  %48 = phi i16 [ %43, %b8 ]
  %49 = phi i16 [ %42, %b8 ]
  %50 = phi i16 [ %41, %b8 ]
  %51 = phi i16 [ %40, %b8 ]
  %52 = add i16 %22, 1
  br label %b5

b10:
  %53 = mul i16 %41, 19
  %54 = xor i16 %40, %39
  %55 = add i16 %53, %54
  br label %b11

b11:
  %56 = phi i16 [ 0, %b10 ], [ %86, %b15 ]
  %57 = phi i16 [ %40, %b10 ], [ %85, %b15 ]
  %58 = phi i16 [ %55, %b10 ], [ %84, %b15 ]
  %59 = phi i16 [ %42, %b10 ], [ %83, %b15 ]
  %60 = phi i16 [ %43, %b10 ], [ %82, %b15 ]
  %61 = lshr i16 %57, 3
  %62 = and i16 %61, 1
  %63 = add i16 %62, 1
  %64 = icmp ult i16 %56, %63
  br i1 %64, label %b13, label %b12

b12:
  %65 = phi i16 [ %60, %b11 ]
  %66 = phi i16 [ %59, %b11 ]
  %67 = phi i16 [ %58, %b11 ]
  %68 = phi i16 [ %57, %b11 ]
  %69 = add i16 %39, 1
  br label %b8

b13:
  %70 = mul i16 %57, 61
  %71 = xor i16 %60, %56
  %72 = add i16 %70, %71
  br label %b14

b14:
  %73 = phi i16 [ 0, %b13 ], [ %102, %b18 ]
  %74 = phi i16 [ %72, %b13 ], [ %101, %b18 ]
  %75 = phi i16 [ %58, %b13 ], [ %100, %b18 ]
  %76 = phi i16 [ %59, %b13 ], [ %99, %b18 ]
  %77 = phi i16 [ %60, %b13 ], [ %98, %b18 ]
  %78 = lshr i16 %77, 4
  %79 = and i16 %78, 1
  %80 = add i16 %79, 1
  %81 = icmp ult i16 %73, %80
  br i1 %81, label %b16, label %b15

b15:
  %82 = phi i16 [ %77, %b14 ]
  %83 = phi i16 [ %76, %b14 ]
  %84 = phi i16 [ %75, %b14 ]
  %85 = phi i16 [ %74, %b14 ]
  %86 = add i16 %56, 1
  br label %b11

b16:
  %87 = mul i16 %77, 111
  %88 = xor i16 %76, %73
  %89 = add i16 %87, %88
  br label %b17

b17:
  %90 = phi i16 [ 0, %b16 ], [ %117, %b21 ]
  %91 = phi i16 [ %74, %b16 ], [ %116, %b21 ]
  %92 = phi i16 [ %75, %b16 ], [ %115, %b21 ]
  %93 = phi i16 [ %76, %b16 ], [ %105, %b21 ]
  %94 = phi i16 [ %89, %b16 ], [ %114, %b21 ]
  %95 = and i16 %93, 1
  %96 = add i16 %95, 1
  %97 = icmp ult i16 %90, %96
  br i1 %97, label %b19, label %b18

b18:
  %98 = phi i16 [ %94, %b17 ]
  %99 = phi i16 [ %93, %b17 ]
  %100 = phi i16 [ %92, %b17 ]
  %101 = phi i16 [ %91, %b17 ]
  %102 = add i16 %73, 1
  br label %b14

b19:
  %103 = mul i16 %93, 39
  %104 = xor i16 %92, %90
  %105 = add i16 %103, %104
  br label %b20

b20:
  %106 = phi i16 [ 0, %b19 ], [ %131, %b24 ]
  %107 = phi i16 [ %91, %b19 ], [ %130, %b24 ]
  %108 = phi i16 [ %92, %b19 ], [ %120, %b24 ]
  %109 = phi i16 [ %94, %b19 ], [ %129, %b24 ]
  %110 = lshr i16 %108, 1
  %111 = and i16 %110, 1
  %112 = add i16 %111, 1
  %113 = icmp ult i16 %106, %112
  br i1 %113, label %b22, label %b21

b21:
  %114 = phi i16 [ %109, %b20 ]
  %115 = phi i16 [ %108, %b20 ]
  %116 = phi i16 [ %107, %b20 ]
  %117 = add i16 %90, 1
  br label %b17

b22:
  %118 = mul i16 %108, 81
  %119 = xor i16 %107, %106
  %120 = add i16 %118, %119
  %121 = add i16 %105, %120
  br label %b23

b23:
  %122 = phi i16 [ 0, %b22 ], [ %136, %b25 ]
  %123 = phi i16 [ %107, %b22 ], [ %134, %b25 ]
  %124 = phi i16 [ %109, %b22 ], [ %135, %b25 ]
  %125 = lshr i16 %123, 2
  %126 = and i16 %125, 1
  %127 = add i16 %126, 1
  %128 = icmp ult i16 %122, %127
  br i1 %128, label %b25, label %b24

b24:
  %129 = phi i16 [ %124, %b23 ]
  %130 = phi i16 [ %123, %b23 ]
  %131 = add i16 %106, 1
  br label %b20

b25:
  %132 = mul i16 %123, 9
  %133 = xor i16 %124, %122
  %134 = add i16 %132, %133
  %135 = xor i16 %124, %121
  %136 = add i16 %122, 1
  br label %b23
}
