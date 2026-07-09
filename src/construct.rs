//! Algebraic constructors for the balanced case (M = W = n, n even).
//!
//! Where local search hits the three-way tension (games vs courts vs
//! same-gender), a good algebraic construction sidesteps it: it builds a
//! *resolvable* saturated design directly, so several optima hold at once.
//!
//! Two constructions, both over `Z_n`:
//!
//! * [`reflection`] — universal for every even `n`. In round `r`, man `i`
//!   partners woman `(i + r) mod n` and opposes man `(c_r − i) mod n`. With the
//!   right `c_r` this saturates *both* hard ledgers (partnerships and mixed
//!   oppositions) and fills every court — but same-gender repeats run high
//!   (~`n²/4`), because opponents are forced by an involution.
//!
//! * [`hsolssom`] — the optimum for even `n ≥ 10`. It hits **all four** targets
//!   at once: both hard ledgers saturated, full courts, *and* both same-gender
//!   excesses at their floor `n/2`. Built from a holey self-orthogonal Latin
//!   square with a symmetric orthogonal mate (Berman–Wakeling). For `n ∈
//!   {4,6,8}` the full target is provably impossible, and for large `n` the
//!   backtracking search may time out; [`construct`] falls back to
//!   [`reflection`] in those cases.
//!
//! Every result is checked by the crate [`verify`](crate::verify) — the same
//! oracle used everywhere else — before being trusted.

use crate::model::{Game, Man, Roster, Round, Schedule, Team, Woman};
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::SeedableRng;

fn gcd(mut a: usize, mut b: usize) -> usize {
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    a
}

fn hole(p: usize) -> usize {
    p / 2
}

/// A game from man/woman *indices* with the cyclic partnership shift `r`
/// already applied to the women. `mi`/`mj` are men, and their partners are
/// women `wi`/`wj`.
fn game(mi: usize, wi: usize, mj: usize, wj: usize) -> Game {
    Game::new(
        Team::new(Man(mi as u16), Woman(wi as u16)),
        Team::new(Man(mj as u16), Woman(wj as u16)),
    )
}

/// Universal even-`n` construction: legal (both hard ledgers saturated) and
/// fully packed for every even `n`, at the cost of high same-gender repeats.
///
/// Round `r`: man `i` partners woman `(i+r) mod n`; men are paired by the
/// fixed-point-free involution `i ↦ (c_r − i) mod n` with `c_r = ((k−1)r + t)`.
/// Choosing `k` odd and coprime to `n` makes `d_r = k·r + t` a bijection, which
/// is exactly what saturates the mixed-opposition ledger; `t` odd keeps `c_r`
/// odd so the involution has no fixed point. `k` is chosen to spread the
/// same-gender load as much as this family allows.
pub fn reflection(roster: Roster) -> Option<Schedule> {
    let n = roster.men as usize;
    if roster.women as usize != n || n < 2 || n % 2 != 0 {
        return None;
    }

    // k odd, gcd(k, n) = 1, minimizing gcd(k-1, n) to spread same-gender load.
    let (mut k, mut best_g) = (0usize, usize::MAX);
    let mut cand = 3;
    while cand <= 4 * n + 3 {
        if gcd(cand, n) == 1 {
            let g = gcd(cand - 1, n);
            if g < best_g {
                best_g = g;
                k = cand;
                if g == 2 {
                    break;
                }
            }
        }
        cand += 2;
    }
    if k == 0 {
        return None; // no valid multiplier (shouldn't happen for even n ≥ 2)
    }
    let t = 1usize;

    let mut rounds = Vec::with_capacity(n);
    for r in 0..n {
        let c = ((k - 1) * r + t) % n;
        let mut games = Vec::with_capacity(n / 2);
        for i in 0..n {
            let j = (c + n - i % n) % n; // (c - i) mod n
            if i < j {
                games.push(game(i, (i + r) % n, j, (j + r) % n));
            }
        }
        rounds.push(Round::new(games));
    }
    Some(Schedule::new(rounds))
}

/// Bit `p` set.
fn bit(p: usize) -> u64 {
    1u64 << p
}

/// Mask of the two members of hole `k`: `{2k, 2k+1}`.
fn hole_mask(k: usize) -> u64 {
    bit(2 * k) | bit(2 * k + 1)
}

