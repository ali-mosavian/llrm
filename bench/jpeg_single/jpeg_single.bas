' flags: -O2 -march=i486
' m16 only: the harness runs BASIC on the Microsoft runtime, and the llrm runtime has no file GET/PUT of strings yet (#1441)
' data: photo.jpg
' JPEG to BMP, an MCU row at a time, as jpeg_single.c: baseline Huffman YCbCr in 4:2:0, 4:4:4, 4:2:2 or 4:4:0 with restart
' markers. Reads PHOTO.JPG, writes a top-down 24-bit OUT.BMP and returns the Adler-32 of the BMP's bytes. IDCT and colour
' conversion in SINGLE. Progressive and arithmetic-coded files are refused with a message.
DEFINT A-Z
CONST MAXW = 256
DECLARE FUNCTION BenchJpegSingle& ()
DECLARE FUNCTION NextByte% ()
DECLARE FUNCTION Next16% ()
DECLARE FUNCTION GetBit% ()
DECLARE FUNCTION Receive% (count AS INTEGER)
DECLARE FUNCTION Decode% (table AS INTEGER)
DECLARE FUNCTION Extend% (value AS INTEGER, size AS INTEGER)
DECLARE FUNCTION Clamp% (value AS SINGLE)
DECLARE SUB Restart ()
DECLARE SUB Emit (outFile AS INTEGER, bytes AS STRING)
DECLARE SUB Put32 (at AS INTEGER, value AS LONG)
DECLARE SUB Idct (p AS INTEGER, x0 AS INTEGER, y0 AS INTEGER)
DECLARE SUB Convert (y AS INTEGER, imgW AS INTEGER, hshift AS INTEGER, vshift AS INTEGER)

DIM SHARED cosine(0 TO 7, 0 TO 7) AS SINGLE
DIM SHARED zigzag(0 TO 63) AS INTEGER, pow2(0 TO 14) AS INTEGER
DIM SHARED planes(0 TO 2, 0 TO 15, 0 TO MAXW - 1) AS INTEGER
DIM SHARED quant(0 TO 3, 0 TO 63) AS INTEGER
DIM SHARED maxcode(0 TO 3, 0 TO 17) AS INTEGER, valptr(0 TO 3, 0 TO 16) AS INTEGER, mincode(0 TO 3, 0 TO 16) AS INTEGER
DIM SHARED huffval(0 TO 3, 0 TO 255) AS INTEGER
DIM SHARED coef(0 TO 63) AS LONG
DIM SHARED inFile AS INTEGER, bad AS INTEGER, inPos AS INTEGER, inLen AS INTEGER, bitBuf AS INTEGER, bitCnt AS INTEGER
DIM SHARED fileLeft AS LONG, adlerA AS LONG, adlerB AS LONG
DIM SHARED inBuf AS STRING, lineBuf AS STRING, bmp AS STRING

PRINT LTRIM$(STR$(BenchJpegSingle&))
END

SUB Emit (outFile AS INTEGER, bytes AS STRING)
    DIM i AS INTEGER
    PUT #outFile, , bytes
    FOR i = 1 TO LEN(bytes)
        adlerA = adlerA + ASC(MID$(bytes, i, 1))
        IF adlerA >= 65521 THEN adlerA = adlerA - 65521
        adlerB = adlerB + adlerA
        IF adlerB >= 65521 THEN adlerB = adlerB - 65521
    NEXT
END SUB

FUNCTION Clamp% (value AS SINGLE)
    IF value <= 0! THEN
        Clamp% = 0
    ELSEIF value >= 255! THEN
        Clamp% = 255
    ELSE
        Clamp% = INT(value)
    END IF
END FUNCTION

SUB Convert (y AS INTEGER, imgW AS INTEGER, hshift AS INTEGER, vshift AS INTEGER)
    DIM x AS INTEGER, cx AS INTEGER, cy AS INTEGER
    DIM luma AS SINGLE, blue AS SINGLE, red AS SINGLE
    cy = y \ (vshift + 1)
    FOR x = 0 TO imgW - 1
        cx = x \ (hshift + 1)
        luma = planes(0, y, x)
        blue = planes(1, cy, cx) - 128!
        red = planes(2, cy, cx) - 128!
        MID$(lineBuf, 3 * x + 1, 1) = CHR$(Clamp%(luma + 1.772! * blue + .5!))
        MID$(lineBuf, 3 * x + 2, 1) = CHR$(Clamp%(luma - .344136! * blue - .714136! * red + .5!))
        MID$(lineBuf, 3 * x + 3, 1) = CHR$(Clamp%(luma + 1.402! * red + .5!))
    NEXT
END SUB

