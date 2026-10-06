; The operating-system routines std.os declares (os.nib), in cdecl32: arguments in dword slots from
; [esp+4], EBX, ESI and EDI kept, a result in EAX. DOS/32A takes a flat DS:EDX buffer for the file
; calls, so none needs a transfer buffer.
.386
.model flat

public N$OOPN
public N$OCRE
public N$OREA
public N$OWRI
public N$OCLO
public N$OEXT
public N$OMEM
public N$OSLO

HEAP_BYTES equ 1048576

.data
; The lowest ESP a checked function may reach (-fsanitize=stack). Start-up sets it.
N$OSLO dd 0
; The heap's next free byte: set on the first N$OMEM.
heap_next dd 0

.data?
heap_arena db HEAP_BYTES dup (?)

.code
; N$OMEM(bytes: usize) -> *near mut u8: `bytes` more of the heap at its end, or 0 when it runs out.
N$OMEM proc
    mov ecx, dword ptr [esp+4]
    mov eax, heap_next
    test eax, eax
    jnz started
    mov eax, offset heap_arena
started:
    lea edx, [eax + ecx]
    cmp edx, offset heap_arena + HEAP_BYTES
    ja refused
    mov heap_next, edx
    ret
refused:
    xor eax, eax
    ret
N$OMEM endp

; The DOS file calls. Each returns what DOS does, or when DOS sets carry, its error code negated.

; N$OOPN(name: *far char, mode: u8) -> i16: a handle to an existing file.
N$OOPN proc
    mov edx, dword ptr [esp+4]
    mov al, byte ptr [esp+8]
    mov ah, 3Dh
    int 21h
    jmp short checked
N$OOPN endp

; N$OCRE(name: *far char) -> i16: a handle to a new, empty file.
N$OCRE proc
    mov edx, dword ptr [esp+4]
    xor ecx, ecx
    mov ah, 3Ch
    int 21h
    jmp short checked
N$OCRE endp

; N$OREA(handle: i16, data: *far mut u8, count: u16) -> i16: bytes read.
N$OREA proc
    mov ah, 3Fh
    jmp short transfer
N$OREA endp

; N$OWRI(handle: i16, data: *far u8, count: u16) -> i16: bytes written.
N$OWRI proc
    mov ah, 40h
transfer::
    push ebx
    movzx ebx, word ptr [esp+8]
    mov edx, dword ptr [esp+12]
    movzx ecx, word ptr [esp+16]
    int 21h
    pop ebx
    jmp short checked
N$OWRI endp

; N$OCLO(handle: i16) -> i16
N$OCLO proc
    push ebx
    movzx ebx, word ptr [esp+8]
    mov ah, 3Eh
    int 21h
    pop ebx
checked::
    jnc short done
    neg ax
done:
    ret
N$OCLO endp

; N$OEXT(code: u8): ends the program.
N$OEXT proc
    mov al, byte ptr [esp+4]
    mov ah, 4Ch
    int 21h
N$OEXT endp

end
