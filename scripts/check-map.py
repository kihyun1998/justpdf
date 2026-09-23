#!/usr/bin/env python3
"""Check map notes: sections, ## Code symbols, links/anchors, invariant reciprocity.

usage: scripts/check-map.py <repo-root> [note.md ...]   (no notes = every note under docs/map)
"""
import os
import re
import sys
import unicodedata

TERRITORY = ["What it is", "Governing decisions", "Design model", "Code",
             "Reference behaviour", "Cross-cutting invariants", "Blast radius",
             "Known holes / open"]
INVARIANT = ["The fact", "Why it is cross-cutting", "Territories it holds in",
             "What a violation looks like", "Discovery history", "Where it will recur"]

root = os.path.abspath(sys.argv[1])
MAP = os.path.join(root, "docs", "map")


def blank_code(text):
    """Blank fenced blocks and code spans (keep offsets/newlines)."""
    def sp(m):
        return re.sub(r"[^\n]", " ", m.group(0))
    text = re.sub(r"(?ms)^(```|~~~).*?^\1[^\n]*$", sp, text)
    text = re.sub(r"`[^`\n]*`", sp, text)
    return text


def slug(h):
    h = h.strip().lower()
    out = []
    for ch in h:
        cat = unicodedata.category(ch)
        if ch in " -":
            out.append("-" if ch == " " else ch)
        elif ch == "_" or cat[0] in "LN" or cat == "Mn":
            out.append(ch)
    return "".join(out)


def headings(path):
    try:
        text = open(path, encoding="utf-8").read()
    except OSError:
        return None
    text = re.sub(r"(?ms)^(```|~~~).*?^\1[^\n]*$", "", text)
    seen, out = {}, set()
    for m in re.finditer(r"(?m)^#{1,6}\s+(.+?)\s*#*\s*$", text):
        s = slug(re.sub(r"`", "", re.sub(r"\[([^\]]*)\]\([^)]*\)", r"\1", m.group(1))))
        n = seen.get(s, 0)
        out.add(s if n == 0 else f"{s}-{n}")
        seen[s] = n + 1
    return out


def sections(text):
    parts = re.split(r"(?m)^## (.+?)\s*$", text)
    return {parts[i].strip(): parts[i + 1] for i in range(1, len(parts), 2)}


def kind(path):
    rel = os.path.relpath(path, MAP)
    if rel.startswith("territory" + os.sep):
        try:
            if open(path, encoding="utf-8").readline().startswith("# Aggregate"):
                return "aggregate"
        except OSError:
            pass
        return "territory"
    if rel.startswith("invariant" + os.sep):
        return "invariant"
    return "other"


def links(path, text):
    out = []
    for m in re.finditer(r"\[[^\]]*\]\(([^)\s]+)\)", blank_code(text)):
        out.append(m.group(1))
    return out


def resolve(path, target):
    if re.match(r"^[a-z]+:", target):
        return None, None
    f, _, a = target.partition("#")
    p = os.path.normpath(os.path.join(os.path.dirname(path), f)) if f else path
    return p, a


def check(path, errs):
    text = open(path, encoding="utf-8").read()
    k = kind(path)
    secs = sections(text)
    want = {"territory": TERRITORY, "invariant": INVARIANT,
            "aggregate": ["Why they sit together"]}.get(k, [])
    for s in want:
        if s not in secs:
            errs.append(f"{path}: missing section '## {s}'")
        elif not secs[s].strip():
            errs.append(f"{path}: empty section '## {s}' (write **None.** + why)")
    # links + anchors
    for t in links(path, text):
        p, a = resolve(path, t)
        if p is None:
            continue
        if not os.path.exists(p):
            errs.append(f"{path}: broken link -> {t}")
            continue
        if a and p.endswith(".md"):
            hs = headings(p)
            if hs is not None and a not in hs:
                errs.append(f"{path}: broken anchor -> {t}")
    # ## Code symbols
    if k == "territory" and "Code" in secs:
        body = secs["Code"]
        if not body.strip().startswith("**None.**"):
            for line in body.splitlines():
                ticks = re.findall(r"`([^`]+)`", line)
                if not line.lstrip().startswith("- ") or not ticks:
                    continue
                fp = os.path.join(root, ticks[0])
                if not os.path.exists(fp):
                    errs.append(f"{path}: Code path missing: {ticks[0]}")
                    continue
                files = [fp] if os.path.isfile(fp) else [
                    os.path.join(d, n) for d, _, ns in os.walk(fp) for n in ns
                    if not d.startswith(os.path.join(root, "target"))]
                src = ""
                for x in files:
                    try:
                        src += open(x, encoding="utf-8", errors="ignore").read()
                    except OSError:
                        pass
                for sym in ticks[1:]:
                    last = re.split(r"::|\.", sym.strip("()"))[-1]
                    last = re.sub(r"\(.*$", "", last)
                    if not re.search(r"(?<![A-Za-z0-9_])" + re.escape(last) + r"(?![A-Za-z0-9_])", src):
                        errs.append(f"{path}: symbol not found: {sym} in {ticks[0]}")
    return k, secs


def targets(path, body):
    out = set()
    for t in links(path, body):
        p, _ = resolve(path, t)
        if p:
            out.add(os.path.normpath(p))
    return out


def main():
    notes = [os.path.abspath(n) for n in sys.argv[2:]] or [
        os.path.join(d, n) for d, _, ns in os.walk(MAP) for n in ns if n.endswith(".md")]
    errs = []
    info = {}
    for n in sorted(notes):
        info[n] = check(n, errs)
    # reciprocity over the whole map (cheap), reported only for the given notes
    allnotes = [os.path.join(d, n) for d, _, ns in os.walk(MAP) for n in ns if n.endswith(".md")]
    for inv in allnotes:
        if kind(inv) != "invariant":
            continue
        isecs = sections(open(inv, encoding="utf-8").read())
        for terr in targets(inv, isecs.get("Territories it holds in", "")):
            if kind(terr) != "territory" or not os.path.exists(terr):
                continue
            tsecs = sections(open(terr, encoding="utf-8").read())
            back = targets(terr, tsecs.get("Cross-cutting invariants", ""))
            if os.path.normpath(inv) not in back and (inv in notes or terr in notes):
                errs.append(f"{terr}: does not claim invariant {os.path.relpath(inv, MAP)} back")
    for terr in allnotes:
        if kind(terr) != "territory":
            continue
        tsecs = sections(open(terr, encoding="utf-8").read())
        for inv in targets(terr, tsecs.get("Cross-cutting invariants", "")):
            if kind(inv) != "invariant" or not os.path.exists(inv):
                continue
            isecs = sections(open(inv, encoding="utf-8").read())
            if os.path.normpath(terr) not in targets(inv, isecs.get("Territories it holds in", "")) and (inv in notes or terr in notes):
                errs.append(f"{inv}: does not list territory {os.path.relpath(terr, MAP)} back")
    for e in errs:
        print("FAIL", os.path.relpath(e.split(":")[0], root) + ":" + e.split(":", 1)[1])
    print(f"{len(notes)} note(s), {len(errs)} problem(s)")
    sys.exit(1 if errs else 0)


main()