/// Cross cells `(i,j)`, `i<j`, with `hole(i) ≠ hole(j)` — the cells to fill.
fn cross_cells(n: usize) -> Vec<(usize, usize)> {
    (0..n)
        .flat_map(|i| ((i + 1)..n).map(move |j| (i, j)))
        .filter(|&(i, j)| hole(i) != hole(j))
        .collect()
}

/// Backtracking search for a self-orthogonal holey Latin square `S` of type
/// `2^m` on symbols `Z_n`, filling cross cells in a fixed order with symbols
/// tried in `order` (a permutation of `0..n`) for restart diversity. Returns
/// `S` (`-1` in hole cells) or `None` if the node budget is exhausted.
fn build_s(n: usize, order: &[usize], budget: &mut u64) -> Option<Vec<Vec<i32>>> {
    let cross = cross_cells(n);
    let mut s = vec![vec![-1i32; n]; n];
    let mut row = vec![0u64; n];
    let mut col = vec![0u64; n];
    let mut pair = vec![vec![false; n]; n];

    #[allow(clippy::too_many_arguments)]
    fn dfs(
        idx: usize,
        cross: &[(usize, usize)],
        order: &[usize],
        n: usize,
        s: &mut Vec<Vec<i32>>,
        row: &mut [u64],
        col: &mut [u64],
        pair: &mut Vec<Vec<bool>>,
        budget: &mut u64,
    ) -> bool {
        if idx == cross.len() {
            return true;
        }
        if *budget == 0 {
            return false;
        }
        *budget -= 1;
        let (i, j) = cross[idx];
        let forb = hole_mask(hole(i)) | hole_mask(hole(j));
        for &u in order {
            if forb & bit(u) != 0 || row[i] & bit(u) != 0 || col[j] & bit(u) != 0 {
                continue;
            }
            for &v in order {
                if v == u || hole(v) == hole(u) {
                    continue;
                }
                if forb & bit(v) != 0 || row[j] & bit(v) != 0 || col[i] & bit(v) != 0 {
                    continue;
                }
                if pair[u][v] || pair[v][u] {
                    continue;
                }
                s[i][j] = u as i32;
                s[j][i] = v as i32;
                row[i] |= bit(u);
                col[j] |= bit(u);
                row[j] |= bit(v);
                col[i] |= bit(v);
                pair[u][v] = true;
                pair[v][u] = true;

                if dfs(idx + 1, cross, order, n, s, row, col, pair, budget) {
                    return true;
                }

                s[i][j] = -1;
                s[j][i] = -1;
                row[i] &= !bit(u);
                col[j] &= !bit(u);
                row[j] &= !bit(v);
                col[i] &= !bit(v);
                pair[u][v] = false;
                pair[v][u] = false;
            }
        }
        false
    }

    if dfs(0, &cross, order, n, &mut s, &mut row, &mut col, &mut pair, budget) {
        Some(s)
    } else {
        None
    }
}

/// Backtracking search for a symmetric holey Latin square `W` orthogonal to
/// `S`, supplying the round label of each cross game. Symbols tried in `order`.
fn build_w(n: usize, s: &[Vec<i32>], order: &[usize], budget: &mut u64) -> Option<Vec<Vec<i32>>> {
    let cross = cross_cells(n);
    let mut w = vec![vec![-1i32; n]; n];
    let mut row = vec![0u64; n];
    let mut so = vec![vec![false; n]; n]; // (S-symbol, W-symbol) used

    #[allow(clippy::too_many_arguments)]
    fn dfs(
        idx: usize,
        cross: &[(usize, usize)],
        order: &[usize],
        n: usize,
        s: &[Vec<i32>],
        w: &mut Vec<Vec<i32>>,
        row: &mut [u64],
        so: &mut Vec<Vec<bool>>,
        budget: &mut u64,
    ) -> bool {
        if idx == cross.len() {
            return true;
        }
        if *budget == 0 {
            return false;
        }
        *budget -= 1;
        let (i, j) = cross[idx];
        let forb = hole_mask(hole(i)) | hole_mask(hole(j));
        let sij = s[i][j] as usize;
        let sji = s[j][i] as usize;
        for &wv in order {
            if forb & bit(wv) != 0 || row[i] & bit(wv) != 0 || row[j] & bit(wv) != 0 {
                continue;
            }
            if so[sij][wv] || so[sji][wv] {
                continue;
            }
            w[i][j] = wv as i32;
            w[j][i] = wv as i32;
            row[i] |= bit(wv);
            row[j] |= bit(wv);
            so[sij][wv] = true;
            so[sji][wv] = true;

            if dfs(idx + 1, cross, order, n, s, w, row, so, budget) {
                return true;
            }

            w[i][j] = -1;
            w[j][i] = -1;
            row[i] &= !bit(wv);
            row[j] &= !bit(wv);
            so[sij][wv] = false;
            so[sji][wv] = false;
        }
        false
    }

    if dfs(0, &cross, order, n, s, &mut w, &mut row, &mut so, budget) {
        Some(w)
    } else {
        None
    }
}

