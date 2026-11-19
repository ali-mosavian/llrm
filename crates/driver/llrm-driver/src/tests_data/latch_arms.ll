target datalayout = "@m32-datalayout@"

@ga = external global i32, align 4
@gb = external global i32, align 4
@src = external global [256 x i32], align 4

; x_cordic's shape: two arms of a loop body join in the latch (`inc; cmp; branch back`).
define dso_local i32 @arms(i32 noundef %n) #0 {
entry:
  %any = icmp sgt i32 %n, 0
  br i1 %any, label %loop, label %done

loop:
  %k = phi i32 [ 0, %entry ], [ %k1, %latch ]
  %x = phi i32 [ 0, %entry ], [ %z, %latch ]
  %at = getelementptr inbounds [256 x i32], ptr @src, i32 0, i32 %k
  %v = load i32, ptr %at, align 4
  %c = icmp sgt i32 %v, 0
  br i1 %c, label %up, label %down

up:
  store volatile i32 %v, ptr @ga, align 4
  %xa = add i32 %x, %v
  br label %latch

down:
  store volatile i32 %v, ptr @gb, align 4
  %xb = sub i32 %x, %v
  br label %latch

latch:
  %x2 = phi i32 [ %xa, %up ], [ %xb, %down ]
  %y = xor i32 %x2, 5
  %z = add i32 %y, %k
  %k1 = add nuw nsw i32 %k, 1
  %again = icmp ne i32 %k1, %n
  br i1 %again, label %loop, label %done

done:
  %out = phi i32 [ 0, %entry ], [ %z, %latch ]
  ret i32 %out
}

attributes #0 = { nounwind }
