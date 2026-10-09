' LEFT$, RIGHT$, MID$, CHR$ and string comparison, in range and past it.
a$ = "Hello, world"
PRINT LEFT$(a$, 5); "|"; RIGHT$(a$, 5); "|"; LEFT$(a$, 99); "|"; RIGHT$(a$, 99)
PRINT "["; LEFT$(a$, 0); "]["; RIGHT$(a$, 0); "]["; MID$(a$, 8, 99); "]["; MID$(a$, 13, 2); "]"
PRINT MID$(a$, 1, 1); MID$(a$, 3, 3); MID$(a$, 12, 1)
PRINT LEFT$(a$ + "!", 3); RIGHT$(LEFT$(a$ + "?", 8), 3); CHR$(65); CHR$(0 + 255) = CHR$(255)
b$ = "abc": c$ = "abd": d$ = "ab"
PRINT b$ < c$; b$ > c$; b$ = b$; d$ < b$; b$ <> d$; "" < b$; "" = ""; CHR$(200) > "a"
PRINT b$ + c$ < c$ + b$; LEFT$(c$, 2) = d$
FOR i% = 1 TO 5: PRINT MID$("abcdef", i%, i%); : NEXT: PRINT
