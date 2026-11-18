#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 DigitGram
"""Writes THIRD-PARTY-LICENSES.txt: every crate that goes into the program (as `cargo metadata` lists it, for the
Windows target), with its version, licence and the licence texts that come with its sources; crates with the same text
are listed together. The licences of these crates ask for their notice to travel with the program.
  python3 tools/third_party.py > THIRD-PARTY-LICENSES.txt        (run `cargo fetch` first so the sources are there)"""
import hashlib
import json
import subprocess
import sys
from pathlib import Path

meta = json.loads(subprocess.run(["cargo", "metadata", "--format-version", "1", "--filter-platform", "x86_64-pc-windows-gnu"],
                                 capture_output=True, text=True, check=True).stdout)
root = meta["resolve"]["root"]
by_id = {p["id"]: p for p in meta["packages"]}
reachable, todo = set(), [root]
nodes = {n["id"]: n for n in meta["resolve"]["nodes"]}
while todo:
    i = todo.pop()
    if i in reachable:
        continue
    reachable.add(i)
    for d in nodes[i]["deps"]:
        # normal and build dependencies only: what is compiled into the program
        if any(k["kind"] in (None, "normal", "build") for k in d["dep_kinds"]):
            todo.append(d["pkg"])
texts, listing = {}, []
for i in sorted(reachable, key=lambda i: by_id[i]["name"]):
    p = by_id[i]
    if i == root:
        continue
    folder = Path(p["manifest_path"]).parent
    files = sorted(f for f in folder.iterdir() if f.is_file() and f.name.upper().startswith(("LICENSE", "LICENCE", "COPYING", "UNLICENSE", "NOTICE")))
    listing.append((p["name"], p["version"], p.get("license") or p.get("license_file") or "?", p.get("repository") or ""))
    for f in files:
        text = f.read_text(encoding="utf-8", errors="replace").strip()
        key = hashlib.sha256(" ".join(text.split()).encode()).hexdigest()
        texts.setdefault(key, (text, []))[1].append(f"{p['name']} {p['version']}")
out = ["THIRD-PARTY LICENCES of NfoGlow for Windows", "=" * 44, "",
       "NfoGlow itself is free software under the GNU GPL v3 or later (see LICENSE). It contains the following",
       "libraries, each under its own licence; their texts follow the list.", "", "LIBRARIES", "---------"]
out += [f"{n} {v}  [{lic}]  {repo}".rstrip() for n, v, lic, repo in listing]
out += ["", "", "LICENCE TEXTS", "-------------"]
for text, users in sorted(texts.values(), key=lambda t: t[1][0]):
    out += ["", "=" * 78, "Used by: " + ", ".join(users), "=" * 78, text]
sys.stdout.write("\n".join(out) + "\n")
