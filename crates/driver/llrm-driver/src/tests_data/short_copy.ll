target datalayout = "@m32-datalayout@"

@a = internal global [256 x i32] zeroinitializer, align 4
@b = internal global [256 x i32] zeroinitializer, align 4

; The copy loop of x_mergesort's msort: a run-time count `%n`, usually a few elements.
define dso_local void @copy_run(i32 noundef %n) #0 {
entry:
  %any = icmp sgt i32 %n, 0
  br i1 %any, label %loop, label %done

loop:
  %i = phi i32 [ 0, %entry ], [ %next, %loop ]
  %from = getelementptr inbounds [256 x i32], ptr @a, i32 0, i32 %i
  %v = load i32, ptr %from, align 4
  %to = getelementptr inbounds [256 x i32], ptr @b, i32 0, i32 %i
  store i32 %v, ptr %to, align 4
  %next = add nuw nsw i32 %i, 1
  %again = icmp slt i32 %next, %n
  br i1 %again, label %loop, label %done

done:
  ret void
}

attributes #0 = { nounwind }
