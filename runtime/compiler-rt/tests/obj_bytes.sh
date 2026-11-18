# Source me. obj_bytes OBJ: the .text and .rodata bytes of an ELF object, no .eh_frame (`size`'s text column counts it).
obj_bytes() { size -A "$1" | awk '$1~/^\.text/||$1~/^\.rodata/{s+=$2} END{print s+0}'; }
