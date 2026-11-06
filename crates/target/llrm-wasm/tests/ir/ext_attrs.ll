declare zeroext i8 @inited()
declare signext i16 @level()
declare void @act(i8 zeroext)

define void @finish() {
  %1 = call zeroext i8 @inited()
  %2 = icmp ne i8 %1, 0
  br i1 %2, label %then, label %done
then:
  call void @act(i8 zeroext 4)
  br label %done
done:
  ret void
}
define i32 @widen(i8 zeroext %a, i16 signext %b) {
  %x = zext i8 %a to i32
  %y = sext i16 %b to i32
  %s = add i32 %x, %y
  %l = call signext i16 @level()
  %z = sext i16 %l to i32
  %r = add i32 %s, %z
  ret i32 %r
}
