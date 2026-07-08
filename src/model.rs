//! Core domain types for the mixed-doubles scheduler.
//!
//! The problem, stated precisely:
//!
//! A *game* is a mixed-doubles match between two teams. Each team is a
//! `(man, woman)` pair, so a game involves exactly two distinct men and two
//! distinct women. Every game touches four *ledgers* at once:
//!
//!   * **partnerships** — the two `(man, woman)` pairs that team up. Each
//!     partnership may be used **at most once** (hard).
//!   * **mixed oppositions** — the two `(man, woman)` pairs that face each
//!     other across the net. Each may occur **at most once** (hard). This is
//!     genuinely separate from partnering: `M1` may partner `W1` in one game
//!     and oppose `W1` in another.
//!   * **man–man oppositions** — the one `{man, man}` pair that faces off.
//!     Repeats are *minimized*, not forbidden (soft).
//!   * **woman–woman oppositions** — likewise the one `{woman, woman}` pair.
//!
//! For a balanced roster of `n` men and `n` women, the partnership ledger caps
//! the schedule at `n²/2` games, and at that ceiling at least `n/2` man–man
//! pairs (and `n/2` woman–woman pairs) *must* repeat by pigeonhole. See
//! [`Roster`] for these bounds.

use std::fmt;

/// A man, identified by a 0-based index within the men's roster.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct Man(pub u16);

/// A woman, identified by a 0-based index within the women's roster.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct Woman(pub u16);

impl fmt::Display for Man {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "M{}", self.0 + 1)
    }
}

impl fmt::Display for Woman {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "W{}", self.0 + 1)
    }
}

/// A mixed-doubles team: one man partnered with one woman.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Team {
    pub man: Man,
    pub woman: Woman,
}

impl Team {
    pub fn new(man: Man, woman: Woman) -> Self {
        Team { man, woman }
    }
}

impl fmt::Display for Team {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} & {}", self.man, self.woman)
    }
}

/// A single game: team `a` versus team `b`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Game {
    pub a: Team,
    pub b: Team,
}

impl Game {
    pub fn new(a: Team, b: Team) -> Self {
        Game { a, b }
    }

    /// The two men in this game (in team order).
    pub fn men(&self) -> (Man, Man) {
        (self.a.man, self.b.man)
    }

    /// The two women in this game (in team order).
    pub fn women(&self) -> (Woman, Woman) {
        (self.a.woman, self.b.woman)
    }

    /// The two `(man, woman)` partnerships consumed by this game.
    pub fn partnerships(&self) -> [(Man, Woman); 2] {
        [(self.a.man, self.a.woman), (self.b.man, self.b.woman)]
    }

    /// The two `(man, woman)` mixed oppositions: each man versus the *other*
    /// team's woman.
    pub fn mixed_opps(&self) -> [(Man, Woman); 2] {
        [(self.a.man, self.b.woman), (self.b.man, self.a.woman)]
    }

    /// The unordered man–man opposition `{man, man}`, canonicalized so the
    /// smaller index comes first (for use as a map key).
    pub fn man_pair(&self) -> (Man, Man) {
        let (x, y) = self.men();
        if x <= y { (x, y) } else { (y, x) }
    }

    /// The unordered woman–woman opposition `{woman, woman}`, canonicalized.
    pub fn woman_pair(&self) -> (Woman, Woman) {
        let (x, y) = self.women();
        if x <= y { (x, y) } else { (y, x) }
    }

    /// Every player in this game, for occupancy/disjointness checks.
    pub fn players(&self) -> [Player; 4] {
        [
            Player::M(self.a.man),
            Player::M(self.b.man),
            Player::W(self.a.woman),
            Player::W(self.b.woman),
        ]
    }

    /// Structurally well-formed: two distinct men and two distinct women.
    pub fn is_well_formed(&self) -> bool {
        self.a.man != self.b.man && self.a.woman != self.b.woman
    }
}

impl fmt::Display for Game {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} vs {}", self.a, self.b)
    }
}

/// A player of either gender — used for occupancy and bye bookkeeping.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub enum Player {
    M(Man),
    W(Woman),
}

