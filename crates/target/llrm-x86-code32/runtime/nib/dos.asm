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

; HEAP_BYTES comes from nib.toml (assembler_defines).

.data
; The lowest ESP a checked function may reach (-fsanitize=stack). Start-up sets it.
N$OSLO dd 0
; The heap's next free byte and the end of its arena: set on the first N$OMEM, which asks the
; extender for the arena.
heap_next dd 0
heap_end dd 0
; The request being served and the arena size being tried, which DPMI 0501h's register use leaves in memory.
heap_ask dd 0
heap_try dd 0

.code
; N$OMEM(bytes: usize) -> *near mut u8: `bytes` more of the heap at its end, or 0 when it runs out.
; The heap grows inside an arena taken from the extender (DPMI 0501h, allocate memory block): the first
; call takes HEAP_BYTES, or the largest half of it the extender has, down to the request. When the arena
; cannot hold a request the next is another arena (at least the request), and the grant is at an address
; that need not follow the last one's: the heap chains arenas.
N$OMEM proc
    mov ecx, dword ptr [esp+4]
    mov eax, heap_next
    test eax, eax
    jz take
    lea edx, [eax + ecx]
    cmp edx, heap_end
    ja take
    mov heap_next, edx
    ret
take:
    push ebx
    push esi
    push edi
    ; DPMI 0501h returns the block's handle in SI:DI, so the request and the size being tried stay in memory.
    mov heap_ask, ecx
    mov edx, HEAP_BYTES
    cmp edx, ecx
    jae sized
    mov edx, ecx
sized:
    mov heap_try, edx
arena:
    mov ebx, heap_try
    shr ebx, 16
    mov ecx, heap_try
    and ecx, 0FFFFh
    mov eax, 0501h
    int 31h
    jnc arena_got
    mov edx, heap_try
    shr edx, 1
    cmp edx, heap_ask
    jb arena_refused
    mov heap_try, edx
    jmp arena
arena_refused:
    pop edi
    pop esi
    pop ebx
    xor eax, eax
    ret
arena_got:
    shl ebx, 16
    mov bx, cx
    mov eax, ebx
    add ebx, heap_try
    mov heap_end, ebx
    mov edx, eax
    add edx, heap_ask
    mov heap_next, edx
    pop edi
    pop esi
    pop ebx
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

; N$OREA(handle: i16, data: *near mut u8, count: usize) -> isize: bytes read, in one call: DOS/32A takes a
; 32-bit count and returns one. Where DOS sets carry, the error code negated.
N$OREA proc
    mov ah, 3Fh
    jmp short transfer
N$OREA endp

; N$OWRI(handle: i16, data: *near u8, count: usize) -> isize: bytes written, as N$OREA.
N$OWRI proc
    mov ah, 40h
transfer::
    push ebx
    movzx ebx, word ptr [esp+8]
    mov edx, dword ptr [esp+12]
    mov ecx, dword ptr [esp+16]
    int 21h
    pop ebx
    jnc short transferred
    movzx eax, ax
    neg eax
transferred:
    ret
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
