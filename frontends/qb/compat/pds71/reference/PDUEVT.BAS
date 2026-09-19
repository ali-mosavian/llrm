' The delivery path requires an ISR plus SetUEvent, so this is parser-only.
dim hitCount as integer

on uevent gosub eventHandler
uevent off
print "REFERENCE ONLY pds-uevent"
end

eventHandler:
hitCount = hitCount + 1
return
