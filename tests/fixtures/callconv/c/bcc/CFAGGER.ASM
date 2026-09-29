	.386p
	.387
	ifndef	??version
?debug	macro
	endm
publicdll macro	name
	public	name
	endm
$comm	macro	name,dist,size,count
	comm	dist name:BYTE:count*size
	endm
	else
$comm	macro	name,dist,size,count
	comm	dist name[size]:BYTE:count
	endm
	endif
	?debug	V 300h
	?debug	S "cfagger.c"
	?debug	C E992603D5D09636661676765722E63
	?debug	C E992603D5D0463662E68
	?debug	C E992603D5D0863616C6C65722E68
	?debug	C E992603D5D0763617365732E68
CFAGGER_TEXT	segment byte public use16 'CODE'
CFAGGER_TEXT	ends
DGROUP	group	_DATA,_BSS
	assume	cs:CFAGGER_TEXT,ds:DGROUP
_DATA	segment word public use16 'DATA'
d@	label	byte
d@w	label	word
_DATA	ends
_BSS	segment word public use16 'BSS'
b@	label	byte
b@w	label	word
_BSS	ends
CFAGGER_TEXT	segment byte public use16 'CODE'
   ;	
   ;	void far call_aggregates(void) {
   ;	
	assume	cs:CFAGGER_TEXT
_call_aggregates	proc	far
   ;	
   ;	    AGGREGATES(CALL)
   ;	
	push	8738
	mov	al,byte ptr DGROUP:_in_s1
	push	ax
	push	4369
	call	far ptr _p_s1
	add	sp,6
	call	far ptr _r_s1
	mov	byte ptr DGROUP:_out_s1,al
	push	8738
	push	word ptr DGROUP:_in_s2
	push	4369
	call	far ptr _p_s2
	add	sp,6
	call	far ptr _r_s2
	mov	word ptr DGROUP:_out_s2,ax
	push	8738
	mov	ax,offset DGROUP:_in_s3
	mov	dx,ds
	mov	cx,4
	call	far ptr F_SPUSH@
	push	4369
	call	far ptr _p_s3
	add	sp,8
	push	ds
	push	offset DGROUP:_out_s3
	call	far ptr _r_s3
	add	sp,4
	push	8738
	push	dword ptr DGROUP:_in_s4
	push	4369
	call	far ptr _p_s4
	add	sp,8
	call	far ptr _r_s4
	shl	eax,16
	shrd	eax,edx,16
	mov	dword ptr DGROUP:_out_s4,eax
	push	8738
	mov	ax,offset DGROUP:_in_s5
	mov	dx,ds
	mov	cx,6
	call	far ptr F_SPUSH@
	push	4369
	call	far ptr _p_s5
	add	sp,10
	push	ds
	push	offset DGROUP:_out_s5
	call	far ptr _r_s5
	add	sp,4
	push	8738
	mov	ax,offset DGROUP:_in_s7
	mov	dx,ds
	mov	cx,8
	call	far ptr F_SPUSH@
	push	4369
	call	far ptr _p_s7
	add	sp,12
	push	ds
	push	offset DGROUP:_out_s7
	call	far ptr _r_s7
	add	sp,4
	push	8738
	mov	ax,offset DGROUP:_in_s8
	mov	dx,ds
	mov	cx,8
	call	far ptr F_SPUSH@
	push	4369
	call	far ptr _p_s8
	add	sp,12
	push	ds
	push	offset DGROUP:_out_s8
	call	far ptr _r_s8
	add	sp,4
	push	8738
	mov	ax,offset DGROUP:_in_s9
	mov	dx,ds
	mov	cx,10
	call	far ptr F_SPUSH@
	push	4369
	call	far ptr _p_s9
	add	sp,14
	push	ds
	push	offset DGROUP:_out_s9
	call	far ptr _r_s9
	add	sp,4
	push	8738
	mov	ax,offset DGROUP:_in_sci
	mov	dx,ds
	mov	cx,4
	call	far ptr F_SPUSH@
	push	4369
	call	far ptr _p_sci
	add	sp,8
	push	ds
	push	offset DGROUP:_out_sci
	call	far ptr _r_sci
	add	sp,4
   ;	
   ;	}
   ;	
	ret	
_call_aggregates	endp
CFAGGER_TEXT	ends
_BSS	segment word public use16 'BSS'
_out_sci	label	word
	db	3 dup (?)
_out_s9	label	word
	db	9 dup (?)
_out_s8	label	word
	db	8 dup (?)
_out_s7	label	word
	db	7 dup (?)
_out_s5	label	word
	db	5 dup (?)
_out_s4	label	word
	db	4 dup (?)
_out_s3	label	word
	db	3 dup (?)
_out_s2	label	word
	db	2 dup (?)
_out_s1	label	word
	db	1 dup (?)
	?debug	C E9
	?debug	C FA00000000
_BSS	ends
_DATA	segment word public use16 'DATA'
s@	label	byte
_DATA	ends
CFAGGER_TEXT	segment byte public use16 'CODE'
CFAGGER_TEXT	ends
	extrn	F_SPUSH@:far
	public	_call_aggregates
	public	_out_sci
	extrn	_r_sci:far
	extrn	_p_sci:far
	public	_out_s9
	extrn	_r_s9:far
	extrn	_p_s9:far
	public	_out_s8
	extrn	_r_s8:far
	extrn	_p_s8:far
	public	_out_s7
	extrn	_r_s7:far
	extrn	_p_s7:far
	public	_out_s5
	extrn	_r_s5:far
	extrn	_p_s5:far
	public	_out_s4
	extrn	_r_s4:far
	extrn	_p_s4:far
	public	_out_s3
	extrn	_r_s3:far
	extrn	_p_s3:far
	public	_out_s2
	extrn	_r_s2:far
	extrn	_p_s2:far
	public	_out_s1
	extrn	_r_s1:far
	extrn	_p_s1:far
	extrn	_in_sci:word
	extrn	_in_s9:word
	extrn	_in_s8:word
	extrn	_in_s7:word
	extrn	_in_s5:word
	extrn	_in_s4:word
	extrn	_in_s3:word
	extrn	_in_s2:word
	extrn	_in_s1:word
_s@	equ	s@
	end
