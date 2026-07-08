//! Local search: polish toward the proven optima.
//!
//! ### The three-way tension
//!
//! Max games, full courts, and minimal same-gender oppositions pull against
//! each other. A game set optimized *purely* for same-gender is usually not
//! *resolvable* — it won't pack into full rounds — so chasing the same-gender
//! floor on the flat set wrecks court utilization. Since a full schedule is a
//! stated goal, we search over **round-structured** schedules instead: the move
//! generator only ever produces full rounds, so court utilization is protected
//! by construction, and we minimize same-gender within that space.
//!
//! ### Ruin-and-recreate
//!
//! At the game ceiling every partnership and mixed opposition is used exactly
//! once, so single swaps almost always collide and get rejected — hill climbing
//! is frozen at saturation. Instead we **rip out a couple of whole rounds**
//! (opening ledger slack) and **rebuild** them greedily, always taking the
//! lowest same-gender-cost game. Simulated annealing with reheating, best of
//! several independent runs, drives repeats down while keeping rounds full.
//!
//! The verifier remains the final oracle — [`optimize`] returns a `Schedule`
//! that `verify` scores exactly like any other.

use crate::construct::{hsolssom, reflection};
use crate::model::{Game, Man, Player, Roster, Round, Schedule, Team, Woman};
use crate::verify::{verify, Report};
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use std::collections::{HashMap, HashSet};

// Same-gender soft weights used *inside* the search: max multiplicity dominates
// (avoid facing the same opponent 3× while others meet once), then total excess,
// then a nudge toward balanced participation. Court utilization is handled at
// candidate-selection time, not here (see `optimize`).
const W_MAX: f64 = 1000.0;
const W_EXCESS: f64 = 10.0;
const W_SPREAD: f64 = 1.0;

/// Emphasis presets: `round_weight` is how much each extra round costs at
/// candidate selection. Higher ⇒ fuller courts at the cost of more same-gender
/// repeats. This is the one knob that slides along the Pareto frontier.
pub const EMPHASIS_VARIETY: f64 = 0.0; // fewest repeats, courts may sit idle
pub const EMPHASIS_BALANCED: f64 = 40.0; // sensible middle (default)
pub const EMPHASIS_COURTS: f64 = 100_000.0; // full courts above all else

/// Same-gender / participation stats for a schedule (no packing — rounds are
/// judged later against the real repack, so the search itself stays consistent).
#[derive(Clone, Copy)]
struct Stats {
    games: usize,
    man_excess: usize,
    woman_excess: usize,
    man_max: usize,
    woman_max: usize,
    part_spread: usize,
}

impl Stats {
    fn soft(&self) -> f64 {
        W_MAX * (self.man_max + self.woman_max) as f64
            + W_EXCESS * (self.man_excess + self.woman_excess) as f64
            + W_SPREAD * self.part_spread as f64
    }

    /// Lexicographic quality: more games first, then lower same-gender cost.
    fn better_than(&self, other: &Stats) -> bool {
        if self.games != other.games {
            self.games > other.games
        } else {
            self.soft() < other.soft()
        }
    }
}

/// Compute same-gender and participation stats for a schedule.
fn stats_of(rounds: &[Vec<Game>], roster: Roster) -> Stats {
    let mut man: HashMap<(u16, u16), usize> = HashMap::new();
    let mut woman: HashMap<(u16, u16), usize> = HashMap::new();
    let mut per_man = vec![0usize; roster.men as usize];
    let mut per_woman = vec![0usize; roster.women as usize];
    let mut games = 0usize;

    for round in rounds {
        for g in round {
            games += 1;
            let mp = g.man_pair();
            *man.entry((mp.0 .0, mp.1 .0)).or_insert(0) += 1;
            let wp = g.woman_pair();
            *woman.entry((wp.0 .0, wp.1 .0)).or_insert(0) += 1;
            per_man[g.a.man.0 as usize] += 1;
            per_man[g.b.man.0 as usize] += 1;
            per_woman[g.a.woman.0 as usize] += 1;
            per_woman[g.b.woman.0 as usize] += 1;
        }
    }

    let all = per_man.iter().chain(per_woman.iter());
    let part_spread = all.clone().max().copied().unwrap_or(0) - all.min().copied().unwrap_or(0);

    Stats {
        games,
        man_excess: man.values().map(|&c| c - 1).sum(),
        woman_excess: woman.values().map(|&c| c - 1).sum(),
        man_max: man.values().copied().max().unwrap_or(0),
        woman_max: woman.values().copied().max().unwrap_or(0),
        part_spread,
    }
}

