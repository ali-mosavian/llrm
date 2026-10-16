"""Publics, externs and segment sizes of an OMF object, enough to compare two builds of a module."""
import struct


def read(path):
    """{'publics': set, 'externs': set, 'code': bytes, 'data': bytes} of the object at `path`."""
    raw = open(path, "rb").read()
    names, segs = [], []
    publics, externs = set(), set()
    at = 0
    while at + 3 <= len(raw):
        kind, length = raw[at], struct.unpack_from("<H", raw, at + 1)[0]
        body = raw[at + 3 : at + 3 + length - 1]
        at += 3 + length
        wide = kind & 1
        if kind == 0x96:
            i = 0
            while i < len(body):
                n = body[i]
                names.append(body[i + 1 : i + 1 + n].decode("latin1"))
                i += 1 + n
        elif kind in (0x98, 0x99):
            attr = body[0]
            i = 1 + (3 if attr >> 5 == 0 else 0)
            size = struct.unpack_from("<I" if wide else "<H", body + b"\0\0", i)[0]
            i += 4 if wide else 2
            name = body[i]
            klass = body[i + 1]
            if attr & 2 and not wide:  # B bit: 64K
                size = 0x10000
            segs.append((names[name - 1] if name else "", names[klass - 1] if klass else "", size))
        elif kind in (0x8C,):
            i = 0
            while i < len(body):
                n = body[i]
                externs.add(body[i + 1 : i + 1 + n].decode("latin1"))
                i += 2 + n
        elif kind in (0x90, 0x91):
            i = 0
            group = body[i]
            seg = body[i + 1]
            i += 2
            if seg == 0:
                i += 2
            while i < len(body):
                n = body[i]
                publics.add(body[i + 1 : i + 1 + n].decode("latin1"))
                i += 1 + n + (4 if wide else 2) + 1
    code = sum(s for _, k, s in segs if k.upper() == "CODE")
    data = sum(s for _, k, s in segs if k.upper() != "CODE")
    return {"publics": publics, "externs": externs, "code": code, "data": data, "segs": segs}
