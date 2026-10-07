.386
.model flat, c
point struc
  px dd ?
  py dd ?
point ends
.data
count dd 0
pt point <1,2>
.code
f proc a:dword, b:dword
    local x:dword
    local q:point
    mov eax, a
    add eax, b
    mov x, eax
    mov count, eax
    ret
f endp
end
