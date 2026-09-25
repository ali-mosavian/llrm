' QuickrBASIC's f-string support, compiled into the programs that use
' f-strings. Every name here begins QUICKR_, which programs cannot use.

' STR$'s text the way Python's str() writes it: no sign space, and a zero
' before a bare point.
FUNCTION QUICKR_NUMBER$ (text AS STRING)
    DIM trimmed AS STRING
    trimmed = LTRIM$(text)
    IF LEFT$(trimmed, 1) = "." THEN
        trimmed = "0" + trimmed
    ELSEIF LEFT$(trimmed, 2) = "-." THEN
        trimmed = "-0" + MID$(trimmed, 2)
    END IF
    QUICKR_NUMBER$ = trimmed
END FUNCTION
