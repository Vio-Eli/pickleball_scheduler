//! The verifier: the single source of truth for whether a schedule is legal
//! and how good it is.
//!
//! Every scheduling algorithm in this crate is judged by [`verify`]. It checks
//! the two hard ledgers (partnerships, mixed oppositions) plus structural
//! sanity, and reports the soft same-gender repeat counts against their
//! information-theoretic floor so we always know how close we are to optimal.

use crate::model::{Game, Player, Roster, Schedule};
use std::collections::HashMap;

/// A structural or hard-constraint violation. A schedule with any of these is
/// illegal; a schedule with none is *legal* (its quality is then measured by
/// the soft metrics in [`Report`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Violation {
    /// A game has two identical men or two identical women.
    Malformed { round: usize, game: usize },
    /// A player index is outside the roster.
    OutOfRange { round: usize, game: usize, who: Player },
    /// A player appears in two games in the same round.
    DoubleBooked { round: usize, who: Player },
    /// A `(man, woman)` partnership is used more than once.
    RepeatedPartnership { man: u16, woman: u16, count: usize },
    /// A `(man, woman)` mixed opposition occurs more than once.
    RepeatedMixedOpp { man: u16, woman: u16, count: usize },
}

/// A full quality report for a schedule.
#[derive(Clone, Debug)]
pub struct Report {
    pub roster: Roster,
    pub courts: u16,

    /// Hard/structural violations. Empty ⇒ the schedule is legal.
    pub violations: Vec<Violation>,

    pub games: usize,
    /// The partnership-ledger ceiling, `⌊men·women/2⌋`.
    pub max_games: usize,
    pub rounds: usize,

    /// Extra man–man encounters beyond the first for each pair, summed.
    pub man_repeat_excess: usize,
    /// The unavoidable floor for `man_repeat_excess` at this game count.
    pub man_repeat_floor: usize,
    /// The most times any single man–man pair meets.
    pub man_max_meetings: usize,

    /// Extra woman–woman encounters beyond the first, summed.
    pub woman_repeat_excess: usize,
    pub woman_repeat_floor: usize,
    pub woman_max_meetings: usize,

    /// Games each player appears in, indexed as men first then women.
    pub games_per_man: Vec<usize>,
    pub games_per_woman: Vec<usize>,

    /// Byes per round (players idle because courts/opponents ran out).
    pub byes_per_round: Vec<usize>,
    /// Court-slots used ÷ court-slots offered = `games / (rounds · courts)`.
    pub court_utilization: f64,
}

impl Report {
    /// Legal ⇔ no hard or structural violations. Soft same-gender repeats do
    /// not affect legality.
    pub fn is_legal(&self) -> bool {
        self.violations.is_empty()
    }

    /// At the partnership ceiling — the most games this roster can support.
    pub fn hits_game_ceiling(&self) -> bool {
        self.games == self.max_games
    }

    /// Same-gender repeats are at their unavoidable floor: no schedule with
    /// this many games can do better on the soft objective.
    pub fn hits_repeat_floor(&self) -> bool {
        self.man_repeat_excess == self.man_repeat_floor
            && self.woman_repeat_excess == self.woman_repeat_floor
    }

    /// Spread of games across players (max − min). Zero ⇒ perfectly balanced
    /// participation, which matters for the Part 2 per-player target.
    pub fn participation_spread(&self) -> usize {
        let all = self.games_per_man.iter().chain(self.games_per_woman.iter());
        let max = all.clone().copied().max().unwrap_or(0);
        let min = all.copied().min().unwrap_or(0);
        max - min
    }
}

/// Tally repeats in a histogram: returns `(excess, max_count)` where `excess`
/// is `Σ max(0, count − 1)` and `max_count` is the largest bucket.
fn repeat_stats<K>(counts: &HashMap<K, usize>) -> (usize, usize) {
    let excess = counts.values().map(|&c| c.saturating_sub(1)).sum();
    let max = counts.values().copied().max().unwrap_or(0);
    (excess, max)
}

