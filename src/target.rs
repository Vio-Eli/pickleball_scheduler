//! Part 2: schedule toward a *target amount of play*, relaxing the once-rules.
//!
//! Part 1 maximizes games with the once-rules hard. Part 2 fixes the amount of
//! play instead and makes those rules **soft** — minimized toward their floor
//! rather than forbidden. Two modes, both reducing to one primitive (build a
//! target number of games):
//!
//! * [`by_games_per_player`] — everyone plays (about) `N` games.
//! * [`by_total_games`] — cap the schedule at exactly `G` games.
//!
//! Below the game ceiling there is slack, so partnerships and mixed
//! oppositions still never repeat (their floor is 0); above it, repeats are
//! forced and this spreads them as thinly as possible. Participation is kept
//! fair (byes rotate to whoever has played least), and when the target is
//! exactly the full round-robin the balanced case delegates to Part 1 so it
//! still gets the optimal algebraic construction.

use crate::model::{Game, Man, Roster, Round, Schedule, Team, Woman};
use crate::search::{optimize, EMPHASIS_BALANCED};
use crate::verify::verify;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::collections::HashMap;

// Cost weights, separated in magnitude so the objective is effectively
// lexicographic: avoid partnership/mixed repeats first, then keep play fair,
// then minimize same-gender oppositions.
const W_HARD: f64 = 1_000_000.0;
const W_FAIR: f64 = 1_000.0;
const W_SAME: f64 = 1.0;

/// Running counts used to score candidate games.
struct Ledgers {
    partner: HashMap<(u16, u16), u32>,
    mixed: HashMap<(u16, u16), u32>,
    man: HashMap<(u16, u16), u32>,
    woman: HashMap<(u16, u16), u32>,
    played_m: Vec<u32>,
    played_w: Vec<u32>,
}

impl Ledgers {
    fn new(roster: Roster) -> Self {
        Ledgers {
            partner: HashMap::new(),
            mixed: HashMap::new(),
            man: HashMap::new(),
            woman: HashMap::new(),
            played_m: vec![0; roster.men as usize],
            played_w: vec![0; roster.women as usize],
        }
    }

    fn man_key(a: Man, b: Man) -> (u16, u16) {
        if a.0 <= b.0 { (a.0, b.0) } else { (b.0, a.0) }
    }
    fn woman_key(x: Woman, y: Woman) -> (u16, u16) {
        if x.0 <= y.0 { (x.0, y.0) } else { (y.0, x.0) }
    }

    fn count(m: &HashMap<(u16, u16), u32>, k: (u16, u16)) -> f64 {
        m.get(&k).copied().unwrap_or(0) as f64
    }

    /// Cost of a specific game (orientation fixed): partnerships `(a,x),(b,y)`,
    /// mixed opps `(a,y),(b,x)`.
    fn cost(&self, a: Man, x: Woman, b: Man, y: Woman) -> f64 {
        let repeats = Self::count(&self.partner, (a.0, x.0))
            + Self::count(&self.partner, (b.0, y.0))
            + Self::count(&self.mixed, (a.0, y.0))
            + Self::count(&self.mixed, (b.0, x.0));
        let same = Self::count(&self.man, Self::man_key(a, b))
            + Self::count(&self.woman, Self::woman_key(x, y));
        let fair = (self.played_m[a.0 as usize]
            + self.played_m[b.0 as usize]
            + self.played_w[x.0 as usize]
            + self.played_w[y.0 as usize]) as f64;
        W_HARD * repeats + W_FAIR * fair + W_SAME * same
    }

    /// The cheaper of the two orientations for players `{a,b} × {x,y}`.
    fn best_game(&self, a: Man, b: Man, x: Woman, y: Woman) -> (f64, Game) {
        let c1 = self.cost(a, x, b, y);
        let c2 = self.cost(a, y, b, x);
        if c1 <= c2 {
            (c1, Game::new(Team::new(a, x), Team::new(b, y)))
        } else {
            (c2, Game::new(Team::new(a, y), Team::new(b, x)))
        }
    }

    fn commit(&mut self, g: &Game) {
        for (m, w) in g.partnerships() {
            *self.partner.entry((m.0, w.0)).or_insert(0) += 1;
        }
        for (m, w) in g.mixed_opps() {
            *self.mixed.entry((m.0, w.0)).or_insert(0) += 1;
        }
        *self.man.entry(Self::man_key(g.a.man, g.b.man)).or_insert(0) += 1;
        *self.woman.entry(Self::woman_key(g.a.woman, g.b.woman)).or_insert(0) += 1;
        self.played_m[g.a.man.0 as usize] += 1;
        self.played_m[g.b.man.0 as usize] += 1;
        self.played_w[g.a.woman.0 as usize] += 1;
        self.played_w[g.b.woman.0 as usize] += 1;
    }
}

