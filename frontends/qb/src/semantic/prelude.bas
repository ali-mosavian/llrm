' QuickrBASIC's f-string support, compiled into the programs that use
' f-strings. Every name here begins QUICKR_, which programs cannot use.
' The compiler parses each format spec; these only lay the value out.

' The digits of a whole, non-negative value below 2 ^ 53 in base 2 to 16.
FUNCTION QUICKR_RADIX$ (BYVAL value AS DOUBLE, BYVAL radix AS INTEGER, BYVAL upper AS INTEGER)
    DIM digits AS STRING, alphabet AS STRING, rest AS DOUBLE, quotient AS DOUBLE
    alphabet = "0123456789abcdef"
    IF upper THEN alphabet = UCASE$(alphabet)
    rest = INT(value)
    digits = ""
    DO
        quotient = INT(rest / radix)
        digits = MID$(alphabet, CINT(rest - quotient * radix) + 1, 1) + digits
        rest = quotient
    LOOP WHILE rest > 0
    QUICKR_RADIX$ = digits
END FUNCTION

' Decimal `digits` times a small `factor`.
FUNCTION QUICKR_TIMES$ (digits AS STRING, BYVAL factor AS INTEGER)
    DIM product AS STRING, at AS INTEGER, carry AS INTEGER
    product = ""
    carry = 0
    FOR at = LEN(digits) TO 1 STEP -1
        carry = carry + factor * (ASC(MID$(digits, at, 1)) - 48)
        product = CHR$(48 + carry MOD 10) + product
        carry = carry \ 10
    NEXT
    IF carry > 0 THEN product = LTRIM$(STR$(carry)) + product
    QUICKR_TIMES$ = product
END FUNCTION

' Decimal `digits` plus one.
FUNCTION QUICKR_INCREMENT$ (digits AS STRING)
    DIM at AS INTEGER
    at = LEN(digits)
    DO WHILE at > 0
        IF MID$(digits, at, 1) <> "9" THEN EXIT DO
        at = at - 1
    LOOP
    IF at = 0 THEN
        QUICKR_INCREMENT$ = "1" + STRING$(LEN(digits), "0")
    ELSE
        QUICKR_INCREMENT$ = LEFT$(digits, at - 1) + CHR$(ASC(MID$(digits, at, 1)) + 1) + STRING$(LEN(digits) - at, "0")
    END IF
END FUNCTION

' The exact decimal digits of a positive value, with `dot` set to how
' many of them come before the decimal point. A double is m * 2 ^ e with
' m below 2 ^ 53, and halving or doubling it to find m is exact.
FUNCTION QUICKR_EXACT$ (BYVAL value AS DOUBLE, dot AS INTEGER)
    DIM mantissa AS DOUBLE, twos AS INTEGER, digits AS STRING, at AS INTEGER
    mantissa = value
    twos = 0
    DO WHILE mantissa >= 9007199254740992#
        mantissa = mantissa / 2
        twos = twos + 1
    LOOP
    DO WHILE mantissa <> INT(mantissa)
        mantissa = mantissa * 2
        twos = twos - 1
    LOOP
    digits = QUICKR_RADIX$(mantissa, 10, 0)
    FOR at = 1 TO twos
        digits = QUICKR_TIMES$(digits, 2)
    NEXT
    dot = LEN(digits)
    ' m / 2 ^ k is m * 5 ^ k / 10 ^ k.
    FOR at = 1 TO -twos
        digits = QUICKR_TIMES$(digits, 5)
    NEXT
    IF twos < 0 THEN dot = LEN(digits) + twos
    QUICKR_EXACT$ = digits
END FUNCTION

' The first `keep` of exact `digits`, rounded half to even on the rest.
' A carry adds a digit in front and moves `dot` one right.
FUNCTION QUICKR_CUT$ (digits AS STRING, dot AS INTEGER, BYVAL keep AS INTEGER)
    DIM head AS STRING, rest AS STRING, up AS INTEGER, last AS STRING
    IF keep < 0 THEN
        QUICKR_CUT$ = ""
        EXIT FUNCTION
    END IF
    IF LEN(digits) <= keep THEN
        QUICKR_CUT$ = digits + STRING$(keep - LEN(digits), "0")
        EXIT FUNCTION
    END IF
    head = LEFT$(digits, keep)
    rest = MID$(digits, keep + 1)
    up = 0
    IF LEFT$(rest, 1) > "5" THEN
        up = -1
    ELSEIF LEFT$(rest, 1) = "5" THEN
        IF MID$(rest, 2) <> STRING$(LEN(rest) - 1, "0") THEN
            up = -1
        ELSE
            last = RIGHT$("0" + head, 1)
            up = INSTR("13579", last) > 0
        END IF
    END IF
    IF up THEN
        head = QUICKR_INCREMENT$(head)
        IF LEN(head) > keep THEN dot = dot + 1
    END IF
    QUICKR_CUT$ = head