/// Verify and score a schedule against a roster and court count.
pub fn verify(schedule: &Schedule, roster: Roster, courts: u16) -> Report {
    let mut violations = Vec::new();

    // Hard ledgers.
    let mut partner_counts: HashMap<(u16, u16), usize> = HashMap::new();
    let mut mixed_counts: HashMap<(u16, u16), usize> = HashMap::new();
    let mut man_counts: HashMap<(u16, u16), usize> = HashMap::new();
    let mut woman_counts: HashMap<(u16, u16), usize> = HashMap::new();

    // Participation.
    let mut games_per_man = vec![0usize; roster.men as usize];
    let mut games_per_woman = vec![0usize; roster.women as usize];
    let mut byes_per_round = Vec::with_capacity(schedule.rounds.len());

    let in_range = |g: &Game| {
        g.a.man.0 < roster.men
            && g.b.man.0 < roster.men
            && g.a.woman.0 < roster.women
            && g.b.woman.0 < roster.women
    };

    for (ri, round) in schedule.rounds.iter().enumerate() {
        let mut seen: HashMap<Player, ()> = HashMap::new();
        let mut active = 0usize;

        for (gi, game) in round.games.iter().enumerate() {
            if !game.is_well_formed() {
                violations.push(Violation::Malformed { round: ri, game: gi });
            }
            if !in_range(game) {
                // Report each offending player; skip ledger updates for safety.
                for who in game.players() {
                    let bad = match who {
                        Player::M(m) => m.0 >= roster.men,
                        Player::W(w) => w.0 >= roster.women,
                    };
                    if bad {
                        violations.push(Violation::OutOfRange { round: ri, game: gi, who });
                    }
                }
                continue;
            }

            // Occupancy within the round.
            for who in game.players() {
                if seen.insert(who, ()).is_some() {
                    violations.push(Violation::DoubleBooked { round: ri, who });
                }
                active += 1;
            }

            // Ledgers.
            for (m, w) in game.partnerships() {
                *partner_counts.entry((m.0, w.0)).or_insert(0) += 1;
            }
            for (m, w) in game.mixed_opps() {
                *mixed_counts.entry((m.0, w.0)).or_insert(0) += 1;
            }
            let mp = game.man_pair();
            *man_counts.entry((mp.0 .0, mp.1 .0)).or_insert(0) += 1;
            let wp = game.woman_pair();
            *woman_counts.entry((wp.0 .0, wp.1 .0)).or_insert(0) += 1;

            games_per_man[game.a.man.0 as usize] += 1;
            games_per_man[game.b.man.0 as usize] += 1;
            games_per_woman[game.a.woman.0 as usize] += 1;
            games_per_woman[game.b.woman.0 as usize] += 1;
        }

        byes_per_round.push(roster.total_players().saturating_sub(active));
    }

    // Hard-ledger violations.
    for (&(m, w), &c) in &partner_counts {
        if c > 1 {
            violations.push(Violation::RepeatedPartnership { man: m, woman: w, count: c });
        }
    }
    for (&(m, w), &c) in &mixed_counts {
        if c > 1 {
            violations.push(Violation::RepeatedMixedOpp { man: m, woman: w, count: c });
        }
    }

    let (man_repeat_excess, man_max_meetings) = repeat_stats(&man_counts);
    let (woman_repeat_excess, woman_max_meetings) = repeat_stats(&woman_counts);

    let games = schedule.num_games();
    let rounds = schedule.num_rounds();
    let court_slots = rounds * courts as usize;
    let court_utilization = if court_slots == 0 {
        0.0
    } else {
        games as f64 / court_slots as f64
    };

    Report {
        roster,
        courts,
        violations,
        games,
        max_games: roster.max_games(),
        rounds,
        man_repeat_excess,
        man_repeat_floor: roster.min_man_repeats(games),
        man_max_meetings,
        woman_repeat_excess,
        woman_repeat_floor: roster.min_woman_repeats(games),
        woman_max_meetings,
        games_per_man,
        games_per_woman,
        byes_per_round,
        court_utilization,
    }
}
