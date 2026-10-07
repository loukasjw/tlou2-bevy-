#!/usr/bin/env python3
"""Extract / list a Naughty Dog 'DSAR' archive (TLOU2 PC *.psarc), streaming.

Layout (worked out from the files, not from documentation; verified on bin.psarc,
sp-common.psarc, common.psarc and core.psarc, all DSAR version 3):
  * DSAR header: magic 'DSAR', version, block count (u32 at 8), ...
  * From 0x18, a table of 32-byte records, one per block:
      marker(8) | stream_offset(8) | file_offset(8) | uncompressed_size(4) | compressed_size(4)
    Blocks are LZ4 (block format). A record with compressed_size == 0 is padding.
  * Placing every block at its stream_offset gives a standard PSARC archive
    (big-endian header, zlib blocks) containing the game's data files.
  * PSARC entry order is by MD5(path); the manifest (entry 0) is alphabetical.
    Each entry's 16-byte id is MD5 of its path, so names are paired by MD5, NOT by position.
    (The first version of this script paired by position and attached the wrong names.)

This version never builds the whole stream in memory. It keeps only the block table
(a few MB) and decompresses blocks on demand through a small cache.

Usage:
  python extract_dsar.py ARCHIVE --list [--only WORD ...] [--sizes]
  python extract_dsar.py ARCHIVE OUT_DIR [--only WORD ...]
--only is a case-insensitive substring match on the file path (any of the words).
Needs: pip install lz4
"""
import argparse
import bisect
import hashlib
import os
import struct
import sys
import zlib
from collections import OrderedDict

import lz4.block

NL = b"\n"
NUL = b"\x00"


class DsarStream:
    """Random-access view of the PSARC stream hidden inside a DSAR file."""

    def __init__(self, path, cache_blocks=256):
        self.f = open(path, "rb")
        head = self.f.read(0x18)
        if head[:4] != b"DSAR":
            sys.exit("Not a DSAR file (magic %r)" % head[:4])
        self.version = struct.unpack_from("<I", head, 4)[0]
        count = struct.unpack_from("<I", head, 8)[0]
        raw = self.f.read(count * 32)
        recs = [struct.unpack_from("<QQQII", raw, i * 32) for i in range(count)]
        recs = [r for r in recs if r[4] != 0]          # drop padding records
        recs.sort(key=lambda r: r[1])
        self.pos = [r[1] for r in recs]
        self.recs = recs
        self.size = max(r[1] + r[3] for r in recs)
        self.cache = OrderedDict()
        self.cache_blocks = cache_blocks

    def _block(self, i):
        b = self.cache.get(i)
        if b is not None:
            self.cache.move_to_end(i)
            return b
        _m, _pos, off, usz, csz = self.recs[i]
        self.f.seek(off)
        b = lz4.block.decompress(self.f.read(csz), uncompressed_size=usz)
        self.cache[i] = b
        if len(self.cache) > self.cache_blocks:
            self.cache.popitem(last=False)
        return b

    def read(self, pos, n):
        out = bytearray()
        end = pos + n
        i = bisect.bisect_right(self.pos, pos) - 1
        while pos < end:
            if i < 0 or i >= len(self.recs):
                out += bytes(end - pos)
                break
            _m, bpos, _off, usz, _csz = self.recs[i]
            if pos >= bpos + usz:                      # gap between blocks -> zeros
                nxt = self.pos[i + 1] if i + 1 < len(self.recs) else end
                take = min(end, nxt) - pos
                out += bytes(take)
                pos += take
                i += 1
                continue
            blk = self._block(i)
            s = pos - bpos
            take = min(end - pos, usz - s)
            out += blk[s:s + take]
            pos += take
            i += 1
        return bytes(out)


class Psarc:
    def __init__(self, S):
        self.S = S
        magic, _ver, self.comp, toclen, entsz, cnt, self.bsz, _flags = struct.unpack(
            ">4sI4sIIIII", S.read(0, 32))
        if magic != b"PSAR":
            sys.exit("Inner archive is not PSARC (%r)" % magic)
        toc = S.read(0, toclen)
        self.ents, ids = [], []
        for i in range(cnt):
            b = toc[32 + i * entsz:32 + (i + 1) * entsz]
            ids.append(b[:16])
            idx = struct.unpack(">I", b[16:20])[0]
            self.ents.append((idx, int.from_bytes(b[20:25], "big"), int.from_bytes(b[25:30], "big")))
        zw = 2 if self.bsz <= 65536 else 3
        t0 = 32 + cnt * entsz
        self.tbl = [int.from_bytes(toc[t0 + i * zw:t0 + (i + 1) * zw], "big")
                    for i in range((toclen - t0) // zw)]
        manifest = [n.decode("utf-8", "replace")
                    for n in self.read(self.ents[0]).replace(NL, NUL).split(NUL) if n]
        by_md5 = {hashlib.md5(n.encode("utf-8")).digest(): n for n in manifest}
        self.names = [by_md5.get(i) for i in ids[1:]]
        self.unmatched = sum(1 for n in self.names if n is None)
        for k, n in enumerate(self.names):
            if n is None:
                self.names[k] = "_unmatched/%08d.bin" % (k + 1)

    def chunks(self, e):
        """Yield the file content one PSARC block at a time."""
        idx, usz, off = e
        pos, left, j = off, usz, idx
        while left > 0:
            raw = min(self.bsz, left)
            c = self.tbl[j] or self.bsz
            blk = self.S.read(pos, c)
            yield blk[:raw] if c >= raw else zlib.decompress(blk)
            pos += c
            left -= raw
            j += 1

    def read(self, e):
        return b"".join(self.chunks(e))


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("archive")
    ap.add_argument("out", nargs="?", help="output folder (not needed with --list)")
    ap.add_argument("--list", action="store_true", help="print file names (and sizes) without extracting")
    ap.add_argument("--sizes", action="store_true", help="with --list: put the uncompressed size before each name")
    ap.add_argument("--only", nargs="+", default=[], help="case-insensitive substrings to match in the path")
    a = ap.parse_args()
    if not a.list and not a.out:
        ap.error("OUT_DIR is required unless --list is given")

    P = Psarc(DsarStream(a.archive))
    if P.unmatched:
        print("warning: %d entries had no matching name" % P.unmatched, file=sys.stderr)
    only = [s.lower() for s in a.only]
    sel = [(e, n) for e, n in zip(P.ents[1:], P.names)
           if not only or any(s in n.lower() for s in only)]

    if a.list:
        for e, n in sorted(sel, key=lambda t: t[1]):
            print("%12d  %s" % (e[1], n) if a.sizes else n)
        print("# %d of %d files, %d bytes uncompressed" % (len(sel), len(P.names), sum(e[1] for e, _ in sel)),
              file=sys.stderr)
        return

    for e, n in sel:
        p = os.path.join(a.out, n)
        os.makedirs(os.path.dirname(p), exist_ok=True)
        with open(p, "wb") as f:
            for c in P.chunks(e):
                f.write(c)
    print("Extracted %d of %d files to %s" % (len(sel), len(P.names), a.out))


if __name__ == "__main__":
    main()
