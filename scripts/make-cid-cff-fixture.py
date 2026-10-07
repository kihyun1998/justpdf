"""Builds justpdf-render/tests/fixtures/cid-test.cff from a name-keyed bare CFF.

Usage: make-cid-cff-fixture.py <NimbusSans-Regular.cff> <out-dir>  (needs fontTools)
Writes cid-test.cff (Top DICT FontMatrix 0.001, the default) and cid-test-half.cff
(FontMatrix 0.0005, so glyphs come out half size). No FDArray FontMatrix in either.
Glyphs: GID 0 .notdef (CID 0), GID 1 = H (CID 300), GID 2 = E (CID 301); subroutines
are flattened into the charstrings. The source font is URW NimbusSans (SIL OFL 1.1).
"""
import io
import sys
import types

from fontTools.cffLib import (
    CFFFontSet,
    FDArrayIndex,
    FDSelect,
    FontDict,
)

src, out_dir = sys.argv[1:3]
fs = CFFFontSet()
fs.decompile(io.BytesIO(open(src, "rb").read()), otFont=None)
top = fs.topDictIndex[0]
cs = top.CharStrings

keep = [".notdef", "H", "E"]
cids = [0, 300, 301]

# Decompile the kept charstrings so they no longer depend on the source's subroutines.
programs = []
for name in keep:
    c = cs[name]
    c.decompile()
    programs.append(c)

# Flatten subroutine calls into the programs.
from fontTools.misc.psCharStrings import T2CharString  # noqa: E402


def flatten(c, local_subrs, global_subrs):
    out = []
    prog = c.program
    i = 0
    while i < len(prog):
        tok = prog[i]
        if tok in ("callsubr", "callgsubr"):
            idx = out.pop()
            subrs = local_subrs if tok == "callsubr" else global_subrs
            bias = 107 if len(subrs) < 1240 else (1131 if len(subrs) < 33900 else 32768)
            sub = subrs[idx + bias]
            sub.decompile()
            body = flatten(sub, local_subrs, global_subrs)
            if body and body[-1] == "return":
                body = body[:-1]
            out.extend(body)
        else:
            out.append(tok)
        i += 1
    return out


local_subrs = getattr(top.Private, "Subrs", [])
global_subrs = fs.GlobalSubrs
flat = [flatten(p, local_subrs, global_subrs) for p in programs]

# New CID-keyed font with the three glyphs.
private = top.Private
if hasattr(private, "Subrs"):
    del private.rawDict["Subrs"]
    private.Subrs = None
    del private.Subrs

new_names = ["cid%05d" % c if c else ".notdef" for c in cids]
cs.charStrings = {}
cs.charStringsIndex.items = []
for i, (name, prog) in enumerate(zip(new_names, flat)):
    t2 = T2CharString(program=prog, private=private, globalSubrs=None)
    cs.charStringsIndex.items.append(t2)
    cs.charStrings[name] = i
top.charset = new_names
fs.GlobalSubrs.items = []

top.ROS = ("Adobe", "Identity", 0)
top.CIDCount = max(cids) + 1
fd = FontDict()
fd.setCFF2(False)
fd.Private = private
top.FDArray = FDArrayIndex()
top.FDArray.append(fd)
top.FDSelect = FDSelect()
top.FDSelect.gidArray = [0] * len(new_names)
top.FDSelect.format = 3
for attr in ("Encoding",):
    if attr in top.rawDict:
        del top.rawDict[attr]
    if hasattr(top, attr):
        delattr(top, attr)
del top.rawDict["Private"]
top.Private = None
del top.Private

for name, matrix in (("cid-test.cff", None), ("cid-test-half.cff", [0.0005, 0, 0, 0.0005, 0, 0])):
    if matrix:
        top.FontMatrix = matrix
    out = io.BytesIO()
    fs.compile(out, otFont=types.SimpleNamespace(recalcBBoxes=False))
    dst = f"{out_dir}/{name}"
    open(dst, "wb").write(out.getvalue())
    print("wrote", dst, len(out.getvalue()), "bytes")