/// Live hard-constraint ledgers plus same-gender meeting counts. Pure
/// bookkeeping — game storage lives in the round structure.
struct Ledger {
    partner: HashSet<(u16, u16)>,
    mixed: HashSet<(u16, u16)>,
    man_meet: HashMap<(u16, u16), usize>,
    woman_meet: HashMap<(u16, u16), usize>,
}

impl Ledger {
    fn new() -> Self {
        Ledger {
            partner: HashSet::new(),
            mixed: HashSet::new(),
            man_meet: HashMap::new(),
            woman_meet: HashMap::new(),
        }
    }

    fn man_key(a: Man, b: Man) -> (u16, u16) {
        if a.0 <= b.0 { (a.0, b.0) } else { (b.0, a.0) }
    }
    fn woman_key(x: Woman, y: Woman) -> (u16, u16) {
        if x.0 <= y.0 { (x.0, y.0) } else { (y.0, x.0) }
    }

    fn from_rounds(rounds: &[Vec<Game>]) -> Self {
        let mut l = Ledger::new();
        for round in rounds {
            for g in round {
                l.add(g);
            }
        }
        l
    }

    fn add(&mut self, g: &Game) {
        for (m, w) in g.partnerships() {
            self.partner.insert((m.0, w.0));
        }
        for (m, w) in g.mixed_opps() {
            self.mixed.insert((m.0, w.0));
        }
        *self.man_meet.entry(Self::man_key(g.a.man, g.b.man)).or_insert(0) += 1;
        *self
            .woman_meet
            .entry(Self::woman_key(g.a.woman, g.b.woman))
            .or_insert(0) += 1;
    }

    /// A legal game from four players, or `None` if neither orientation avoids
    /// a repeated partnership or mixed opposition.
    fn legal_game(&self, a: Man, b: Man, x: Woman, y: Woman) -> Option<Game> {
        if !self.partner.contains(&(a.0, x.0))
            && !self.partner.contains(&(b.0, y.0))
            && !self.mixed.contains(&(a.0, y.0))
            && !self.mixed.contains(&(b.0, x.0))
        {
            return Some(Game::new(Team::new(a, x), Team::new(b, y)));
        }
        if !self.partner.contains(&(a.0, y.0))
            && !self.partner.contains(&(b.0, x.0))
            && !self.mixed.contains(&(a.0, x.0))
            && !self.mixed.contains(&(b.0, y.0))
        {
            return Some(Game::new(Team::new(a, y), Team::new(b, x)));
        }
        None
    }

    fn soft_cost(&self, a: Man, b: Man, x: Woman, y: Woman) -> usize {
        self.man_meet.get(&Self::man_key(a, b)).copied().unwrap_or(0)
            + self.woman_meet.get(&Self::woman_key(x, y)).copied().unwrap_or(0)
    }
}

/// Build one round: fill up to `courts` player-disjoint games from players not
/// yet used this round, always taking a minimum same-gender-cost legal game
/// (ties broken uniformly). Returns the games; empty if none can be formed.
fn build_round(led: &mut Ledger, roster: Roster, courts: u16, rng: &mut StdRng) -> Vec<Game> {
    let mut used_m: HashSet<u16> = HashSet::new();
    let mut used_w: HashSet<u16> = HashSet::new();
    let mut round = Vec::new();

    for _ in 0..courts {
        let mut men: Vec<Man> = roster.men_iter().filter(|m| !used_m.contains(&m.0)).collect();
        let mut women: Vec<Woman> = roster
            .women_iter()
            .filter(|w| !used_w.contains(&w.0))
            .collect();
        // Shuffle so restarts explore different packings, not just one shape.
        men.shuffle(rng);
        women.shuffle(rng);

        let mut best_cost = usize::MAX;
        let mut best: Option<Game> = None;
        let mut ties = 0u32;

        for i in 0..men.len() {
            for j in (i + 1)..men.len() {
                let (a, b) = (men[i], men[j]);
                for k in 0..women.len() {
                    for l in (k + 1)..women.len() {
                        let (x, y) = (women[k], women[l]);
                        if let Some(g) = led.legal_game(a, b, x, y) {
                            let c = led.soft_cost(a, b, x, y);
                            if c < best_cost {
                                best_cost = c;
                                best = Some(g);
                                ties = 1;
                            } else if c == best_cost {
                                ties += 1;
                                if rng.random_range(0..ties) == 0 {
                                    best = Some(g);
                                }
                            }
                        }
                    }
                }
            }
        }

        match best {
            Some(g) => {
                led.add(&g);
                used_m.insert(g.a.man.0);
                used_m.insert(g.b.man.0);
                used_w.insert(g.a.woman.0);
                used_w.insert(g.b.woman.0);
                round.push(g);
            }
            None => break,
        }
    }

    round
}

