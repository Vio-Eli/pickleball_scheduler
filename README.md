# Pickleball Scheduler

A scheduler for **mixed-doubles round-robin** play: given a set of men, a set of
women, and some courts, build a schedule of games that is fair, packed, and
provably close to optimal.

## The problem

A *game* is a mixed-doubles match — two teams, each a `(man, woman)` pair, so
four distinct players (two men, two women). Every game touches four ledgers:

| Ledger | What it tracks | Rule |
| --- | --- | --- |
| **Partnerships** | the two `(man, woman)` pairs that team up | each used **at most once** (hard) |
| **Mixed oppositions** | each man vs the *other* team's woman | each occurs **at most once** (hard) |
| **Man–man oppositions** | the `{man, man}` facing off | repeats **minimized** (soft) |
| **Woman–woman oppositions** | the `{woman, woman}` facing off | repeats **minimized** (soft) |

Partnering and mixed-opposing are genuinely separate: `M1` may *partner* `W1` in
one game and *oppose* `W1` in another. On top of the ledgers we want to keep
every court busy every round and spread byes fairly.

### Provable bounds (balanced roster, `n` men + `n` women)

* **Max games = `n²/2`** — the partnership ledger caps it (`n²` partnerships,
  two per game). For 6×6 that's 18 games in 6 full rounds of 3 courts.
* **Same-gender repeats have a floor.** With one man–man opposition per game and
  only `C(n,2)` distinct pairs, at the ceiling at least `n/2` man–man pairs (and
  `n/2` woman–woman pairs) *must* repeat. "As few as possible" means this floor,
  not zero.

These bounds live in code on [`Roster`](src/model.rs), and the
[verifier](src/verify.rs) scores every schedule against them.

## Architecture

| Module | Role |
| --- | --- |
| [`model`](src/model.rs) | Domain types (`Man`, `Woman`, `Team`, `Game`, `Round`, `Schedule`, `Roster`) and the bounds |
| [`verify`](src/verify.rs) | The single source of truth: legality + full quality report vs. the bounds |
| [`greedy`](src/greedy.rs) | Round-based randomized-greedy constructor — a fast "good enough" seed |
| [`search`](src/search.rs) | Ruin-and-recreate local search + court-first builder; the main optimizer |
| [`report`](src/report.rs) | The court grid and quality summary |

Pipeline: **construct → verify → report.**

## The three-way tension

Max games, full courts, and minimal same-gender repeats **pull against each
other**. A game set optimized purely for fewest repeats usually isn't
*resolvable* — it won't pack into full rounds — so chasing the same-gender floor
costs court utilization, and vice versa. This is a genuine Pareto tradeoff, so
the optimizer exposes an **emphasis** knob:

| Emphasis | 6×6 result |
| --- | --- |
| `courts` | 18 games, **6 full rounds (100% courts)**, ~15 same-gender repeats |
| `variety` | 18 games, **same-gender floor (3+3)**, ~65% court utilization |
| `balanced` | picks whichever corner scores better (default) |

Hitting *both* corners at once — full courts **and** the repeat floor — is what
the algebraic constructor is for (see roadmap); a general local search can't
reliably reach it for balanced rosters.

## Usage

```
cargo run -- [men] [women] [courts] [emphasis] [ls_iters] [seed]
# defaults: 6 6 3 balanced 40000
# emphasis: courts | balanced | variety
```

## Roadmap

- [x] Domain model + verifier scored against the proven bounds
- [x] Round-based greedy seed (soft same-gender, fills courts)
- [x] **Local search** — ruin-and-recreate on the same-gender objective, plus a
      court-first builder, with an emphasis knob along the Pareto frontier
- [ ] **Optimal constructor** — cyclic/algebraic build that reaches the game
      ceiling *and* the same-gender floor *and* full courts at once for balanced
      cases (with a short search for the small exceptional `n`)
- [ ] **Part 2** — target modes: each player plays exactly *N* games, and/or a
      hard total-game cap, relaxing the once-only rules toward their minimum
- [ ] **Exact solver** (CP-SAT / ILP) as an opt-in "prove it's optimal" mode
- [ ] GUI, team/single-list input
