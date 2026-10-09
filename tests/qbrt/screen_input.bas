' INPUT: the prompt, "Redo from start" for a line that does not fit, strings
' with blanks, the semicolon forms.  The keys come from screen_input.in.
CLS
INPUT "Number"; n%
PRINT "got"; n%
INPUT "Two numbers"; a%, b%
PRINT "got"; a%; b%
INPUT s$
PRINT "got ["; s$; "]"
LOCATE 25, 1