/// Append full rounds until no more games can be formed from the current
/// ledger state.
fn build_rounds(led: &mut Ledger, roster: Roster, courts: u16, rng: &mut StdRng) -> Vec<Vec<Game>> {
    let mut rounds = Vec::new();
    loop {
        let round = build_round(led, roster, courts, rng);
        if round.is_empty() {
            break;
        }
        rounds.push(round);
    }
    rounds
}

/// Court-first construction: best of many full-round builds, minimizing the
/// round count first (fullest courts) and same-gender second. Reliably finds
/// the resolvable, fully-packed corner of the frontier.
fn court_first(roster: Roster, courts: u16, restarts: u32, rng: &mut StdRng) -> Vec<Vec<Game>> {
    let mut best: Option<Vec<Vec<Game>>> = None;
    // Maximize games first, then minimize rounds (fullest courts), then
    // same-gender. Without the games term this degenerates to a tiny schedule.
    let mut best_key = (i64::MAX, usize::MAX, f64::MAX);
    for _ in 0..restarts.max(1) {
        let mut led = Ledger::new();
        let rounds = build_rounds(&mut led, roster, courts, rng);
        let st = stats_of(&rounds, roster);
        let key = (-(st.games as i64), rounds.len(), st.soft());
        if key < best_key {
            best_key = key;
            best = Some(rounds);
        }
    }
    best.unwrap_or_default()
}

/// One ruin-and-recreate annealing run over round-structured schedules,
/// minimizing same-gender oppositions. Returns the best game set found.
fn anneal(roster: Roster, courts: u16, iters: u32, rng: &mut StdRng) -> Vec<Vec<Game>> {
    let mut led = Ledger::new();
    let mut cur = build_rounds(&mut led, roster, courts, rng);
    let mut cur_stats = stats_of(&cur, roster);
    let mut best = cur.clone();
    let mut best_stats = cur_stats;

    let (t0, t_end) = (12.0_f64, 0.05_f64);
    let reheat_span = (iters / 4).max(2_000);
    let mut cycle_pos = 0u32;
    let mut since_improve = 0u32;

    for _ in 0..iters {
        let frac = cycle_pos as f64 / reheat_span as f64;
        let t = t0 * (t_end / t0).powf(frac.min(1.0));

        // Ruin: drop 1–2 whole rounds, opening ledger slack.
        let mut trial = cur.clone();
        if !trial.is_empty() {
            let k = rng.random_range(1..=2usize).min(trial.len());
            for _ in 0..k {
                let idx = rng.random_range(0..trial.len());
                trial.swap_remove(idx);
            }
        }
        // Recreate: rebuild full rounds from the reduced state.
        let mut trial_led = Ledger::from_rounds(&trial);
        trial.extend(build_rounds(&mut trial_led, roster, courts, rng));
        let ts = stats_of(&trial, roster);

        let accept = if ts.games != cur_stats.games {
            ts.games > cur_stats.games
        } else {
            let delta = ts.soft() - cur_stats.soft();
            delta <= 0.0 || rng.random::<f64>() < (-delta / t).exp()
        };

        if accept {
            cur = trial;
            cur_stats = ts;
        }

        cycle_pos += 1;
        if cur_stats.better_than(&best_stats) {
            best = cur.clone();
            best_stats = cur_stats;
            since_improve = 0;
        } else {
            since_improve += 1;
        }

        if since_improve >= reheat_span {
            cur = best.clone();
            cur_stats = best_stats;
            cycle_pos = 0;
            since_improve = 0;
        }
    }

    best
}

/// Repack a game set into as few rounds as possible (each round ≤ `courts`
/// player-disjoint games), most-constrained-player first, best of many
/// randomized tie-breaks. Tightens the round-structured result without
/// changing the game set (so same-gender stats are preserved).
fn repack(games: &[Game], courts: u16, rng: &mut StdRng) -> Schedule {
    let c = courts as usize;
    let tries = 400;
    let mut best: Option<Schedule> = None;
    let mut best_key = (usize::MAX, i64::MIN);

    for _ in 0..tries {
        let mut remaining = games.to_vec();
        remaining.shuffle(rng);

        let mut rounds: Vec<Vec<Game>> = Vec::new();
        while !remaining.is_empty() {
            let mut deg: HashMap<Player, usize> = HashMap::new();
            for g in &remaining {
                for p in g.players() {
                    *deg.entry(p).or_insert(0) += 1;
                }
            }

            let mut round: Vec<Game> = Vec::new();
            let mut used: HashSet<Player> = HashSet::new();
            while round.len() < c {
                let mut pick: Option<usize> = None;
                let mut best_score = -1i64;
                for (i, g) in remaining.iter().enumerate() {
                    if g.players().iter().any(|p| used.contains(p)) {
                        continue;
                    }
                    let score: i64 = g.players().iter().map(|p| deg[p] as i64).sum();
                    if score > best_score {
                        best_score = score;
                        pick = Some(i);
                    }
                }
                match pick {
                    Some(i) => {
                        let g = remaining.swap_remove(i);
                        for p in g.players() {
                            used.insert(p);
                        }
                        round.push(g);
                    }
                    None => break,
                }
            }
            rounds.push(round);
        }

        let fill: i64 = rounds.iter().map(|r| (r.len() * r.len()) as i64).sum();
        let key = (rounds.len(), -fill);
        if key < best_key {
            best_key = key;
            best = Some(Schedule::new(rounds.into_iter().map(Round::new).collect()));
        }
    }

    best.unwrap_or_default()
}

