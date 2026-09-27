; invalid: has a duplicate case
define void @f(i16 %x) {
entry:
  switch i16 %x, label %done [
    i16 1, label %done
    i16 1, label %done
  ]
done:
  ret void
}
