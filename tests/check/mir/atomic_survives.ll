; RUN: llrm-llc -O2 -S -o /dev/stdout %s
; Atomic accesses go through the pipeline as calls nothing may move: each stays where it is, none merged with another or
; lifted out of the loop, and the unused one is not removed. Two acquire loads of one flag, a release store of `i`, per trip.
target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@flag = global i16 0
@data = global i16 0

define i16 @f(i16 %n) {
entry:
  br label %loop

loop:
  %i = phi i16 [ 0, %entry ], [ %next, %loop ]
  %a = load atomic i16, ptr @flag acquire, align 2
  %b = load atomic i16, ptr @flag acquire, align 2
  store atomic i16 %i, ptr @data release, align 2
  %unused = load atomic i16, ptr @flag acquire, align 2
  %next = add i16 %i, 1
  %more = icmp ult i16 %next, %n
  br i1 %more, label %loop, label %done

done:
  %r = add i16 %a, %b
  ret i16 %r
}

; CHECK-LABEL: _f@3 proc
; CHECK: mov {{.*}}, word ptr _flag
; CHECK-NEXT: mov {{.*}}, word ptr _flag
; CHECK-NEXT: mov word ptr _data, {{.*}}
; CHECK-NEXT: mov {{.*}}, word ptr _flag
