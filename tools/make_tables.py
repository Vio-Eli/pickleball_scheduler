#!/usr/bin/env python3
"""Transcode the verified table JSONs in tools/tables/ into src/tables.rs.

Independently re-verifies all four targets before emitting; rejects any table
that fails. One schedule per n, preferring field-direct > recursive-design >
strong-solver > cpsat. The Rust side re-verifies again with the real verifier
(test `cached_tables_all_hit_full_target`).

Run from anywhere:  python tools/make_tables.py
"""
import glob
import json
import os

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
TABLES = os.path.join(REPO, "tools", "tables")
OUT = os.path.join(REPO, "src", "tables.rs")
PREF = ["field-direct", "recursive-design", "strong-solver", "cpsat"]


def verify(n, rounds):
    """Check all four targets: partnerships & mixed-opps saturated once, full
    rounds, both same-gender excesses == n/2."""
    part, mixed, mm, ww, total = {}, {}, {}, {}, 0
    if len(rounds) != n:
        return False, "num rounds %d != %d" % (len(rounds), n)
    for r in rounds:
        if len(r) != n // 2:
            return False, "round size %d != %d" % (len(r), n // 2)
        men, wom = set(), set()
        for g in r:
            (a, x), (b, y) = g[0], g[1]
            total += 1
            for key in ((a, x), (b, y)):
                part[key] = part.get(key, 0) + 1
            for key in ((a, y), (b, x)):
                mixed[key] = mixed.get(key, 0) + 1
            mm_k = tuple(sorted((a, b)))
            mm[mm_k] = mm.get(mm_k, 0) + 1
            ww_k = tuple(sorted((x, y)))
            ww[ww_k] = ww.get(ww_k, 0) + 1
            men |= {a, b}
            wom |= {x, y}
        if len(men) != n or len(wom) != n:
            return False, "round not full"
    if total != n * n // 2:
        return False, "total games %d" % total
    if len(part) != n * n or any(v > 1 for v in part.values()):
        return False, "partnerships not saturated-once"
    if len(mixed) != n * n or any(v > 1 for v in mixed.values()):
        return False, "mixed opps not saturated-once"
    me = sum(v - 1 for v in mm.values())
    we = sum(v - 1 for v in ww.values())
    if me != n // 2 or we != n // 2:
        return False, "same-gender excess %d/%d (floor %d)" % (me, we, n // 2)
    return True, "ok"


def main():
    best = {}
    for f in sorted(glob.glob(os.path.join(TABLES, "*.json"))):
        d = json.load(open(f))
        n, rounds, method = d["n"], d["rounds"], d.get("method", "?")
        ok, msg = verify(n, rounds)
        if not ok:
            print("REJECT %-20s n=%s: %s" % (os.path.basename(f), n, msg))
            continue
        rank = PREF.index(method) if method in PREF else 99
        if n not in best or rank < best[n][0]:
            best[n] = (rank, rounds, method)
        print("OK     %-20s n=%s %s" % (os.path.basename(f), n, msg))

    ns = sorted(best)
    out = []
    out.append("//! Auto-generated cached HSOLSSOM schedules -- DO NOT EDIT BY HAND.")
    out.append("//! Each schedule hits all four optima (partnerships & mixed-opps saturated")
    out.append("//! once, full courts, both same-gender excesses at floor n/2), independently")
    out.append("//! verified here and re-verified by the crate verifier in tests.")
    out.append("//! Regenerate via `python tools/make_tables.py` after adding a table JSON.")
    out.append("")
    out.append("/// A game as `[[manA, womanA], [manB, womanB]]` (0-indexed).")
    out.append("pub type G = [[u16; 2]; 2];")
    out.append("")
    for n in ns:
        _, rounds, method = best[n]
        out.append("// n=%d (%s)" % (n, method))
        out.append("static N%d: &[&[G]] = &[" % n)
        for r in rounds:
            games = ", ".join(
                "[[%d,%d],[%d,%d]]" % (g[0][0], g[0][1], g[1][0], g[1][1]) for g in r
            )
            out.append("    &[%s]," % games)
        out.append("];")
        out.append("")
    out.append("/// The cached optimal schedule for `n` (rounds of games), if one is embedded.")
    out.append("pub fn cached(n: usize) -> Option<&'static [&'static [G]]> {")
    out.append("    match n {")
    for n in ns:
        out.append("        %d => Some(N%d)," % (n, n))
    out.append("        _ => None,")
    out.append("    }")
    out.append("}")
    open(OUT, "w", encoding="utf-8", newline="\n").write("\n".join(out) + "\n")
    print("WROTE %s for n = %s" % (OUT, ns))


if __name__ == "__main__":
    main()