impl fmt::Display for Player {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Player::M(m) => write!(f, "{}", m),
            Player::W(w) => write!(f, "{}", w),
        }
    }
}

/// A round: a set of games played simultaneously, one per court. No player may
/// appear twice in a round (nobody is on two courts at once).
#[derive(Clone, Debug, Default)]
pub struct Round {
    pub games: Vec<Game>,
}

impl Round {
    pub fn new(games: Vec<Game>) -> Self {
        Round { games }
    }

    /// Players active this round (those *not* on a bye).
    pub fn active_players(&self) -> Vec<Player> {
        self.games.iter().flat_map(|g| g.players()).collect()
    }
}

/// A complete schedule: an ordered list of rounds.
#[derive(Clone, Debug, Default)]
pub struct Schedule {
    pub rounds: Vec<Round>,
}

impl Schedule {
    pub fn new(rounds: Vec<Round>) -> Self {
        Schedule { rounds }
    }

    /// Every game across every round.
    pub fn all_games(&self) -> impl Iterator<Item = &Game> {
        self.rounds.iter().flat_map(|r| r.games.iter())
    }

    pub fn num_games(&self) -> usize {
        self.rounds.iter().map(|r| r.games.len()).sum()
    }

    pub fn num_rounds(&self) -> usize {
        self.rounds.len()
    }
}

/// The roster: how many men and how many women are available.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Roster {
    pub men: u16,
    pub women: u16,
}

impl Roster {
    pub fn new(men: u16, women: u16) -> Self {
        Roster { men, women }
    }

    pub fn total_players(&self) -> usize {
        self.men as usize + self.women as usize
    }

    /// Iterator over all men.
    pub fn men_iter(&self) -> impl Iterator<Item = Man> {
        (0..self.men).map(Man)
    }

    /// Iterator over all women.
    pub fn women_iter(&self) -> impl Iterator<Item = Woman> {
        (0..self.women).map(Woman)
    }

    /// The maximum number of games any schedule can contain, capped by the
    /// partnership ledger: `⌊(men · women) / 2⌋`. Each game consumes two of the
    /// `men · women` distinct partnerships, and none may repeat.
    pub fn max_games(&self) -> usize {
        (self.men as usize * self.women as usize) / 2
    }

    /// Number of distinct man–man opposition pairs, `C(men, 2)`.
    pub fn distinct_man_pairs(&self) -> usize {
        let m = self.men as usize;
        m * m.saturating_sub(1) / 2
    }

    /// Number of distinct woman–woman opposition pairs, `C(women, 2)`.
    pub fn distinct_woman_pairs(&self) -> usize {
        let w = self.women as usize;
        w * w.saturating_sub(1) / 2
    }

    /// The unavoidable number of man–man opposition *repeats* (extra encounters
    /// beyond the first) when a schedule plays `games` games. There is one
    /// man–man opposition per game, so any excess over the distinct-pair count
    /// must repeat: `max(0, games − C(men, 2))`.
    pub fn min_man_repeats(&self, games: usize) -> usize {
        games.saturating_sub(self.distinct_man_pairs())
    }

    /// Likewise for woman–woman oppositions: `max(0, games − C(women, 2))`.
    pub fn min_woman_repeats(&self, games: usize) -> usize {
        games.saturating_sub(self.distinct_woman_pairs())
    }

    /// Number of distinct `(man, woman)` pairs — the size of both the
    /// partnership and the mixed-opposition ledger: `men · women`.
    pub fn distinct_pairs(&self) -> usize {
        self.men as usize * self.women as usize
    }

    /// The unavoidable number of *partnership* repeats when a schedule plays
    /// `games` games. Each game uses two partnerships, and there are only
    /// `men · women` distinct ones: `max(0, 2·games − men·women)`. Zero at or
    /// below the game ceiling; positive only when Part 2 pushes past it.
    pub fn min_partner_repeats(&self, games: usize) -> usize {
        (2 * games).saturating_sub(self.distinct_pairs())
    }

    /// Likewise for mixed oppositions (same ledger size): `max(0, 2·games −
    /// men·women)`.
    pub fn min_mixed_repeats(&self, games: usize) -> usize {
        (2 * games).saturating_sub(self.distinct_pairs())
    }
}