END FUNCTION

' A non-negative value times 10 ^ shift, with `precision` decimals.
FUNCTION QUICKR_FIXED$ (BYVAL value AS DOUBLE, BYVAL precision AS INTEGER, BYVAL shift AS INTEGER)
    DIM digits AS STRING, dot AS INTEGER, whole AS STRING, fraction AS STRING
    IF value = 0 THEN
        digits = ""
        dot = 0
    ELSE
        digits = QUICKR_EXACT$(value, dot)
        dot = dot + shift
        digits = QUICKR_CUT$(digits, dot, dot + precision)
    END IF
    IF digits = "" THEN dot = -precision
    IF dot <= 0 THEN
        whole = "0"
        fraction = STRING$(-dot, "0") + digits
    ELSE
        whole = LEFT$(digits, dot)
        fraction = MID$(digits, dot + 1)
    END IF
    IF precision > 0 THEN whole = whole + "." + LEFT$(fraction + STRING$(precision, "0"), precision)
    QUICKR_FIXED$ = whole
END FUNCTION

' The `count` significant digits of a positive value, rounded, and its
' decimal exponent in `power`.
FUNCTION QUICKR_SIGNIFICANT$ (BYVAL value AS DOUBLE, BYVAL count AS INTEGER, power AS INTEGER)
    DIM digits AS STRING, dot AS INTEGER
    digits = QUICKR_CUT$(QUICKR_EXACT$(value, dot), dot, count)
    power = dot - 1
    QUICKR_SIGNIFICANT$ = LEFT$(digits, count)
END FUNCTION

' An exponent as Python writes one: a sign and at least two digits.
FUNCTION QUICKR_POWER$ (BYVAL power AS INTEGER, BYVAL upper AS INTEGER)
    DIM text AS STRING
    text = "e"
    IF upper THEN text = "E"
    IF power < 0 THEN text = text + "-" ELSE text = text + "+"
    QUICKR_POWER$ = text + RIGHT$("0" + LTRIM$(STR$(ABS(power))), 2 - (ABS(power) >= 100))
END FUNCTION

' A non-negative value as d.ddd, `precision` decimals, and an exponent.
FUNCTION QUICKR_EXPONENT$ (BYVAL value AS DOUBLE, BYVAL precision AS INTEGER, BYVAL upper AS INTEGER)
    DIM digits AS STRING, power AS INTEGER, mantissa AS STRING
    power = 0
    digits = STRING$(precision + 1, "0")
    IF value > 0 THEN digits = QUICKR_SIGNIFICANT$(value, precision + 1, power)
    mantissa = LEFT$(digits, 1)
    IF precision > 0 THEN mantissa = mantissa + "." + MID$(digits, 2)
    QUICKR_EXPONENT$ = mantissa + QUICKR_POWER$(power, upper)
END FUNCTION

' Trailing zeros of a decimal fraction, and then a bare point, dropped.
FUNCTION QUICKR_TRIM$ (digits AS STRING)
    DIM mantissa AS STRING, exponent AS STRING, at AS INTEGER
    at = INSTR(UCASE$(digits), "E")
    IF at = 0 THEN at = LEN(digits) + 1
    mantissa = LEFT$(digits, at - 1)
    exponent = MID$(digits, at)
    IF INSTR(mantissa, ".") > 0 THEN
        DO WHILE RIGHT$(mantissa, 1) = "0"
            mantissa = LEFT$(mantissa, LEN(mantissa) - 1)
        LOOP
        IF RIGHT$(mantissa, 1) = "." THEN mantissa = LEFT$(mantissa, LEN(mantissa) - 1)
    END IF
    QUICKR_TRIM$ = mantissa + exponent
END FUNCTION

' `digits` with a point before any exponent, if it has none.
FUNCTION QUICKR_POINTED$ (digits AS STRING)
    DIM at AS INTEGER
    at = INSTR(UCASE$(digits), "E")
    IF at = 0 THEN at = LEN(digits) + 1
    IF INSTR(digits, ".") = 0 THEN
        QUICKR_POINTED$ = LEFT$(digits, at - 1) + "." + MID$(digits, at)
    ELSE
        QUICKR_POINTED$ = digits
    END IF
END FUNCTION

