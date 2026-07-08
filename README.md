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
| [`greedy`](src/greedy.rs) | Round-based randomized-greedy constructor — the fast "good enough" seed |
| [`report`](src/report.rs) | The court grid and quality summary |

Pipeline: **construct → verify → report.**

## Usage

```
cargo run -- [men] [women] [courts] [restarts] [seed]
# defaults: 6 6 3 500
```

Example (6 men × 6 women, 3 courts) hits the optimum on games and packing:

```
Games:            18 / 18 max   ← ceiling ✓
Rounds:           6   (court utilization 100%)
Man–man repeats:  6 extra (floor 3)      ← soft objective, still has headroom
Woman–woman rpts: 9 extra (floor 3)
Games per player: 6–6   (spread 0)
```

## Roadmap

- [x] Domain model + verifier scored against the proven bounds
- [x] Round-based greedy seed (soft same-gender, fills courts)
- [ ] **Optimal constructor** — cyclic/algebraic build that reaches the game
      ceiling *and* the same-gender floor for balanced cases (with a short
      search for the small exceptional `n`)
- [ ] **Local search** on top of the seed to squeeze same-gender repeats down
- [ ] **Part 2** — target modes: each player plays exactly *N* games, and/or a
      hard total-game cap, relaxing the once-only rules toward their minimum
- [ ] **Exact solver** (CP-SAT / ILP) as an opt-in "prove it's optimal" mode
- [ ] GUI, team/single-list input
