; The C library routines a program calls for a copy or a fill the compiler does not move inline (c.toml `libcalls`), in both
; conventions. One copy: the C runtime (ext.asm) and the QB runtime (runtime/qb/dos/m32/memfuncs.asm) both include it.
.code

; void *memcpy(void *dst, const void *src, unsigned long n), regions apart: dst in EAX, src in EDX, n in EBX; dst
; comes back in EAX. A callee that keeps ECX, ESI, EDI and EBP; EDX and EBX are its arguments. The direction flag is
; clear on entry and left so. `rep movsd` for n / 4, `rep movsb` for the rest: n = 0 and n past 64 KiB copy right.
public memcpy_
memcpy_ proc
    push ecx
    push esi
    push edi
    mov edi, eax
    mov esi, edx
    mov ecx, ebx
    shr ecx, 2
    rep movsd
    mov ecx, ebx
    and ecx, 3
    rep movsb
    pop edi
    pop esi
    pop ecx
    ret
memcpy_ endp

; void *memmove(void *dst, const void *src, unsigned long n), regions that may overlap: registers as memcpy_. A copy with
; dst above src inside it runs backwards (dwords from the end, then the head bytes) under the direction flag, which is
; cleared again before the return.
public memmove_
memmove_ proc
    push ecx
    push esi
    push edi
    mov edi, eax
    mov esi, edx
    mov ecx, ebx
    cmp edi, esi
    jbe forward
    lea edx, [esi+ebx]
    cmp edi, edx
    jae forward
    lea esi, [esi+ebx-4]
    lea edi, [edi+ebx-4]
    shr ecx, 2
    std
    rep movsd
    mov ecx, ebx
    and ecx, 3
    add esi, 3
    add edi, 3
    rep movsb
    cld
    jmp done
forward:
    shr ecx, 2
    rep movsd
    mov ecx, ebx
    and ecx, 3
    rep movsb
done:
    pop edi
    pop esi
    pop ecx
    ret
memmove_ endp

; void *memset(void *dst, int c, unsigned long n): dst in EAX, c in EDX (its low byte), n in EBX; dst comes back in EAX.
; ECX, ESI, EDI and EBP are kept.
public memset_
memset_ proc
    push ecx
    push edi
    push eax
    movzx eax, dl
    imul eax, 01010101h
    mov edi, dword ptr [esp]
    mov ecx, ebx
    shr ecx, 2
    rep stosd
    mov ecx, ebx
    and ecx, 3
    rep stosb
    pop eax
    pop edi
    pop ecx
    ret
memset_ endp

; memcpy in the stack convention: dst, src and n on the stack, which the caller removes; dst in EAX; EBX, ESI, EDI and
; EBP kept.
public _memcpy
_memcpy proc
    push esi
    push edi
    mov edi, dword ptr [esp+12]
    mov esi, dword ptr [esp+16]
    mov ecx, dword ptr [esp+20]
    mov edx, ecx
    shr ecx, 2
    rep movsd
    mov ecx, edx
    and ecx, 3
    rep movsb
    mov eax, dword ptr [esp+12]
    pop edi
    pop esi
    ret
_memcpy endp

; memmove and memset in the stack convention: arguments on the stack, which the caller removes; dst in EAX; EBX, ESI, EDI
; and EBP kept.
public _memmove
_memmove proc
    push esi
    push edi
    mov edi, dword ptr [esp+12]
    mov esi, dword ptr [esp+16]
    mov ecx, dword ptr [esp+20]
    cmp edi, esi
    jbe forward
    lea edx, [esi+ecx]
    cmp edi, edx
    jae forward
    mov edx, ecx
    lea esi, [esi+ecx-4]
    lea edi, [edi+ecx-4]
    shr ecx, 2
    std
    rep movsd
    mov ecx, edx
    and ecx, 3
    add esi, 3
    add edi, 3
    rep movsb
    cld
    jmp done
forward:
    mov edx, ecx
    shr ecx, 2
    rep movsd
    mov ecx, edx
    and ecx, 3
    rep movsb
done:
    mov eax, dword ptr [esp+12]
    pop edi
    pop esi
    ret
_memmove endp

public _memset
_memset proc
    push edi
    movzx eax, byte ptr [esp+12]
    imul eax, 01010101h
    mov edi, dword ptr [esp+8]
    mov ecx, dword ptr [esp+16]
    mov edx, ecx
    shr ecx, 2
    rep stosd
    mov ecx, edx
    and ecx, 3
    rep stosb
    mov eax, dword ptr [esp+8]
    pop edi
    ret
_memset endp
