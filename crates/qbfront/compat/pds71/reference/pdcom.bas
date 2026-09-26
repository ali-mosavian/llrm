dim comFile as integer
dim hitCount as integer

on com(1) gosub comHandler
com(1) on
comFile = freefile
open "COM1:9600,N,8,1" for random as comFile len = 64
print "REFERENCE ONLY pds-com-event"
close comFile
end

comHandler:
hitCount = hitCount + 1
return