' 'g' and a float's bare precision: `precision` significant digits, fixed
' while the exponent is in [-4, limit), else exponent form. `plain` is
' the bare form, which keeps a digit after the point.
FUNCTION QUICKR_GENERAL$ (BYVAL value AS DOUBLE, BYVAL precision AS INTEGER, BYVAL upper AS INTEGER, BYVAL alternate AS INTEGER, BYVAL plain AS INTEGER)
    DIM digits AS STRING, power AS INTEGER, limit AS INTEGER
    IF precision = 0 THEN precision = 1
    power = 0
    IF value > 0 THEN digits = QUICKR_SIGNIFICANT$(value, precision, power)
    limit = precision
    IF plain THEN limit = precision - 1
    IF power >= -4 AND power < limit THEN
        digits = QUICKR_FIXED$(value, precision - 1 - power, 0)
    ELSE
        digits = QUICKR_EXPONENT$(value, precision - 1, upper)
    END IF
    IF alternate THEN
        digits = QUICKR_POINTED$(digits)
    ELSE
        digits = QUICKR_TRIM$(digits)
        IF plain AND INSTR(UCASE$(digits), "E") = 0 AND INSTR(digits, ".") = 0 THEN digits = digits + ".0"
    END IF
    QUICKR_GENERAL$ = digits
END FUNCTION

' A float as Python's repr() writes one: the fewest digits that read back
' as the same value (a SINGLE's, when `narrow`), fixed while the exponent
' is in [-4, 16), always with a point.
FUNCTION QUICKR_REPR$ (BYVAL value AS DOUBLE, BYVAL narrow AS INTEGER)
    DIM magnitude AS DOUBLE, digits AS STRING, count AS INTEGER, power AS INTEGER, text AS STRING, back AS DOUBLE
    magnitude = ABS(value)
    power = 0
    IF magnitude = 0 THEN
        text = "0.0"
    ELSE
        FOR count = 1 TO 17
            digits = QUICKR_SIGNIFICANT$(magnitude, count, power)
            back = VAL(digits + "E" + LTRIM$(STR$(power - count + 1)))
            IF narrow THEN
                IF CSNG(back) = CSNG(magnitude) THEN EXIT FOR
            ELSEIF back = magnitude THEN
                EXIT FOR
            END IF
        NEXT
        DO WHILE LEN(digits) > 1 AND RIGHT$(digits, 1) = "0"
            digits = LEFT$(digits, LEN(digits) - 1)
        LOOP
        IF power >= 16 OR power < -4 THEN
            text = LEFT$(digits, 1)
            IF LEN(digits) > 1 THEN text = text + "." + MID$(digits, 2)
            text = text + QUICKR_POWER$(power, 0)
        ELSEIF power < 0 THEN
            text = "0." + STRING$(-power - 1, "0") + digits
        ELSEIF LEN(digits) > power + 1 THEN
            text = LEFT$(digits, power + 1) + "." + MID$(digits, power + 2)
        ELSE
            text = digits + STRING$(power + 1 - LEN(digits), "0") + ".0"
        END IF
    END IF
    IF value < 0 THEN text = "-" + text
    QUICKR_REPR$ = text
END FUNCTION

' `digits` with `separator` between each group of `every`, from the right.
FUNCTION QUICKR_GROUP$ (digits AS STRING, separator AS STRING, BYVAL every AS INTEGER)
    DIM grouped AS STRING, rest AS STRING
    rest = digits
    grouped = ""
    DO WHILE LEN(rest) > every
        grouped = separator + RIGHT$(rest, every) + grouped
        rest = LEFT$(rest, LEN(rest) - every)
    LOOP
    QUICKR_GROUP$ = rest + grouped
END FUNCTION

' `lead` (sign and prefix) and `body`, grouped and filled out to `wide`.
FUNCTION QUICKR_PAD$ (lead AS STRING, body AS STRING, fill AS STRING, align AS STRING, BYVAL wide AS INTEGER, separator AS STRING, BYVAL every AS INTEGER)
    DIM digits AS STRING, rest AS STRING, grouped AS STRING, at AS INTEGER, missing AS INTEGER, before AS INTEGER
    at = LEN(body) + 1
    IF every = 3 THEN
        at = 1
        DO WHILE at <= LEN(body)
            IF INSTR("0123456789", MID$(body, at, 1)) = 0 THEN EXIT DO
            at = at + 1
        LOOP
    END IF
    digits = LEFT$(body, at - 1)
    rest = MID$(body, at)
    grouped = body
    IF separator <> "" THEN
        grouped = QUICKR_GROUP$(digits, separator, every) + rest
        IF align = "=" AND fill = "0" THEN
            DO WHILE LEN(lead) + LEN(grouped) < wide
                digits = "0" + digits
                grouped = QUICKR_GROUP$(digits, separator, every) + rest
            LOOP
        END IF
    END IF
    missing = wide - LEN(lead) - LEN(grouped)
    IF missing <= 0 THEN
        QUICKR_PAD$ = lead + grouped
    ELSEIF align = "<" THEN
        QUICKR_PAD$ = lead + grouped + STRING$(missing, fill)
    ELSEIF align = "^" THEN
        before = missing \ 2
        QUICKR_PAD$ = STRING$(before, fill) + lead + grouped + STRING$(missing - before, fill)
    ELSEIF align = "=" THEN
        QUICKR_PAD$ = lead + STRING$(missing, fill) + grouped
    ELSE
        QUICKR_PAD$ = STRING$(missing, fill) + lead + grouped
    END IF
