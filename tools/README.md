# tools/ — optimal-schedule table generation

The optimal constructor (`src/construct.rs::hsolssom`) serves balanced even-`n`
schedules that hit **all four optima at once** from a cache of pre-verified
tables in `src/tables.rs`. This directory holds everything needed to *reproduce*
that cache and to *extend* it to new sizes.

## Contents

| File | Role |
| --- | --- |
| `tables/n<N>.json` | Source-of-truth verified schedules (currently n = 10, 14, 18) |
| `make_tables.py` | Transcode `tables/*.json` → `src/tables.rs` (re-verifies all four targets first) |
| `generate.py` | Generate a new table for a given `n` via OR-Tools CP-SAT |

Table JSON format (0-indexed; `rounds[r]` has `n/2` games; game = teamA vs teamB):

```json
{ "n": 14, "method": "...", "verified": true, "man_excess": 7, "woman_excess": 7,
  "rounds": [ [ [[a,x],[b,y]], ... ], ... ] }
```

## Add a size

1. `python tools/generate.py <n>` (needs `pip install ortools`) → writes `tools/tables/n<N>.json`.
2. `python tools/make_tables.py` → regenerates `src/tables.rs`.
3. `cargo test cached_tables` → the crate verifier confirms all four floors.

## Coverage & the open problem

Reproducible today: **n = 10, 14, 18** — the cases with **odd `m = n/2`**.

The **even-`m`** sizes (`n = 12, 16, 20, …`, i.e. `n ≡ 0 mod 4`) are the open
follow-up. Every method tried — finite-field over `Z_{n-1}`, randomized
recursive search, and CP-SAT (this `generate.py`) — reliably cracks odd `m` and
**stalls on even `m`** (10k+ failed attempts; CP-SAT `UNKNOWN` at 8 min for
n=16). These designs provably exist (Berman–Wakeling; HSOLSSOM(2^m) for all
m ≥ 5), so it is not an impossibility — reaching the even-`m` frames needs a
proper **recursive HSOLSSOM construction** (build the even-`m` frame by filling
holes of smaller ingredient designs) rather than search. `generate.py` is the
clean scaffold to add that on.

For everything not cached (even-`m`, `n ∈ {4,6,8}`, odd `n`, unbalanced
rosters), the crate falls back to the `reflection` construction (legal + fully
packed) and the local search — see the top-level `README.md`.
