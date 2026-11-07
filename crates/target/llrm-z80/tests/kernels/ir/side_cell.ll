target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"

; Cell `t[side]` of a table of words, side = a < b ? 1 : 2. The select is 2 + (-1 or 0)
; and the index is shifted by one: `inc hl; inc hl; add hl, hl; add hl, bc`. Folding
; the shift into the add (pin visitSHL) leaves `add hl, hl; add hl, bc; ld bc, #4; add hl, bc`.
define i16 @side_cell(i16 %a, i16 %b, ptr %t) {
  %c = icmp slt i16 %a, %b
  %side = select i1 %c, i16 1, i16 2
  %p = getelementptr i16, ptr %t, i16 %side
  %v = load i16, ptr %p
  ret i16 %v
}