/// Greedily build rounds until `total_games` games are placed. Each round holds
/// up to `courts` player-disjoint games; each game is the minimum-cost choice
/// (ties broken at random), so repeats stay at their floor and byes fall on
/// whoever has played least.
fn build(roster: Roster, courts: u16, total_games: usize, rng: &mut StdRng) -> Vec<Vec<Game>> {
    let mut led = Ledgers::new(roster);
    let mut rounds: Vec<Vec<Game>> = Vec::new();
    let mut placed = 0usize;

    while placed < total_games {
        let mut used_m = vec![false; roster.men as usize];
        let mut used_w = vec![false; roster.women as usize];
        let mut round: Vec<Game> = Vec::new();

        while round.len() < courts as usize && placed < total_games {
            let men: Vec<Man> = (0..roster.men).filter(|m| !used_m[*m as usize]).map(Man).collect();
            let women: Vec<Woman> =
                (0..roster.women).filter(|w| !used_w[*w as usize]).map(Woman).collect();
            if men.len() < 2 || women.len() < 2 {
                break;
            }

            let mut best_cost = f64::INFINITY;
            let mut best: Option<Game> = None;
            let mut ties = 0u32;
            for i in 0..men.len() {
                for j in (i + 1)..men.len() {
                    for k in 0..women.len() {
                        for l in (k + 1)..women.len() {
                            let (c, g) = led.best_game(men[i], men[j], women[k], women[l]);
                            if c < best_cost - 1e-9 {
                                best_cost = c;
                                best = Some(g);
                                ties = 1;
                            } else if (c - best_cost).abs() <= 1e-9 {
                                ties += 1;
                                if rng.random_range(0..ties) == 0 {
                                    best = Some(g);
                                }
                            }
                        }
                    }
                }
            }

            match best {
                Some(g) => {
                    led.commit(&g);
                    used_m[g.a.man.0 as usize] = true;
                    used_m[g.b.man.0 as usize] = true;
                    used_w[g.a.woman.0 as usize] = true;
                    used_w[g.b.woman.0 as usize] = true;
                    round.push(g);
                    placed += 1;
                }
                None => break,
            }
        }

        if round.is_empty() {
            break; // roster too small to place any game
        }
        rounds.push(round);
    }

    rounds
}

/// Comparable quality key for a Part 2 schedule: fewest partnership+mixed
/// repeats, then fewest same-gender repeats, then most balanced participation.
fn score(sched: &Schedule, roster: Roster, courts: u16) -> (usize, usize, usize) {
    let r = verify(sched, roster, courts);
    (
        r.partner_repeat_excess + r.mixed_repeat_excess,
        r.man_repeat_excess + r.woman_repeat_excess,
        r.participation_spread(),
    )
}

/// Build a `total_games`-game schedule, best of several randomized restarts.
fn build_best(roster: Roster, courts: u16, total_games: usize, seed: u64) -> Schedule {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut best: Option<((usize, usize, usize), Schedule)> = None;
    for _ in 0..60 {
        let rounds = build(roster, courts, total_games, &mut rng);
        let sched = Schedule::new(rounds.into_iter().map(Round::new).collect());
        let key = score(&sched, roster, courts);
        if best.as_ref().map_or(true, |(bk, _)| key < *bk) {
            best = Some((key, sched));
        }
    }
    best.map(|(_, s)| s).unwrap_or_default()
}

/// The number of games needed for every player to appear `n_each` times, given
/// four players per game: `round(n_each · players / 4)`.
fn games_for_per_player(roster: Roster, n_each: u32) -> usize {
    let players = roster.total_players();
    (n_each as usize * players + 2) / 4
}

/// Part 2 mode: each player plays about `n_each` games.
///
/// For the balanced full round-robin (`n_each == n`, courts ≥ n/2) this is
/// exactly Part 1, so it delegates to [`optimize`] to inherit the optimal
/// construction. Otherwise it targets `round(n_each · players / 4)` games with
/// fair byes.
pub fn by_games_per_player(roster: Roster, courts: u16, n_each: u32, seed: u64) -> Schedule {
    let n = roster.men as usize;
    let balanced_even = roster.women as usize == n && n >= 2 && n % 2 == 0;
    if balanced_even && n_each as usize == n && courts as usize >= n / 2 {
        return optimize(roster, courts, 20_000, EMPHASIS_BALANCED, seed);
    }
    let total = games_for_per_player(roster, n_each);
    build_best(roster, courts, total, seed)
}

/// Part 2 mode: cap the schedule at exactly `total_games` games (the final
/// round may be partial to hit the count exactly).
pub fn by_total_games(roster: Roster, courts: u16, total_games: usize, seed: u64) -> Schedule {
    build_best(roster, courts, total_games, seed)
}
