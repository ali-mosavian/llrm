dim fileNumber as integer
dim done as integer
dim lineText as string

fileNumber = freefile
open "input.cfg" for input as #fileNumber
done = eof(fileNumber)
line input #fileNumber, lineText
close #fileNumber

fileNumber = freefile
open "output.dat" for output as #fileNumber
close #fileNumber

fileNumber = freefile
open "raw.bin" for binary as #fileNumber
close #fileNumber