/// Rebuild a schedule from a cached table of `[[manA,womanA],[manB,womanB]]`
/// games grouped into rounds.
fn schedule_from_table(rounds: &[&[crate::tables::G]]) -> Schedule {
    Schedule::new(
        rounds
            .iter()
            .map(|r| {
                Round::new(
                    r.iter()
                        .map(|g| {
                            Game::new(
                                Team::new(Man(g[0][0]), Woman(g[0][1])),
                                Team::new(Man(g[1][0]), Woman(g[1][1])),
                            )
                        })
                        .collect(),
                )
            })
            .collect(),
    )
}

/// Optimal construction for even `n ≥ 10`: all four targets at once (both hard
/// ledgers saturated, full courts, both same-gender excesses at the floor
/// `n/2`). Returns `None` for `n < 10`, odd `n`, `M ≠ W`, `n > 64`, or if no
/// cached table exists and the backtracking budget is exhausted.
///
/// Cached tables (embedded, pre-verified) are used when present — instant and
/// deterministic — otherwise a randomized-restart backtracker runs, which only
/// scales to about `n = 10`.
pub fn hsolssom(roster: Roster) -> Option<Schedule> {
    let n = roster.men as usize;
    if roster.women as usize != n || n < 10 || n % 2 != 0 || n > 64 {
        return None;
    }

    if let Some(rounds) = crate::tables::cached(n) {
        return Some(schedule_from_table(rounds));
    }

    let m = n / 2;

    // Randomized-restart backtracking. This reliably builds the frame for
    // n = 10 within a second; for larger n the naive search does not scale
    // (the self-orthogonal frame gets expensive) and we return None quickly so
    // the caller falls back to `reflection`. Extending the optimum to n >= 12
    // wants cached squares or the recursive design-theory constructions.
    let mut rng = StdRng::seed_from_u64(0x5150_7ab1_e5eed ^ n as u64);
    let mut order: Vec<usize> = (0..n).collect();
    let (s, w) = 'attempts: loop {
        for _ in 0..16 {
            order.shuffle(&mut rng);
            let mut budget: u64 = 1_500_000;
            if let Some(s) = build_s(n, &order, &mut budget) {
                order.shuffle(&mut rng);
                let mut wbudget: u64 = 1_500_000;
                if let Some(w) = build_w(n, &s, &order, &mut wbudget) {
                    break 'attempts (s, w);
                }
            }
        }
        return None; // no HSOLSSOM found within the restart budget (falls back)
    };

    let mut rounds: Vec<Vec<Game>> = vec![Vec::new(); n];
    // Cross games: routed to round W[i][j].
    for i in 0..n {
        for j in (i + 1)..n {
            if hole(i) == hole(j) {
                continue;
            }
            let (si, sj) = (s[i][j] as usize, s[j][i] as usize);
            rounds[w[i][j] as usize].push(game(i, si, j, sj));
        }
    }
    // Hole gadgets: hole k contributes the two doubled same-gender pairs.
    for k in 0..m {
        let (a, b) = (2 * k, 2 * k + 1);
        rounds[a].push(game(a, a, b, b));
        rounds[b].push(game(a, b, b, a));
    }

    Some(Schedule::new(rounds.into_iter().map(Round::new).collect()))
}

/// Best available construction for a roster: the optimal [`hsolssom`] when it
/// applies and succeeds, otherwise the universal [`reflection`]. Returns `None`
/// for odd `n` or `M ≠ W` (not a constructor case — use the search heuristic).
pub fn construct(roster: Roster) -> Option<Schedule> {
    hsolssom(roster).or_else(|| reflection(roster))
}
