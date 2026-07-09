#!/usr/bin/env python3
"""Generate a balanced-even-n HSOLSSOM mixed-doubles schedule via CP-SAT.

Self-contained (needs only OR-Tools: `pip install ortools`). Builds a
holey self-orthogonal Latin square S (type 2^m) with a symmetric orthogonal
mate W, then reduces to n rounds hitting all four optima, verifies, and writes
`tools/tables/n<N>.json` in the format `make_tables.py` consumes.

    python tools/generate.py <n> [max_seconds] [workers]

Status: reliably solves ODD m = n/2 (n = 10, 14, 18, ...). For EVEN m
(n = 12, 16, 20, ..., i.e. n = 0 mod 4) CP-SAT returns UNKNOWN even at long
timeouts -- those frames need a *recursive* HSOLSSOM construction, which is the
open follow-up. This file is the clean scaffold to build that on.
"""
import json
import os
import sys
import time

from ortools.sat.python import cp_model

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def hole(p):
    return p // 2


def reduce_to_rounds(n, S, W):
    """(S, W) -> n rounds of games. Cross cell (i<j) -> round W[i][j]; each of
    the m holes contributes the two doubled same-gender games."""
    m = n // 2
    rounds = [[] for _ in range(n)]
    for i in range(n):
        for j in range(i + 1, n):
            if hole(i) == hole(j):
                continue
            r = W[i][j]
            rounds[r].append(((i, S[i][j]), (j, S[j][i])))
    for k in range(m):
        a, b = 2 * k, 2 * k + 1
        rounds[a].append(((a, a), (b, b)))
        rounds[b].append(((a, b), (b, a)))
    return rounds


def verify(n, rounds):
    part, mixed, mm, ww, total = {}, {}, {}, {}, 0
    for r in rounds:
        men, wom = set(), set()
        for (A, B) in r:
            a, x = A
            b, y = B
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
        if len(men) != n or len(wom) != n or len(r) != n // 2:
            return False, None, None
    me = sum(v - 1 for v in mm.values())
    we = sum(v - 1 for v in ww.values())
    ok = (
        total == n * n // 2
        and len(rounds) == n
        and len(part) == n * n
        and all(v == 1 for v in part.values())
        and len(mixed) == n * n
        and all(v == 1 for v in mixed.values())
        and me == n // 2
        and we == n // 2
    )
    return ok, me, we


def build_and_solve(n, max_time, workers, seed):
    m = n // 2
    cross = [(i, j) for i in range(n) for j in range(n) if i != j and hole(i) != hole(j)]

    def dom(i, j):
        return [s for s in range(n) if hole(s) != hole(i) and hole(s) != hole(j)]

    model = cp_model.CpModel()
    S, W = {}, {}
    for (i, j) in cross:
        S[i, j] = model.NewIntVarFromDomain(cp_model.Domain.FromValues(dom(i, j)), f"S_{i}_{j}")
    for (i, j) in cross:
        if i < j:
            v = model.NewIntVarFromDomain(cp_model.Domain.FromValues(dom(i, j)), f"W_{i}_{j}")
            W[i, j] = v
            W[j, i] = v

    for i in range(n):
        model.AddAllDifferent([S[i, j] for j in range(n) if (i, j) in S])
    for j in range(n):
        model.AddAllDifferent([S[i, j] for i in range(n) if (i, j) in S])
    for i in range(n):
        model.AddAllDifferent([W[i, j] for j in range(n) if (i, j) in W])

    hS, hW = {}, {}
    for (i, j) in cross:
        hS[i, j] = model.NewIntVar(0, m - 1, f"hS_{i}_{j}")
        model.AddDivisionEquality(hS[i, j], S[i, j], 2)
        hW[i, j] = model.NewIntVar(0, m - 1, f"hW_{i}_{j}")
        model.AddDivisionEquality(hW[i, j], W[i, j], 2)
    for (i, j) in cross:
        if i < j:
            model.Add(hS[i, j] != hS[j, i])  # cross-game women not a hole-pair
        model.Add(hS[i, j] != hW[i, j])  # round-hole alignment

    so = []
    for (i, j) in cross:
        e = model.NewIntVar(0, n * n - 1, f"so_{i}_{j}")
        model.Add(e == n * S[i, j] + S[j, i])
        so.append(e)
    model.AddAllDifferent(so)  # S self-orthogonal

    orth = []
    for (i, j) in cross:
        e = model.NewIntVar(0, n * n - 1, f"orth_{i}_{j}")
        model.Add(e == n * S[i, j] + W[i, j])
        orth.append(e)
    model.AddAllDifferent(orth)  # S _|_ W

    solver = cp_model.CpSolver()
    solver.parameters.max_time_in_seconds = max_time
    solver.parameters.num_search_workers = workers
    solver.parameters.random_seed = seed
    status = solver.Solve(model)
    if status not in (cp_model.OPTIMAL, cp_model.FEASIBLE):
        return None, solver.StatusName(status)

    Sm = [[None] * n for _ in range(n)]
    Wm = [[None] * n for _ in range(n)]
    for (i, j) in cross:
        Sm[i][j] = solver.Value(S[i, j])
        Wm[i][j] = solver.Value(W[i, j])
    return reduce_to_rounds(n, Sm, Wm), solver.StatusName(status)


def main():
    n = int(sys.argv[1])
    max_time = float(sys.argv[2]) if len(sys.argv) > 2 else 120.0
    workers = int(sys.argv[3]) if len(sys.argv) > 3 else 8
    if n < 10 or n % 2 or n % 4 == 0:
        print(f"n={n}: constructor domain is even n>=10 with odd m=n/2; "
              f"n=0 mod 4 needs a recursive construction (not this scaffold).")
    t0 = time.time()
    rounds, status = build_and_solve(n, max_time, workers, seed=0)
    dt = time.time() - t0
    if rounds is None:
        print(f"n={n} CP-SAT {status} (no solution) {dt:.1f}s")
        sys.exit(1)
    ok, me, we = verify(n, rounds)
    print(f"n={n} CP-SAT {status} {dt:.1f}s verify={ok} man={me} woman={we}")
    if ok:
        out = {"n": n, "method": "cpsat", "verified": True, "man_excess": me,
               "woman_excess": we, "rounds": [[[list(t) for t in g] for g in r] for r in rounds]}
        path = os.path.join(REPO, "tools", "tables", f"n{n}.json")
        json.dump(out, open(path, "w"))
        print(f"wrote {path} -- now run: python tools/make_tables.py")


if __name__ == "__main__":
    main()
