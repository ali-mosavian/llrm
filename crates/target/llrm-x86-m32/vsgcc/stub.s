.text
.globl report, _report, input_read, _input_read, report_, input_read_
report:
_report:
input_read:
_input_read:
 ret
# llrm's default convention takes the value in EAX, and the harness reads it there: a place of its own
report_:
input_read_:
 ret
# libc stand-ins: dword string ops, then the tail bytes
.globl memset, memcpy, memmove, memcpy_, memmove_, memset_
memset:
 push %edi
 mov 8(%esp),%edi
 movzbl 12(%esp),%eax
 imul $0x01010101,%eax,%eax
 mov 16(%esp),%ecx
 mov %ecx,%edx
 shr $2,%ecx
 rep stosl
 mov %edx,%ecx
 and $3,%ecx
 rep stosb
 mov 8(%esp),%eax
 pop %edi
 ret
memcpy:
memmove:
 push %esi
 push %edi
 mov 12(%esp),%edi
 mov 16(%esp),%esi
 mov 20(%esp),%ecx
 cmp %esi,%edi
 jbe 1f
 lea (%esi,%ecx),%eax
 cmp %eax,%edi
 jae 1f
 # overlapping with dst above src: copy backwards, dwords from the end, then the head bytes
 std
 lea -4(%esi,%ecx),%esi
 lea -4(%edi,%ecx),%edi
 mov %ecx,%edx
 shr $2,%ecx
 rep movsl
 mov %edx,%ecx
 and $3,%ecx
 add $3,%esi
 add $3,%edi
 rep movsb
 cld
 jmp 2f
1:
 mov %ecx,%edx
 shr $2,%ecx
 rep movsl
 mov %edx,%ecx
 and $3,%ecx
 rep movsb
2:
 mov 12(%esp),%eax
 pop %edi
 pop %esi
 ret
# the pinned clang turns a strlen loop into a call: byte loop, as the source writes it
.globl strlen
strlen:
 mov 4(%esp),%eax
1:
 cmpb $0,(%eax)
 lea 1(%eax),%eax
 jne 1b
 dec %eax
 sub 4(%esp),%eax
 ret
# memcpy in llrm's default convention: dst in EAX, src in EDX, n in EBX, dst back in EAX; ECX, ESI, EDI and EBP kept
memcpy_:
 push %ecx
 push %esi
 push %edi
 mov %eax,%edi
 mov %edx,%esi
 mov %ebx,%ecx
 shr $2,%ecx
 rep movsl
 mov %ebx,%ecx
 and $3,%ecx
 rep movsb
 pop %edi
 pop %esi
 pop %ecx
 ret
# memmove and memset in llrm's default convention, as memcpy_: dst in EAX, src (or the byte) in EDX, n in EBX; dst back in EAX
memmove_:
 push %ecx
 push %esi
 push %edi
 mov %eax,%edi
 mov %edx,%esi
 mov %ebx,%ecx
 cmp %esi,%edi
 jbe 1f
 lea (%esi,%ebx),%edx
 cmp %edx,%edi
 jae 1f
 lea -4(%esi,%ebx),%esi
 lea -4(%edi,%ebx),%edi
 shr $2,%ecx
 std
 rep movsl
 mov %ebx,%ecx
 and $3,%ecx
 add $3,%esi
 add $3,%edi
 rep movsb
 cld
 jmp 2f
1:
 shr $2,%ecx
 rep movsl
 mov %ebx,%ecx
 and $3,%ecx
 rep movsb
2:
 pop %edi
 pop %esi
 pop %ecx
 ret
memset_:
 push %ecx
 push %edi
 push %eax
 movzbl %dl,%eax
 imul $0x01010101,%eax,%eax
 mov (%esp),%edi
 mov %ebx,%ecx
 shr $2,%ecx
 rep stosl
 mov %ebx,%ecx
 and $3,%ecx
 rep stosb
 pop %eax
 pop %edi
 pop %ecx
 ret
