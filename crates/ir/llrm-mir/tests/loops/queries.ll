; Hand-derived cases for Loop's invariance, LCSSA, simplify-form and
; reverse-postorder queries; opt has no printer for them.

; %x is defined in the loop and used in the exit without a phi.
define i16 @leaks(i16 %n) {
entry:
  br label %h
h:
  %i = phi i16 [ 0, %entry ], [ %i.next, %h ]
  %x = mul i16 %i, 3
  %i.next = add i16 %i, 1
  %c = icmp ult i16 %i.next, %n
  br i1 %c, label %h, label %exit
exit:
  ret i16 %x
}

; The same, through a phi in the exit.
define i16 @closed(i16 %n) {
entry:
  br label %h
h:
  %i = phi i16 [ 0, %entry ], [ %i.next, %h ]
  %x = mul i16 %i, 3
  %i.next = add i16 %i, 1
  %c = icmp ult i16 %i.next, %n
  br i1 %c, label %h, label %exit
exit:
  %r = phi i16 [ %x, %h ]
  ret i16 %r
}

; %k is defined before the loop, %m and %i in it.
define i16 @invariance(i16 %n) {
entry:
  %k = mul i16 %n, 7
  br label %h
h:
  %i = phi i16 [ 0, %entry ], [ %i.next, %h ]
  %m = add i16 %k, %n
  %j = add i16 %i, %k
  %i.next = add i16 %i, 1
  %c = icmp ult i16 %i.next, %n
  br i1 %c, label %h, label %exit
exit:
  ret i16 %j
}

; h -> a, b -> m -> h or exit.
define void @diamond(i1 %p, i1 %q) {
entry:
  br label %h
h:
  br i1 %p, label %a, label %b
a:
  br label %m
b:
  br label %m
m:
  br i1 %q, label %h, label %exit
exit:
  ret void
}
