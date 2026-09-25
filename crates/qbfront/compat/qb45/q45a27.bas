' QB45 compatibility source.
dim machineCode(0 to 0) as integer

def seg = varseg(machineCode(0))
poke varptr(machineCode(0)), 203
call absolute(varptr(machineCode(0)))
def seg
print "PASS absolute"
end