/// Full pipeline: several independent round-structured ruin-and-recreate runs
/// (best of N kills seed-to-seed variance), then a final repack of the winner.
/// `ls_iters` is the total step budget, split across the runs.
///
/// `round_weight` slides along the court-fullness ⇄ same-gender frontier — see
/// [`EMPHASIS_VARIETY`], [`EMPHASIS_BALANCED`], [`EMPHASIS_COURTS`].
pub fn optimize(
    roster: Roster,
    courts: u16,
    ls_iters: u32,
    round_weight: f64,
    seed: u64,
) -> Schedule {
    let mut rng = StdRng::seed_from_u64(seed);
    let n = roster.men as usize;
    let balanced_even = roster.women as usize == n && n >= 2 && n % 2 == 0;

    // Optimal algebraic construction: when an HSOLSSOM build succeeds it is
    // provably optimal on *all four* objectives at once (both hard ledgers
    // saturated, full courts, both same-gender excesses at floor), so it
    // dominates every heuristic candidate under any emphasis — return it
    // directly. Its native layout is n rounds of n/2 games; if the caller has
    // fewer courts than n/2 we keep the (still-optimal) game set and repack.
    if balanced_even {
        if let Some(sched) = hsolssom(roster) {
            if verify(&sched, roster, courts).is_legal() {
                if courts as usize >= n / 2 {
                    return sched;
                }
                let games: Vec<Game> = sched.all_games().copied().collect();
                return repack(&games, courts, &mut rng);
            }
        }
    }

    // Otherwise assemble candidate schedules spanning the frontier and let the
    // emphasis-weighted selection choose.
    let mut candidates: Vec<Schedule> = Vec::new();

    // Court-first candidate: keep its own fully-packed round structure (the
    // low-round corner) — repacking it blindly would only scatter it.
    let cf = court_first(roster, courts, 200, &mut rng);
    candidates.push(Schedule::new(cf.into_iter().map(Round::new).collect()));

    // Reflection candidate (balanced even n): deterministic, legal, fully
    // packed — a strong court-emphasis option when HSOLSSOM isn't available.
    if balanced_even {
        if let Some(refl) = reflection(roster) {
            if courts as usize >= n / 2 {
                candidates.push(refl);
            } else {
                let games: Vec<Game> = refl.all_games().copied().collect();
                candidates.push(repack(&games, courts, &mut rng));
            }
        }
    }

    // Variety candidates: ruin-and-recreate (low same-gender corner), which
    // have no round structure of their own, so repack them tightly.
    let starts = 6u32;
    let per = (ls_iters / starts).max(1);
    for _ in 0..starts {
        let rounds = anneal(roster, courts, per, &mut rng);
        let games: Vec<Game> = rounds.into_iter().flatten().collect();
        candidates.push(repack(&games, courts, &mut rng));
    }

    // Select using the verifier's *true* stats (same oracle as the output), so
    // the choice is consistent. `round_weight` slides court-fullness vs repeats.
    let mut best: Option<((i64, f64), Schedule)> = None;
    for cand in candidates {
        let report = verify(&cand, roster, courts);
        let cost = selection_cost(&report, round_weight);
        if best.as_ref().map_or(true, |(bc, _)| cost < *bc) {
            best = Some((cost, cand));
        }
    }

    best.expect("at least one candidate").1
}

/// Selection cost for a finished schedule: maximize games first (as `-games`),
/// then a `round_weight`-tunable blend of court utilization and same-gender
/// repeats. Uses the verifier's real, repacked stats.
fn selection_cost(r: &Report, round_weight: f64) -> (i64, f64) {
    let soft = round_weight * r.rounds as f64
        + W_MAX * (r.man_max_meetings + r.woman_max_meetings) as f64
        + W_EXCESS * (r.man_repeat_excess + r.woman_repeat_excess) as f64
        + W_SPREAD * r.participation_spread() as f64;
    (-(r.games as i64), soft)
}