END FUNCTION

' The sign a number shows: its minus, or what the spec asks of the others.
FUNCTION QUICKR_SIGN$ (BYVAL negative AS INTEGER, sign AS STRING)
    IF negative THEN
        QUICKR_SIGN$ = "-"
    ELSEIF sign = "-" THEN
        QUICKR_SIGN$ = ""
    ELSE
        QUICKR_SIGN$ = sign
    END IF
END FUNCTION

' A number under a spec with a presentation type ("" is a float's bare
' precision). Precision is already defaulted.
FUNCTION QUICKR_FORMAT$ (BYVAL value AS DOUBLE, kind AS STRING, sign AS STRING, BYVAL alternate AS INTEGER, BYVAL zeroless AS INTEGER, fill AS STRING, align AS STRING, BYVAL wide AS INTEGER, separator AS STRING, BYVAL precision AS INTEGER)
    DIM negative AS INTEGER, magnitude AS DOUBLE, body AS STRING, prefix AS STRING, every AS INTEGER
    negative = value < 0
    magnitude = ABS(value)
    prefix = ""
    every = 3
    IF kind = "b" THEN
        body = QUICKR_RADIX$(magnitude, 2, 0)
        prefix = "0b"
        every = 4
    ELSEIF kind = "o" THEN
        body = QUICKR_RADIX$(magnitude, 8, 0)
        prefix = "0o"
        every = 4
    ELSEIF kind = "x" THEN
        body = QUICKR_RADIX$(magnitude, 16, 0)
        prefix = "0x"
        every = 4
    ELSEIF kind = "X" THEN
        body = QUICKR_RADIX$(magnitude, 16, -1)
        prefix = "0X"
        every = 4
    ELSEIF kind = "c" THEN
        body = CHR$(CINT(value))
        negative = 0
    ELSEIF kind = "d" THEN
        body = QUICKR_FIXED$(magnitude, 0, 0)
    ELSEIF kind = "f" OR kind = "F" THEN
        body = QUICKR_FIXED$(magnitude, precision, 0)
        IF alternate AND precision = 0 THEN body = body + "."
    ELSEIF kind = "e" OR kind = "E" THEN
        body = QUICKR_EXPONENT$(magnitude, precision, kind = "E")
        IF alternate AND precision = 0 THEN body = LEFT$(body, 1) + "." + MID$(body, 2)
    ELSEIF kind = "%" THEN
        ' As CPython: the product rounded to a DOUBLE, then formatted exactly.
        body = QUICKR_FIXED$(magnitude * 100, precision, 0)
        IF alternate AND precision = 0 THEN body = body + "."
        body = body + "%"
    ELSE
        body = QUICKR_GENERAL$(magnitude, precision, kind = "G", alternate, kind = "")
    END IF
    IF NOT alternate THEN prefix = ""
    IF zeroless AND negative THEN
        IF QUICKR_TRIM$(body) = "0" THEN negative = 0
    END IF
    QUICKR_FORMAT$ = QUICKR_PAD$(QUICKR_SIGN$(negative, sign) + prefix, body, fill, align, wide, separator, every)
END FUNCTION

' A number's plain text, from QUICKR_REPR$, under a spec without a type.
FUNCTION QUICKR_NUMERIC$ (text AS STRING, sign AS STRING, fill AS STRING, align AS STRING, BYVAL wide AS INTEGER, separator AS STRING)
    DIM negative AS INTEGER, body AS STRING
    negative = LEFT$(text, 1) = "-"
    body = text
    IF negative THEN body = MID$(text, 2)
    QUICKR_NUMERIC$ = QUICKR_PAD$(QUICKR_SIGN$(negative, sign), body, fill, align, wide, separator, 3)
END FUNCTION

' A string cut to `precision` characters (none cut when negative) and
' filled out to `wide`.
FUNCTION QUICKR_TEXT$ (text AS STRING, fill AS STRING, align AS STRING, BYVAL wide AS INTEGER, BYVAL precision AS INTEGER)
    DIM cut AS STRING
    cut = text
    IF precision >= 0 THEN cut = LEFT$(text, precision)
    QUICKR_TEXT$ = QUICKR_PAD$("", cut, fill, align, wide, "", 0)
END FUNCTION
