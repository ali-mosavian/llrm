; RUN: llrm-mir %s
; CHECK: define {{.*}} @_fn(

; scaling.py's nest(4) (loops 4 deep, a statement at every depth) as the pipeline hands it to isel (llrm-c -m16 -O2, 21-homes.ll),
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
  %22 = phi i16 [ 0, %b4 ], [ %50, %b9 ]
  %23 = phi i16 [ %5, %b4 ], [ %49, %b9 ]
  %24 = phi i16 [ %6, %b4 ], [ %48, %b9 ]
  %25 = phi i16 [ %7, %b4 ], [ %38, %b9 ]
  %26 = phi i16 [ %21, %b4 ], [ %47, %b9 ]
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
  %39 = phi i16 [ 0, %b7 ], [ %64, %b12 ]
  %40 = phi i16 [ %23, %b7 ], [ %63, %b12 ]
  %41 = phi i16 [ %24, %b7 ], [ %53, %b12 ]
  %42 = phi i16 [ %26, %b7 ], [ %62, %b12 ]
  %43 = lshr i16 %41, 2
  %44 = and i16 %43, 1
  %45 = add i16 %44, 1
  %46 = icmp ult i16 %39, %45
  br i1 %46, label %b10, label %b9

b9:
  %47 = phi i16 [ %42, %b8 ]
  %48 = phi i16 [ %41, %b8 ]
  %49 = phi i16 [ %40, %b8 ]
  %50 = add i16 %22, 1
  br label %b5

b10:
  %51 = mul i16 %41, 19
  %52 = xor i16 %40, %39
  %53 = add i16 %51, %52
  %54 = add i16 %38, %53
  br label %b11

b11:
  %55 = phi i16 [ 0, %b10 ], [ %69, %b13 ]
  %56 = phi i16 [ %40, %b10 ], [ %67, %b13 ]
  %57 = phi i16 [ %42, %b10 ], [ %68, %b13 ]
  %58 = lshr i16 %56, 3
  %59 = and i16 %58, 1
  %60 = add i16 %59, 1
  %61 = icmp ult i16 %55, %60
  br i1 %61, label %b13, label %b12

b12:
  %62 = phi i16 [ %57, %b11 ]
  %63 = phi i16 [ %56, %b11 ]
  %64 = add i16 %39, 1
  br label %b8

b13:
  %65 = mul i16 %56, 61
  %66 = xor i16 %57, %55
  %67 = add i16 %65, %66
  %68 = xor i16 %57, %54
  %69 = add i16 %55, 1
  br label %b11
}
