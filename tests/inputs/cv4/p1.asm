.model small
.386
point struc
  px dw ?
  py dw ?
point ends
.data
count dw 0
pt point <1,2>
pp dw offset pt
arr dw 4 dup (0)
big dd 0
.code
f proc near c, a:word, b:word
    local x:word
    local q:point
    mov ax, a
    add ax, b
    mov x, ax
    mov count, ax
    ret
f endp
end
