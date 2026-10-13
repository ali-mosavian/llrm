' gfx_back_to_text.bas at one address space: video memory is an address held in a variable (DEF SEG is ignored there).
' SCREEN 0 after SCREEN 9: the text screen is clean and PRINT shows, in colour; CLS clears the key line too.
SCREEN 9
LINE (0, 0)-(100, 100), 4, BF
SCREEN 0
WIDTH 80, 25
video& = &HB8000
PRINT PEEK(video&); PEEK(video& + 1); PEEK(video& + 3998); PEEK(video& + 3999)
COLOR 15, 0
CLS
PRINT "hello"
PRINT PEEK(video& + 3999); PEEK(video& + 161)
LOCATE 5, 5: COLOR 12: PRINT "red"
SLEEP
