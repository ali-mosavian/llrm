' flags: -O1 | -O2 | -Os
PRINT HEX$(-1%); HEX$(-16%); HEX$(-2%); HEX$(255%); HEX$(32767%)
PRINT OCT$(-1%); OCT$(-16%); OCT$(-2%); OCT$(255%)
PRINT HEX$(-1&); HEX$(-16&); OCT$(-1&)
x% = -1: y& = -16
PRINT HEX$(x%); OCT$(x%); HEX$(y&); OCT$(y&)
PRINT HEX$(65535); HEX$(65536); HEX$(-32768); HEX$(-1); OCT$(-1)
PRINT HEX$(-1.5); HEX$(1E5)
