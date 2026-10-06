; The externals the C run tests call, in cdecl32: `report` prints a signed decimal and a
; newline on stdout (handle 1), as code16's runtime does.
.386
.model flat

.data
digits  db 12 dup (?)

.code
; void report(long v)
public _report
_report proc
    push ebp
    mov ebp, esp
    push ebx
    push edi
    mov eax, dword ptr [ebp+8]
    mov edi, offset digits + 12
    mov byte ptr [edi-1], 10
    mov byte ptr [edi-2], 13
    sub edi, 2
    xor ecx, ecx
    test eax, eax
    jns positive
    neg eax
    inc ecx
positive:
    mov ebx, 10
more:
    xor edx, edx
    div ebx
    add dl, '0'
    dec edi
    mov [edi], dl
    test eax, eax
    jnz more
    test ecx, ecx
    jz unsigned
    dec edi
    mov byte ptr [edi], '-'
unsigned:
    mov edx, edi
    mov ecx, offset digits + 12
    sub ecx, edi
    mov ebx, 1
    mov ah, 40h
    int 21h
    pop edi
    pop ebx
    pop ebp
    ret
_report endp

end