FUNCTION Decode% (table AS INTEGER)
    DIM code AS INTEGER, length AS INTEGER
    code = GetBit%
    length = 1
    DO WHILE code > maxcode(table, length)
        code = code * 2 + GetBit%
        length = length + 1
        IF length > 16 THEN
            bad = 1
            Decode% = 0
            EXIT FUNCTION
        END IF
    LOOP
    Decode% = huffval(table, valptr(table, length) + code - mincode(table, length))
END FUNCTION

FUNCTION Extend% (value AS INTEGER, size AS INTEGER)
    IF value >= pow2(size - 1) THEN
        Extend% = value
    ELSE
        Extend% = value - pow2(size) + 1
    END IF
END FUNCTION

FUNCTION GetBit%
    IF bitCnt = 0 THEN
        bitBuf = NextByte%
        IF bitBuf = 255 THEN
            IF NextByte% <> 0 THEN bad = 1
        END IF
        bitCnt = 8
    END IF
    bitCnt = bitCnt - 1
    GetBit% = (bitBuf \ pow2(bitCnt)) AND 1
END FUNCTION

SUB Idct (p AS INTEGER, x0 AS INTEGER, y0 AS INTEGER)
    DIM tmp(0 TO 63) AS SINGLE
    DIM sum AS SINGLE
    DIM v AS INTEGER, x AS INTEGER, u AS INTEGER, y AS INTEGER
    FOR v = 0 TO 7
        FOR x = 0 TO 7
            sum = 0!
            FOR u = 0 TO 7
                sum = sum + cosine(x, u) * CSNG(coef(v * 8 + u))
            NEXT
            tmp(v * 8 + x) = sum
        NEXT
    NEXT
    FOR y = 0 TO 7
        FOR x = 0 TO 7
            sum = 0!
            FOR v = 0 TO 7
                sum = sum + cosine(y, v) * tmp(v * 8 + x)
            NEXT
            planes(p, y0 + y, x0 + x) = Clamp%(sum + 128.5!)
        NEXT
    NEXT
END SUB

FUNCTION Next16%
    DIM high AS INTEGER
    high = NextByte%
    Next16% = high * 256 + NextByte%
END FUNCTION

FUNCTION NextByte%
    DIM n AS INTEGER
    IF inPos >= inLen THEN
        IF fileLeft <= 0 THEN
            bad = 1
            NextByte% = 0
            EXIT FUNCTION
        END IF
        IF fileLeft < 512 THEN n = fileLeft ELSE n = 512
        inBuf = SPACE$(n)
        GET #inFile, , inBuf
        fileLeft = fileLeft - n
        inLen = n
        inPos = 0
    END IF
    inPos = inPos + 1
    NextByte% = ASC(MID$(inBuf, inPos, 1))
END FUNCTION

SUB Put32 (at AS INTEGER, value AS LONG)
    DIM i AS INTEGER, rest AS LONG
    rest = value
    FOR i = 1 TO 4
        MID$(bmp, at + i, 1) = CHR$(rest AND 255)
        rest = (rest + 1073741824) \ 256 - 4194304
    NEXT
END SUB

FUNCTION Receive% (count AS INTEGER)
    DIM value AS INTEGER, i AS INTEGER
    value = 0
    FOR i = 1 TO count
        value = value * 2 + GetBit%
    NEXT
    Receive% = value
END FUNCTION

SUB Restart
    DIM marker AS INTEGER
    bitCnt = 0
    DO
        DO
            marker = NextByte%
        LOOP UNTIL marker = 255 OR bad
        marker = NextByte%
    LOOP UNTIL bad OR (marker >= 208 AND marker <= 215)
END SUB

