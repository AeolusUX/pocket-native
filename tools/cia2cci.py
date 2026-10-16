#!/usr/bin/env python3
"""Convert a CIA (plain content layer) into a decrypted .3ds (NCSD/CCI).

usage: cia2cci.py in.cia out.3ds [aes_keys.txt]
Input is only read. NCCH crypto handled: original (0x2C) and 7.x (0x25),
9.3 (0x18), 9.6 (0x1B); seeded titles are rejected.
"""
import os
import struct
import sys

from Cryptodome.Cipher import AES
from Cryptodome.Util import Counter

MU = 0x200
CHUNK = 16 << 20
M128 = (1 << 128) - 1


def rol(x, n):
    return ((x << n) | (x >> (128 - n))) & M128


def load_keys(path):
    k = {}
    for line in open(path):
        if "=" in line:
            name, val = line.strip().split("=", 1)
            k[name.strip()] = int(val.strip(), 16)
    return k


def normal_key(keys, slot, key_y):
    x = keys[f"slot0x{slot:02X}KeyX"]
    return rol((rol(x, 2) ^ key_y) + keys["generator"] & M128, 87).to_bytes(16, "big")


def ctr_cipher(key, partition_id, kind, byte_off):
    base = int.from_bytes(partition_id[::-1] + bytes([kind]) + bytes(7), "big")
    return AES.new(key, AES.MODE_CTR, counter=Counter.new(128, initial_value=base + byte_off // 16))


def cia_contents(f):
    hs, _, _, cs, ts, tmds = struct.unpack("<IHHIII", f.read(0x14))
    al = lambda x: (x + 63) & ~63
    tmdo = al(al(al(hs) + cs) + ts)
    co = al(tmdo + tmds)
    f.seek(tmdo)
    tmd = f.read(tmds)
    sig = 4 + {0x10003: 0x23C, 0x10004: 0x13C, 0x10005: 0x7C}[struct.unpack(">I", tmd[:4])[0]]
    b = tmd[sig:]
    count = struct.unpack(">H", b[0x9E:0xA0])[0]
    rec = b[0xC4 + 64 * 0x24:]
    out, off = [], co
    for i in range(count):
        _, idx, typ, size = struct.unpack(">IHHQ", rec[i * 0x30:i * 0x30 + 16])
        if typ & 1:
            sys.exit(f"content {i} is CIA-layer encrypted (titlekey); not supported")
        out.append((idx, off, size))
        off += al(size)
    return out


def copy_region(src, dst, s_off, d_off, size, cipher=None):
    src.seek(s_off)
    dst.seek(d_off)
    while size:
        n = min(CHUNK, size)
        buf = src.read(n)
        dst.write(cipher.decrypt(buf) if cipher else buf)
        size -= n


def decrypt_ncch(src, dst, s_base, d_base, size, keys):
    src.seek(s_base)
    hdr = bytearray(src.read(0x200))
    assert hdr[0x100:0x104] == b"NCCH", "no NCCH"
    copy_region(src, dst, s_base, d_base, size)
    if hdr[0x18F] & 0x04:
        return
    if hdr[0x18F] & 0x20:
        sys.exit("seeded NCCH (needs seeddb); not supported")
    key_y = int.from_bytes(hdr[:16], "big")
    pid = bytes(hdr[0x108:0x110])
    sec = {0x00: 0x2C, 0x01: 0x25, 0x0A: 0x18, 0x0B: 0x1B}[hdr[0x18B]]
    k_base, k_sec = normal_key(keys, 0x2C, key_y), normal_key(keys, sec, key_y)

    exh_sz = struct.unpack("<I", hdr[0x180:0x184])[0]
    if exh_sz:
        copy_region(src, dst, s_base + 0x200, d_base + 0x200, 0x800, ctr_cipher(k_base, pid, 1, 0))
        copy_region(src, dst, s_base + 0x200 + 0x800, d_base + 0x200 + 0x800, 0x400,
                    ctr_cipher(k_base, pid, 1, 0x800))

    exefs_off, exefs_sz = (v * MU for v in struct.unpack("<II", hdr[0x1A0:0x1A8]))
    if exefs_sz:
        src.seek(s_base + exefs_off)
        exefs_hdr = ctr_cipher(k_base, pid, 2, 0).decrypt(src.read(0x200))
        dst.seek(d_base + exefs_off)
        dst.write(exefs_hdr)
        for i in range(10):
            name = exefs_hdr[i * 16:i * 16 + 8].rstrip(b"\0")
            off, fsz = struct.unpack("<II", exefs_hdr[i * 16 + 8:i * 16 + 16])
            if not fsz:
                continue
            key = k_base if name in (b"icon", b"banner") else k_sec
            rel = 0x200 + off
            copy_region(src, dst, s_base + exefs_off + rel, d_base + exefs_off + rel,
                        (fsz + MU - 1) // MU * MU, ctr_cipher(key, pid, 2, rel))

    romfs_off, romfs_sz = (v * MU for v in struct.unpack("<II", hdr[0x1B0:0x1B8]))
    if romfs_sz:
        copy_region(src, dst, s_base + romfs_off, d_base + romfs_off, romfs_sz,
                    ctr_cipher(k_sec, pid, 3, 0))

    hdr[0x18B] = 0
    hdr[0x18F] = (hdr[0x18F] & ~0x03) | 0x04
    dst.seek(d_base)
    dst.write(hdr)


def main():
    if len(sys.argv) < 3:
        sys.exit(__doc__)
    src_path, dst_path = sys.argv[1], sys.argv[2]
    keys = load_keys(sys.argv[3] if len(sys.argv) > 3 else os.path.expanduser("~/Downloads/aes_keys.txt"))
    if os.path.abspath(src_path) == os.path.abspath(dst_path):
        sys.exit("refusing to overwrite input")
    with open(src_path, "rb") as src, open(dst_path, "wb") as dst:
        contents = cia_contents(src)
        table, pos = [], 0x4000
        for idx, off, size in contents:
            if idx >= 8:
                continue
            decrypt_ncch(src, dst, off, pos, size, keys)
            table.append((idx, pos, size))
            pos += (size + MU - 1) // MU * MU
        src.seek(contents[0][1] + 0x108)
        title_id = src.read(8)
        ncsd = bytearray(0x200)
        ncsd[0x100:0x104] = b"NCSD"
        ncsd[0x104:0x108] = struct.pack("<I", pos // MU)
        ncsd[0x108:0x110] = title_id
        for idx, p, size in table:
            ncsd[0x120 + idx * 8:0x128 + idx * 8] = struct.pack("<II", p // MU, (size + MU - 1) // MU)
        ncsd[0x18D] = 1
        ncsd[0x18F] = 0x02
        dst.seek(0)
        dst.write(ncsd)
        dst.truncate(pos)
    print(f"wrote {dst_path}: {len(table)} partition(s), {pos} bytes")


if __name__ == "__main__":
    main()
