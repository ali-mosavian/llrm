' PUT # without a variable calls B$PUT1.
OPEN "T.DAT" FOR RANDOM AS #1 LEN = 4
PUT #1
CLOSE #1
