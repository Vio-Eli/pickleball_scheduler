//! A round-based randomized-greedy constructor — the "fast-and-good" seed.
//!
//! Unlike the original prototype (which found games first, then packed them
//! loosely into rounds), this builds full rounds directly: each round greedily
//! fills every court with a player-disjoint game, always preferring the game
//! whose same-gender oppositions have been seen least. Same-gender repeats are
//! *soft* here — never forbidden, only minimized — so the search can push all
//! the way to the partnership ceiling instead of stalling early.
//!
//! It is a heuristic: good, not provably optimal. Local search and the exact
//! solver build on top of it. Multiple randomized restarts are run and the
//! best (by [`verify`](crate::verify)) is returned.

use crate::model::{Game, Man, Roster, Round, Schedule, Team, Woman};
use crate::verify::verify;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use std::cmp::Reverse;
use std::collections::{HashMap, HashSet};

/// Running record of what has already been played, across all rounds.
struct Ledgers {
    /// `(man, woman)` partnerships already used (hard: never reuse).
    partners: HashSet<(u16, u16)>,
    /// `(man, woman)` mixed oppositions already used (hard: never reuse).
    mixed: HashSet<(u16, u16)>,
    /// How many times each man–man pair has opposed (soft: minimize).
    man_meet: HashMap<(u16, u16), usize>,
    /// How many times each woman–woman pair has opposed (soft: minimize).
    woman_meet: HashMap<(u16, u16), usize>,
}

impl Ledgers {
    fn new() -> Self {
        Ledgers {
            partners: HashSet::new(),
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

    /// Soft cost of same-gender oppositions for men `{a,b}` and women `{x,y}`:
    /// how many times these pairs have already met. Lower is better.
    fn soft_cost(&self, a: Man, b: Man, x: Woman, y: Woman) -> usize {
        self.man_meet.get(&Self::man_key(a, b)).copied().unwrap_or(0)
            + self.woman_meet.get(&Self::woman_key(x, y)).copied().unwrap_or(0)
    }

    fn partner_free(&self, m: Man, w: Woman) -> bool {
        !self.partners.contains(&(m.0, w.0))
    }

    fn mixed_free(&self, m: Man, w: Woman) -> bool {
        !self.mixed.contains(&(m.0, w.0))
    }

    /// Build a *legal* game from four players if either partnership
    /// orientation is available, else `None`. Both orientations share the same
    /// man- and woman-pair, so the soft cost is identical either way; we only
    /// need one that respects the hard ledgers.
    fn legal_game(&self, a: Man, b: Man, x: Woman, y: Woman) -> Option<Game> {
        // Orientation 1: (a,x) partners, (b,y) partners; cross-opps (a,y),(b,x).
        if self.partner_free(a, x)
            && self.partner_free(b, y)
            && self.mixed_free(a, y)
            && self.mixed_free(b, x)
        {
            return Some(Game::new(Team::new(a, x), Team::new(b, y)));
        }
        // Orientation 2: (a,y) partners, (b,x) partners; cross-opps (a,x),(b,y).
        if self.partner_free(a, y)
            && self.partner_free(b, x)
            && self.mixed_free(a, x)
            && self.mixed_free(b, y)
        {
            return Some(Game::new(Team::new(a, y), Team::new(b, x)));
        }
        None
    }

    fn commit(&mut self, g: &Game) {
        for (m, w) in g.partnerships() {
            self.partners.insert((m.0, w.0));
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
}

/// Fill one round: for each court, pick the minimum-soft-cost legal game among
/// players not yet used this round (ties broken uniformly at random). Stops
/// when a court can't be filled — that's the natural end of the round.
fn build_round(led: &mut Ledgers, roster: Roster, courts: u16, rng: &mut StdRng) -> Round {
    let mut used_m: HashSet<u16> = HashSet::new();
    let mut used_w: HashSet<u16> = HashSet::new();
    let mut games = Vec::new();

    for _ in 0..courts {
        let mut men: Vec<Man> = roster.men_iter().filter(|m| !used_m.contains(&m.0)).collect();
        let mut women: Vec<Woman> = roster
            .women_iter()
            .filter(|w| !used_w.contains(&w.0))
            .collect();
        men.shuffle(rng);
        women.shuffle(rng);

        // Reservoir-sample among all minimum-cost legal games for fairness.
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
                led.commit(&g);
                used_m.insert(g.a.man.0);
                used_m.insert(g.b.man.0);
                used_w.insert(g.a.woman.0);
                used_w.insert(g.b.woman.0);
                games.push(g);
            }
            None => break, // no legal game for the remaining players
        }
    }

    Round::new(games)
}

/// One greedy pass: keep building rounds until a round comes up empty.
fn greedy_once(roster: Roster, courts: u16, rng: &mut StdRng) -> Schedule {
    let mut led = Ledgers::new();
    let mut rounds = Vec::new();
    loop {
        let round = build_round(&mut led, roster, courts, rng);
        if round.games.is_empty() {
            break;
        }
        rounds.push(round);
    }
    Schedule::new(rounds)
}

/// Comparable quality key: more games, then fewer same-gender repeats, then
/// fewer rounds (tighter court packing).
fn score(schedule: &Schedule, roster: Roster, courts: u16) -> (usize, Reverse<usize>, Reverse<usize>) {
    let r = verify(schedule, roster, courts);
    let repeats = r.man_repeat_excess + r.woman_repeat_excess;
    (r.games, Reverse(repeats), Reverse(r.rounds))
}

/// Run `restarts` randomized greedy passes from the given seed and return the
/// best schedule found.
pub fn greedy(roster: Roster, courts: u16, restarts: u32, seed: u64) -> Schedule {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut best: Option<((usize, Reverse<usize>, Reverse<usize>), Schedule)> = None;

    for _ in 0..restarts.max(1) {
        let s = greedy_once(roster, courts, &mut rng);
        let key = score(&s, roster, courts);
        if best.as_ref().map_or(true, |(bk, _)| key > *bk) {
            best = Some((key, s));
        }
    }

    best.map(|(_, s)| s).unwrap_or_default()
}
