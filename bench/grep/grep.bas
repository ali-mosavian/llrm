' data: @dickens
' Aho-Corasick over the bytes of DICKENS, as grep.c: built once from ten patterns, then a table walk over 1K chunks.
DEFINT A-Z
DECLARE FUNCTION BenchGrep& (checksum AS LONG)

DIM sum AS LONG
PRINT LTRIM$(STR$(BenchGrep&(sum)))
PRINT LTRIM$(STR$(sum))

FUNCTION BenchGrep& (checksum AS LONG)
    CONST STRIDE = 32
    DIM nxt(0 TO 80 * STRIDE - 1) AS INTEGER, classof(0 TO 255) AS INTEGER
    DIM fail(0 TO 79) AS INTEGER, hit(0 TO 79) AS INTEGER, queue(0 TO 79) AS INTEGER
    DIM patterns AS STRING, buf AS STRING
    DIM classes AS INTEGER, states AS INTEGER, id AS INTEGER, s AS INTEGER, head AS INTEGER, tail AS INTEGER
    DIM cl AS INTEGER, n AS INTEGER, i AS INTEGER, c AS INTEGER, t AS INTEGER, f AS INTEGER, inline AS INTEGER
    DIM lines AS LONG, sum AS LONG, at AS LONG, remaining AS LONG

    patterns = "Oliver|Scrooge|Copperfield|Dombey|Marley|Micawber|Heep|ghost|gentleman|Tiny Tim|"
    classes = 1: states = 1: id = 1: s = 0
    FOR i = 1 TO LEN(patterns)
        c = ASC(MID$(patterns, i, 1))
        IF c = 124 THEN
            hit(s) = id
            id = id + 1
            s = 0
        ELSE
            IF classof(c) = 0 THEN
                classof(c) = classes
                classes = classes + 1
            END IF
            IF nxt(s * STRIDE + classof(c)) = 0 THEN
                nxt(s * STRIDE + classof(c)) = states
                states = states + 1
            END IF
            s = nxt(s * STRIDE + classof(c))
        END IF
    NEXT
    head = 0: tail = 0
    FOR cl = 1 TO classes - 1
        IF nxt(cl) <> 0 THEN
            queue(tail) = nxt(cl)
            tail = tail + 1
        END IF
    NEXT
    DO WHILE head < tail
        s = queue(head)
        head = head + 1
        FOR cl = 0 TO classes - 1
            t = nxt(s * STRIDE + cl)
            f = nxt(fail(s) * STRIDE + cl)
            IF t <> 0 THEN
                fail(t) = f
                IF hit(t) = 0 THEN hit(t) = hit(f)
                queue(tail) = t
                tail = tail + 1
            ELSE
                nxt(s * STRIDE + cl) = f
            END IF
        NEXT
    LOOP

    OPEN "DICKENS" FOR BINARY AS #1
    remaining = LOF(1)
    buf = SPACE$(1024)
    s = 0: inline = 0: lines = 0: sum = 0: at = 0
    DO WHILE remaining > 0
        IF remaining < 1024 THEN
            n = remaining
            buf = SPACE$(n)
        ELSE
            n = 1024
        END IF
        GET #1, , buf
        remaining = remaining - n
        FOR i = 1 TO n
            c = ASC(MID$(buf, i, 1))
            s = nxt(s * STRIDE + classof(c))
            IF hit(s) <> 0 THEN
                inline = 1
                sum = ((sum * 2) + at + hit(s)) AND &H1FFFFFFF
            END IF
            IF c = 10 THEN
                lines = lines + inline
                inline = 0
            END IF
            at = at + 1
        NEXT
    LOOP
    CLOSE #1
    checksum = sum
    BenchGrep& = lines + inline
END FUNCTION
