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
	?debug	S "pnagger.c"
	?debug	C E9996A3D5D09706E61676765722E63
	?debug	C E9996A3D5D04706E2E68
	?debug	C E9996A3D5D0863616C6C65722E68
	?debug	C E9996A3D5D0763617365732E68
PNAGGER_TEXT	segment byte public use16 'CODE'
PNAGGER_TEXT	ends
DGROUP	group	_DATA,_BSS
	assume	cs:PNAGGER_TEXT,ds:DGROUP
_DATA	segment word public use16 'DATA'
d@	label	byte
d@w	label	word
_DATA	ends
_BSS	segment word public use16 'BSS'
b@	label	byte
b@w	label	word
_BSS	ends
PNAGGER_TEXT	segment byte public use16 'CODE'
   ;	
   ;	void far call_aggregates(void) {
   ;	
	assume	cs:PNAGGER_TEXT
_call_aggregates	proc	far
   ;	
   ;	    AGGREGATES(CALL)
   ;	
	push	4369
	mov	al,byte ptr DGROUP:_in_s1
	push	ax
	push	8738
	call	near ptr P_S1
	call	near ptr R_S1
	mov	byte ptr DGROUP:_out_s1,al
	push	4369
	push	word ptr DGROUP:_in_s2
	push	8738
	call	near ptr P_S2
	call	near ptr R_S2
	mov	word ptr DGROUP:_out_s2,ax
	push	4369
	mov	ax,offset DGROUP:_in_s3
	mov	dx,ds
	mov	cx,4
	call	far ptr F_SPUSH@
	push	8738
	call	near ptr P_S3
	push	ds
	push	offset DGROUP:_out_s3
	call	near ptr R_S3
	push	4369
	push	dword ptr DGROUP:_in_s4
	push	8738
	call	near ptr P_S4
	call	near ptr R_S4
	shl	eax,16
	shrd	eax,edx,16
	mov	dword ptr DGROUP:_out_s4,eax
	push	4369
	mov	ax,offset DGROUP:_in_s5
	mov	dx,ds
	mov	cx,6
	call	far ptr F_SPUSH@
	push	8738
	call	near ptr P_S5
	push	ds
	push	offset DGROUP:_out_s5
	call	near ptr R_S5
	push	4369
	mov	ax,offset DGROUP:_in_s7
	mov	dx,ds
	mov	cx,8
	call	far ptr F_SPUSH@
	push	8738
	call	near ptr P_S7
	push	ds
	push	offset DGROUP:_out_s7
	call	near ptr R_S7
	push	4369
	mov	ax,offset DGROUP:_in_s8
	mov	dx,ds
	mov	cx,8
	call	far ptr F_SPUSH@
	push	8738
	call	near ptr P_S8
	push	ds
	push	offset DGROUP:_out_s8
	call	near ptr R_S8
	push	4369
	mov	ax,offset DGROUP:_in_s9
	mov	dx,ds
	mov	cx,10
	call	far ptr F_SPUSH@
	push	8738
	call	near ptr P_S9
	push	ds
	push	offset DGROUP:_out_s9
	call	near ptr R_S9
	push	4369
	mov	ax,offset DGROUP:_in_sci
	mov	dx,ds
	mov	cx,4
	call	far ptr F_SPUSH@
	push	8738
	call	near ptr P_SCI
	push	ds
	push	offset DGROUP:_out_sci
	call	near ptr R_SCI
   ;	
   ;	}
   ;	
	ret	
_call_aggregates	endp
PNAGGER_TEXT	ends
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
PNAGGER_TEXT	segment byte public use16 'CODE'
PNAGGER_TEXT	ends
	extrn	F_SPUSH@:far
	public	_call_aggregates
	public	_out_sci
	extrn	R_SCI:near
	extrn	P_SCI:near
	public	_out_s9
	extrn	R_S9:near
	extrn	P_S9:near
	public	_out_s8
	extrn	R_S8:near
	extrn	P_S8:near
	public	_out_s7
	extrn	R_S7:near
	extrn	P_S7:near
	public	_out_s5
	extrn	R_S5:near
	extrn	P_S5:near
	public	_out_s4
	extrn	R_S4:near
	extrn	P_S4:near
	public	_out_s3
	extrn	R_S3:near
	extrn	P_S3:near
	public	_out_s2
	extrn	R_S2:near
	extrn	P_S2:near
	public	_out_s1
	extrn	R_S1:near
	extrn	P_S1:near
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