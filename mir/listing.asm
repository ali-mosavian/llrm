.386
.model flat

public _main
public _far_a
public _near_a
public _near_b
.data
.data?
    align 16
_far_a label byte
    db 200 dup (?)
    align 16
_near_a label byte
    db 200 dup (?)
    align 16
_near_b label byte
    db 200 dup (?)
extern memcpy_:near
extern report_:near
.code ES_ACROSS_COPY_TEXT
_main proc near
L0_0:
    xor eax, eax
    xor ecx, ecx
L0_30:
    mov word ptr _near_a[ecx*2], cx
    mov word ptr _far_a[ecx*2], ax
    inc ecx
    add eax, 3
    cmp ecx, 100
    jne L0_30
L0_43:
    push ebx
    push esi
    xor esi, esi
    xor ecx, ecx
L0_1:
    mov eax, -200
L0_17:
    movzx edx, word ptr _near_a+200[eax]
    add dx, si
    mov word ptr _near_b+200[eax], dx
    add eax, 2
    jne L0_17
L0_5:
    movsx eax, word ptr _far_a[esi*2]
    add ecx, eax
    movsx eax, word ptr _near_b[esi*2]
    add ecx, eax
    mov eax, offset _near_a
    mov edx, offset _near_b
    mov ebx, 200
    call memcpy_
    inc esi
    cmp esi, 100
    jne L0_1
L0_28:
    mov eax, ecx
    call report_
    xor eax, eax
    pop esi
    pop ebx
    ret
_main endp
end
