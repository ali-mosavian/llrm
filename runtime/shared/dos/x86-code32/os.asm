; The operating-system routines std.os declares (os.nib), in cdecl32: arguments in dword slots from
; [esp+4], EBX, ESI and EDI kept, a result in EAX. DOS/32A takes a flat DS:EDX buffer for the file
; calls, so none needs a transfer buffer.
.386
.model flat

public _llrm_os_open
public _llrm_os_create
public _llrm_os_read
public _llrm_os_write_file
public _llrm_os_close
public _llrm_os_exit
public _llrm_os_more
public _llrm_os_stack_low

; HEAP_BYTES comes from os.toml, DOS_* from the OS's facts: the assembler is told both.

.data
; The lowest ESP a checked function may reach (-fsanitize=stack). Start-up sets it.
_llrm_os_stack_low dd 0
; The heap's next free byte and the end of its arena: set on the first _llrm_os_more, which asks the
; extender for the arena.
heap_next dd 0
heap_end dd 0
; The request being served and the arena size being tried, which DPMI 0501h's register use leaves in memory.
heap_ask dd 0
heap_try dd 0

.code
; _llrm_os_more(bytes: usize) -> *near mut u8: `bytes` more of the heap at its end, or 0 when it runs out.
; The heap grows inside an arena taken from the extender (DPMI 0501h, allocate memory block): the first
; call takes HEAP_BYTES, or the largest half of it the extender has, down to the request. When the arena
; cannot hold a request the next is another arena (at least the request), and the grant is at an address
; that need not follow the last one's: the heap chains arenas.
_llrm_os_more proc
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
    mov eax, DOS_DPMI_ALLOC
    int DOS_DPMI_INT
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
_llrm_os_more endp

; The DOS file calls. Each returns what DOS does, or when DOS sets carry, its error code negated.

; _llrm_os_open(name: *far char, mode: u8) -> i16: a handle to an existing file.
_llrm_os_open proc
    mov edx, dword ptr [esp+4]
    mov al, byte ptr [esp+8]
    mov ah, DOS_OPEN
    int DOS_INT
    jmp short checked
_llrm_os_open endp

; _llrm_os_create(name: *far char) -> i16: a handle to a new, empty file.
_llrm_os_create proc
    mov edx, dword ptr [esp+4]
    xor ecx, ecx
    mov ah, DOS_CREATE
    int DOS_INT
    jmp short checked
_llrm_os_create endp

; _llrm_os_read(handle: i16, data: *near mut u8, count: usize) -> isize: bytes read, in one call: DOS/32A takes a
; 32-bit count and returns one. Where DOS sets carry, the error code negated.
_llrm_os_read proc
    mov ah, DOS_READ
    jmp short transfer
_llrm_os_read endp

; _llrm_os_write_file(handle: i16, data: *near u8, count: usize) -> isize: bytes written, as _llrm_os_read.
_llrm_os_write_file proc
    mov ah, DOS_WRITE
transfer::
    push ebx
    movzx ebx, word ptr [esp+8]
    mov edx, dword ptr [esp+12]
    mov ecx, dword ptr [esp+16]
    int DOS_INT
    pop ebx
    jnc short transferred
    movzx eax, ax
    neg eax
transferred:
    ret
_llrm_os_write_file endp

; _llrm_os_close(handle: i16) -> i16
_llrm_os_close proc
    push ebx
    movzx ebx, word ptr [esp+8]
    mov ah, DOS_CLOSE
    int DOS_INT
    pop ebx
    jc short checked
    xor eax, eax                   ; DOS leaves AX undefined on success
checked::
    jnc short done
    neg ax
done:
    ret
_llrm_os_close endp

; _llrm_os_exit(code: u8): ends the program.
_llrm_os_exit proc
    mov al, byte ptr [esp+4]
    mov ah, DOS_EXIT
    int DOS_INT
_llrm_os_exit endp

end
