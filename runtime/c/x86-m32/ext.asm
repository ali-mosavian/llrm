; The externals the C run tests call, in watcall32, the default convention, so that the tests declare them as
; they would any function: arguments in EAX and EDX, a callee that keeps every register but EAX and its
; argument registers. `report` prints a signed decimal and a newline on standard output, as m16's runtime does.
; The OS layer's `_llrm_os_*` stay in cdecl32, which these call.
.386
.model flat

extrn LL$OPEN:near
extrn LL$READ:near
extrn LL$WRITE_FILE:near
extrn LL$EXIT:near

.data
digits  db 13 dup (?)
inhandle dd 0FFFFFFFFh
inname  db 'DICKENS', 0
stkmsg  db 'Stack Overflow!', 13, 10

public __STKOVERFLOW

.code
; Open Watcom's stack overflow (clib stk086.asm `__STKOVERFLOW`): the message and exit status 1,
; which a checked function enters (-fsanitize=stack). On stdout, which a test captures.
__STKOVERFLOW proc
    push 17
    push offset stkmsg
    push DOS_STDOUT
    call LL$WRITE_FILE
    push 1
    call LL$EXIT
__STKOVERFLOW endp

; void report(long v): v in EAX.
public report_
report_ proc
    push ebx
    push ecx
    push edx
    push edi
    mov edi, offset digits + 13
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
    mov ecx, offset digits + 13
    sub ecx, edi
    push ecx
    push edi
    push DOS_STDOUT
    call LL$WRITE_FILE
    add esp, 12
    pop edi
    pop edx
    pop ecx
    pop ebx
    ret
report_ endp

; int input_read(char *buffer, int count): buffer in EAX, count in EDX. Up to count bytes of DICKENS (opened on
; the first call, in the current directory), 0 at its end or on an error. bench/grep reads its input through this.
public input_read_
input_read_ proc
    push ebx
    push ecx
    push esi
    push edi
    mov esi, eax
    mov edi, edx
    cmp inhandle, 0FFFFFFFFh
    jne opened
    push 0
    push offset inname
    call LL$OPEN
    add esp, 8
    movsx eax, ax
    test eax, eax
    js failed
    mov inhandle, eax
opened:
    push edi
    push esi
    push inhandle
    call LL$READ
    add esp, 12
    test eax, eax
    jns done
failed:
    xor eax, eax
done:
    pop edi
    pop esi
    pop ecx
    pop ebx
    ret
input_read_ endp

; int keep(int x), long keep32(long x): opaque to the optimizer, which cannot see through a call. x is in EAX,
; where the answer goes.
public keep_
keep_ proc
    ret
keep_ endp

public keep32_
keep32_ proc
    ret
keep32_ endp

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

; The same routines for a program in the stack convention (-mabi=sysv): arguments on the stack, which the caller removes.
public _report
_report proc
    mov eax, dword ptr [esp+4]
    jmp report_
_report endp

public _input_read
_input_read proc
    mov eax, dword ptr [esp+4]
    mov edx, dword ptr [esp+8]
    jmp input_read_
_input_read endp

public _keep
_keep proc
    mov eax, dword ptr [esp+4]
    ret
_keep endp

public _keep32
_keep32 proc
    mov eax, dword ptr [esp+4]
    ret
_keep32 endp


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

end