FUNCTION BenchJpegSingle&
    DIM id(0 TO 2) AS INTEGER, sample(0 TO 2) AS INTEGER, qsel(0 TO 2) AS INTEGER
    DIM dcsel(0 TO 2) AS INTEGER, acsel(0 TO 2) AS INTEGER, pred(0 TO 2) AS INTEGER, counts(0 TO 16) AS INTEGER
    DIM imgW AS INTEGER, imgH AS INTEGER, interval AS INTEGER, scanned AS INTEGER, comps AS INTEGER
    DIM marker AS INTEGER, length AS INTEGER, i AS INTEGER, k AS INTEGER, c AS INTEGER, t AS INTEGER
    DIM hmax AS INTEGER, vmax AS INTEGER, mcusX AS INTEGER, mcusY AS INTEGER, mx AS INTEGER, my AS INTEGER
    DIM bx AS INTEGER, by AS INTEGER, bw AS INTEGER, bh AS INTEGER, stride AS INTEGER, remaining AS INTEGER
    DIM outFile AS INTEGER, h AS INTEGER, v AS INTEGER, n AS INTEGER, want AS INTEGER, selector AS INTEGER
    DIM code AS INTEGER, at AS INTEGER, rs AS INTEGER, runLen AS INTEGER, bits AS INTEGER
    DIM size AS LONG

    ' cos((2x+1)u*pi/16) * (u ? 1 : 1/sqrt(2)) / 2, row by row
    DATA .353553385,.490392625,.461939752,.415734798,.353553385,.277785122,.191341713,.0975451618
    DATA .353553385,.415734798,.191341713,-.0975451618,-.353553385,-.490392625,-.461939752,-.277785122
    DATA .353553385,.277785122,-.191341713,-.490392625,-.353553385,.0975451618,.461939752,.415734798
    DATA .353553385,.0975451618,-.461939752,-.277785122,.353553385,.415734798,-.191341713,-.490392625
    DATA .353553385,-.0975451618,-.461939752,.277785122,.353553385,-.415734798,-.191341713,.490392625
    DATA .353553385,-.277785122,-.191341713,.490392625,-.353553385,-.0975451618,.461939752,-.415734798
    DATA .353553385,-.415734798,.191341713,.0975451618,-.353553385,.490392625,-.461939752,.277785122
    DATA .353553385,-.490392625,.461939752,-.415734798,.353553385,-.277785122,.191341713,-.0975451618
    DATA 0,1,8,16,9,2,3,10,17,24,32,25,18,11,4,5,12,19,26,33,40,48,41,34,27,20,13,6,7,14,21,28
    DATA 35,42,49,56,57,50,43,36,29,22,15,23,30,37,44,51,58,59,52,45,38,31,39,46,53,60,61,54,47,55,62,63
    FOR i = 0 TO 7
        FOR k = 0 TO 7
            READ cosine(i, k)
        NEXT
    NEXT
    FOR i = 0 TO 63
        READ zigzag(i)
    NEXT
    pow2(0) = 1
    FOR i = 1 TO 14
        pow2(i) = pow2(i - 1) * 2
    NEXT
    adlerA = 1: adlerB = 0: bad = 0: bitCnt = 0: inPos = 0: inLen = 0
    lineBuf = SPACE$(MAXW * 3 + 3)
    bmp = SPACE$(54)
    FOR i = 1 TO 54
        MID$(bmp, i, 1) = CHR$(0)
    NEXT

    OPEN "PHOTO.JPG" FOR BINARY AS #1
    inFile = 1
    fileLeft = LOF(1)
    IF NextByte% <> 255 OR NextByte% <> 216 THEN
        PRINT "not a JPEG"
        BenchJpegSingle& = 0
        EXIT FUNCTION
    END IF
    scanned = 0: interval = 0: comps = 0
    DO WHILE scanned = 0 AND bad = 0
        DO
            marker = NextByte%
        LOOP UNTIL marker = 255 OR bad
        DO
            marker = NextByte%
        LOOP WHILE marker = 255
        length = Next16% - 2
        IF marker = &HDB THEN
            DO WHILE length > 0
                t = NextByte%
                IF t \ 16 <> 0 THEN
                    PRINT "16-bit quantization tables not supported"
                    BenchJpegSingle& = 0
                    EXIT FUNCTION
                END IF
                FOR k = 0 TO 63
                    quant(t AND 3, k) = NextByte%
                NEXT
                length = length - 65
            LOOP
        ELSEIF marker = &HC4 THEN
            DO WHILE length > 0 AND bad = 0
                t = NextByte%
                t = (t \ 16) * 2 + (t AND 1)
                n = 0
                FOR k = 1 TO 16
                    counts(k) = NextByte%
                    n = n + counts(k)
                NEXT
                code = 0: at = 0
                FOR k = 1 TO 16
                    valptr(t, k) = at
                    mincode(t, k) = code
                    code = code + counts(k)
                    at = at + counts(k)
                    IF counts(k) <> 0 THEN maxcode(t, k) = code - 1 ELSE maxcode(t, k) = -1
                    code = code * 2
                NEXT
                FOR k = 0 TO n - 1
                    huffval(t, k) = NextByte%
                NEXT
                length = length - 17 - n
            LOOP
        ELSEIF marker = &HC0 THEN
            IF NextByte% <> 8 THEN
                PRINT "only 8-bit samples are supported"
                BenchJpegSingle& = 0
                EXIT FUNCTION
            END IF
            imgH = Next16%
            imgW = Next16%
            comps = NextByte%
            FOR i = 0 TO comps - 1
                IF i < 3 THEN
                    id(i) = NextByte%
                    sample(i) = NextByte%
                    qsel(i) = NextByte% AND 3
                END IF
            NEXT
            IF comps < 3 THEN length = length - 6 - 3 * comps ELSE length = length - 15
        ELSEIF marker = &HC2 THEN
            PRINT "progressive JPEG not supported"
            BenchJpegSingle& = 0
            EXIT FUNCTION
        ELSEIF marker >= &HC9 AND marker <= &HCF AND marker <> &HCC THEN
            PRINT "arithmetic-coded JPEG not supported"
            BenchJpegSingle& = 0
            EXIT FUNCTION
        ELSEIF marker >= &HC1 AND marker <= &HCF AND marker <> &HC4 AND marker <> &HC8 AND marker <> &HCC THEN
            PRINT "only baseline JPEG is supported"
            BenchJpegSingle& = 0
            EXIT FUNCTION
        ELSEIF marker = &HDD THEN
            interval = Next16%
            length = length - 2
        ELSEIF marker = &HDA THEN
            n = NextByte%
            FOR i = 1 TO n
                want = NextByte%
                selector = NextByte%
                FOR c = 0 TO 2
                    IF id(c) = want THEN
                        dcsel(c) = selector \ 16
                        acsel(c) = 2 + (selector AND 1)
                    END IF
                NEXT
            NEXT
            t = NextByte%
            t = NextByte%
            t = NextByte%
            scanned = 1
            length = 0
        END IF
        DO WHILE length > 0
            t = NextByte%
            length = length - 1
        LOOP
    LOOP
    hmax = sample(0) \ 16
    vmax = sample(0) AND 15
    IF bad OR comps <> 3 OR hmax < 1 OR hmax > 2 OR vmax < 1 OR vmax > 2 OR sample(1) <> &H11 OR sample(2) <> &H11 OR imgW < 1 OR imgW > MAXW OR imgH < 1 THEN
        PRINT "unsupported JPEG"
        BenchJpegSingle& = 0
        EXIT FUNCTION
    END IF
    bw = 8 * hmax
    bh = 8 * vmax
    mcusX = (imgW + bw - 1) \ bw
    mcusY = (imgH + bh - 1) \ bh
    stride = (imgW * 3 + 3) AND &HFFFC
    size = 54& + CLNG(stride) * imgH
    MID$(bmp, 1, 1) = "B"
    MID$(bmp, 2, 1) = "M"
    Put32 2, size
    Put32 10, 54&
    Put32 14, 40&
    Put32 18, CLNG(imgW)
    Put32 22, -CLNG(imgH)
    MID$(bmp, 27, 1) = CHR$(1)
    MID$(bmp, 29, 1) = CHR$(24)
    Put32 34, size - 54&
    Put32 38, 2835&
    Put32 42, 2835&
    OPEN "OUT.BMP" FOR BINARY AS #2
    outFile = 2
    Emit outFile, bmp
    FOR i = 0 TO 2
        pred(i) = 0
    NEXT
    remaining = interval
    FOR my = 0 TO mcusY - 1
        IF bad THEN EXIT FOR
        FOR mx = 0 TO mcusX - 1
            IF bad THEN EXIT FOR
            IF interval <> 0 AND remaining = 0 THEN
                Restart
                FOR i = 0 TO 2
                    pred(i) = 0
                NEXT
                remaining = interval
            END IF
            remaining = remaining - 1
            FOR c = 0 TO 2
                IF c = 0 THEN h = hmax: v = vmax ELSE h = 1: v = 1
                FOR by = 0 TO v - 1
                    FOR bx = 0 TO h - 1
                        FOR k = 0 TO 63
                            coef(k) = 0
                        NEXT
                        t = Decode%(dcsel(c))
                        IF t <> 0 THEN pred(c) = pred(c) + Extend%(Receive%(t), t)
                        coef(0) = CLNG(pred(c)) * quant(qsel(c), 0)
                        k = 1
                        DO WHILE k < 64 AND bad = 0
                            rs = Decode%(acsel(c))
                            runLen = rs \ 16
                            bits = rs AND 15
                            IF bits = 0 THEN
                                IF runLen <> 15 THEN EXIT DO
                                k = k + 16
                            ELSE
                                k = k + runLen
                                IF k > 63 THEN EXIT DO
                                coef(zigzag(k)) = CLNG(Extend%(Receive%(bits), bits)) * quant(qsel(c), k)
                                k = k + 1
                            END IF
                        LOOP
                        Idct c, (mx * h + bx) * 8, by * 8
                    NEXT
                NEXT
            NEXT
        NEXT
        FOR i = 0 TO bh - 1
            IF my * bh + i >= imgH THEN EXIT FOR
            Convert i, imgW, hmax - 1, vmax - 1
            FOR k = imgW * 3 + 1 TO stride
                MID$(lineBuf, k, 1) = CHR$(0)
            NEXT
            Emit outFile, LEFT$(lineBuf, stride)
        NEXT
    NEXT
    CLOSE #2
    CLOSE #1
    IF adlerB >= 32768 THEN
        adlerB = adlerB - 65536
        BenchJpegSingle& = adlerB * 65536 + adlerA
    ELSE
        BenchJpegSingle& = adlerB * 65536 + adlerA
    END IF
END FUNCTION
