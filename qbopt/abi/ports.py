"""What an `in` or `out` can reach, by the device its port names.

A port is a device, and what the instruction can do to memory is what that
device can. A port nobody has named here can start a DMA transfer or a bus
master, so it keeps unknown reach. A device listed here has no path to
memory: its state lives behind the port.
"""

# The VGA DAC: PEL mask, read index, write index, data. The palette is mapped
# at no address.
SILENT = ((0x3C6, 0x3C9),)


def silent(port: int) -> bool:
    return any(low <= port <= high for low, high in SILENT)
