	.area _CODE
	.globl _lz4c
_lz4c::
	push ix
	ld ix, #0
	add ix, sp
	ld iy, #-12
	add iy, sp
	ld sp, iy
L0_0:
	ld -4(ix), l
	ld -3(ix), h
	ld l, e
	ld h, d
	add hl, bc
	ld -6(ix), l
	ld -5(ix), h
	ld a, b
	or a, c
	jr nz, L0_66
L0_72:
	ld de, #0
	jp L0_3
L0_66:
	ld -8(ix), e
	ld -7(ix), d
	ld -10(ix), e
	ld -9(ix), d
L0_8:
	ld e, -4(ix)
	ld d, -3(ix)
	ld c, e
	ld b, d
	inc bc
	ld a, (de)
	ld d, a
	srl d
	srl d
	srl d
	srl d
	ld l, d
	ld h, #0
	ld -4(ix), l
	ld -3(ix), h
	and a, #15
	add a, #4
	ld -1(ix), a
	ld a, d
	cp a, #15
	jr nz, L0_89
L0_64:
	ld hl, #15
L0_9:
	ld e, c
	ld d, b
	inc bc
	ld a, (de)
	ld e, a
	ld d, #0
	add hl, de
	cp a, #255
	jr z, L0_9
L0_77:
	ld -4(ix), l
	ld -3(ix), h
L0_89:
	ld l, -10(ix)
	ld h, -9(ix)
	ld e, -4(ix)
	ld d, -3(ix)
	ld a, d
	or a, e
	jr z, L0_12
L0_11:
	ld e, c
	ld d, b
	ld c, -4(ix)
	ld b, -3(ix)
	ex de, hl
	ldir
	ld c, l
	ld b, h
	ex de, hl
L0_12:
	ld e, -6(ix)
	ld d, -5(ix)
	ld a, l
	sub a, e
	ld a, h
	sbc a, d
	jr c, L0_13
L0_81:
	ld c, -8(ix)
	ld b, -7(ix)
L0_5:
	ld e, l
	ld d, h
	or a, a
	sbc hl, bc
	ld c, l
	ld b, h
	ld l, -8(ix)
	ld h, -7(ix)
	ld a, e
	sub a, l
	jr nz, 100$
	ld a, d
	sub a, h
100$:
	jp nz, L0_59
L0_88:
	ld de, #0
	jp L0_3
L0_13:
	ld e, c
	ld d, b
	inc de
	ld -10(ix), l
	ld -9(ix), h
	ld l, c
	ld h, b
	ld c, (hl)
	ld b, #0
	inc hl
	inc hl
	ld -4(ix), l
	ld -3(ix), h
	xor a, a
	ex de, hl
	sub a, (hl)
	ld l, #0
	ld h, a
	or a, a
	sbc hl, bc
	ld e, -10(ix)
	ld d, -9(ix)
	add hl, de
	ld a, -1(ix)
	ld c, a
	ld b, #0
	cp a, #19
	ex de, hl
	jr z, L0_61
L0_82:
	ld l, -10(ix)
	ld h, -9(ix)
L0_17:
	ex de, hl
	ldir
	ex de, hl
	ld c, -8(ix)
	ld b, -7(ix)
	jp L0_7
L0_61:
	ld -12(ix), e
	ld -11(ix), d
	ld bc, #19
L0_14:
	ld e, -4(ix)
	ld d, -3(ix)
	ld l, e
	ld h, d
	inc hl
	ld -4(ix), l
	ld -3(ix), h
	ld a, (de)
	ld e, a
	ld d, #0
	ld l, c
	ld h, b
	add hl, de
	cp a, #255
	ld c, l
	ld b, h
	jr z, L0_14
L0_15:
	ld a, b
	or a, c
	jr z, L0_84
L0_85:
	ld l, -10(ix)
	ld h, -9(ix)
	pop de
	push de
	jp L0_17
L0_84:
	ld c, -8(ix)
	ld b, -7(ix)
	ld l, -10(ix)
	ld h, -9(ix)
L0_7:
	ld e, -6(ix)
	ld d, -5(ix)
	ld a, l
	sub a, e
	ld a, h
	sbc a, d
	jp nc, L0_5
L0_73:
	ld -10(ix), l
	ld -9(ix), h
	jp L0_8
L0_59:
	ld d, #0
	ld e, d
L0_2:
	ld a, e
	add a, (hl)
	ld e, a
	add a, d
	ld d, a
	dec bc
	inc hl
	ld a, b
	or a, c
	jr nz, L0_2
L0_30:
L0_3:
	ld sp, ix
	pop ix
	ret
