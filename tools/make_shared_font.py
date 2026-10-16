#!/usr/bin/env python3
"""Build shared_font.bin (0x80 APT header + unrelocated BCFNT) from Azahar's
open-source shared font archive.

usage: make_shared_font.py shared_font.app.romfs.h out.bin
"""
import re
import struct
import sys


def romfs_files(d):
    meta_off, meta_len = struct.unpack("<II", d[0x1C:0x24])
    data_off = struct.unpack("<I", d[0x24:0x28])[0]
    p = meta_off
    while p < meta_off + meta_len:
        off, size = struct.unpack("<QQ", d[p + 8:p + 24])
        name_len = struct.unpack("<I", d[p + 28:p + 32])[0]
        name = d[p + 32:p + 32 + name_len].decode("utf-16le")
        yield name, d[data_off + off:data_off + off + size]
        p += (32 + name_len + 3) & ~3


def lz11(src):
    assert src[0] == 0x11, "not LZ11"
    size = int.from_bytes(src[1:4], "little")
    out, i = bytearray(), 4
    while len(out) < size:
        flags = src[i]; i += 1
        for bit in range(8):
            if len(out) >= size:
                break
            if not flags & (0x80 >> bit):
                out.append(src[i]); i += 1
                continue
            b = src[i]
            ind = b >> 4
            if ind == 0:
                n = (((b & 0xF) << 4) | (src[i + 1] >> 4)) + 0x11
                disp = ((src[i + 1] & 0xF) << 8) | src[i + 2]; i += 3
            elif ind == 1:
                n = (((b & 0xF) << 12) | (src[i + 1] << 4) | (src[i + 2] >> 4)) + 0x111
                disp = ((src[i + 2] & 0xF) << 8) | src[i + 3]; i += 4
            else:
                n = ind + 1
                disp = ((b & 0xF) << 8) | src[i + 1]; i += 2
            for _ in range(n):
                out.append(out[-disp - 1])
    return bytes(out)


def main():
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    text = open(sys.argv[1]).read()
    body = text[text.index("{") + 1:text.rindex("}")]
    romfs = bytes(int(x, 16) for x in re.findall(r"0x[0-9a-fA-F]{2}", body))
    font = dict(romfs_files(romfs))["cbf_std.bcfnt.lz"]
    bcfnt = bytearray(lz11(font))
    assert bcfnt[:4] == b"CFNT", bcfnt[:4]
    bcfnt[3] = ord("U")
    header = struct.pack("<III", 2, 1, len(bcfnt)).ljust(0x80, b"\0")
    open(sys.argv[2], "wb").write(header + bcfnt)
    print(f"wrote {sys.argv[2]}: {len(header) + len(bcfnt)} bytes")


if __name__ == "__main__":
    main()
