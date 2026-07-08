//! Mixed-doubles round-robin scheduler.
//!
//! See [`model`] for the precise problem statement and the provable bounds.
//! The pipeline is: construct a schedule (currently [`greedy`]), then
//! [`verify`] it against those bounds, then [`report`] it.

pub mod greedy;
pub mod model;
pub mod report;
pub mod search;
pub mod verify;

#[cfg(test)]
mod tests {
    use crate::greedy::greedy;
    use crate::model::{Game, Man, Roster, Round, Schedule, Team, Woman};
    use crate::verify::{verify, Violation};

    fn game(m1: u16, w1: u16, m2: u16, w2: u16) -> Game {
        Game::new(
            Team::new(Man(m1), Woman(w1)),
            Team::new(Man(m2), Woman(w2)),
        )
    }

    #[test]
    fn bounds_math() {
        let r = Roster::new(6, 6);
        assert_eq!(r.max_games(), 18);
        assert_eq!(r.distinct_man_pairs(), 15);
        // 18 games ⇒ at least 3 man–man pairs must repeat.
        assert_eq!(r.min_man_repeats(18), 3);
        assert_eq!(r.min_man_repeats(15), 0);
    }

    #[test]
    fn verifier_flags_repeated_partnership() {
        // Same game in two rounds reuses partnerships (0,0) and (1,1).
        let g = game(0, 0, 1, 1);
        let s = Schedule::new(vec![Round::new(vec![g]), Round::new(vec![g])]);
        let r = verify(&s, Roster::new(4, 4), 2);
        assert!(!r.is_legal());
        assert!(r
            .violations
            .iter()
            .any(|v| matches!(v, Violation::RepeatedPartnership { .. })));
    }

    #[test]
    fn verifier_flags_double_booking() {
        // M0 plays on two courts in the same round.
        let round = Round::new(vec![game(0, 0, 1, 1), game(0, 2, 2, 3)]);
        let s = Schedule::new(vec![round]);
        let r = verify(&s, Roster::new(3, 4), 2);
        assert!(!r.is_legal());
        assert!(r
            .violations
            .iter()
            .any(|v| matches!(v, Violation::DoubleBooked { .. })));
    }

    #[test]
    fn verifier_flags_malformed_game() {
        // Same man on both teams.
        let s = Schedule::new(vec![Round::new(vec![game(0, 0, 0, 1)])]);
        let r = verify(&s, Roster::new(2, 2), 1);
        assert!(r
            .violations
            .iter()
            .any(|v| matches!(v, Violation::Malformed { .. })));
    }

    #[test]
    fn greedy_is_always_legal() {
        for (m, w, c) in [(6, 6, 3), (8, 8, 4), (5, 7, 2), (4, 6, 3), (10, 10, 5)] {
            let roster = Roster::new(m, w);
            let s = greedy(roster, c, 100, 42);
            let r = verify(&s, roster, c);
            assert!(r.is_legal(), "illegal schedule for {}x{}: {:?}", m, w, r.violations);
            assert!(r.games <= r.max_games, "exceeded game ceiling");
        }
    }

    #[test]
    fn greedy_beats_baseline_on_6x6() {
        // The original prototype landed around 12/18. A soft-constraint greedy
        // with restarts should comfortably clear 80% of the ceiling.
        let roster = Roster::new(6, 6);
        let s = greedy(roster, 3, 500, 7);
        let r = verify(&s, roster, 3);
        assert!(r.is_legal());
        assert!(r.games >= 15, "expected ≥15 games, got {}", r.games);
    }

    #[test]
    fn optimize_is_legal_and_near_ceiling() {
        use crate::search::{optimize, EMPHASIS_BALANCED};
        for (m, w, c) in [(6, 6, 3), (8, 8, 4), (5, 7, 2), (10, 10, 5)] {
            let roster = Roster::new(m, w);
            let s = optimize(roster, c, 24_000, EMPHASIS_BALANCED, 1);
            let r = verify(&s, roster, c);
            assert!(r.is_legal(), "illegal for {}x{}: {:?}", m, w, r.violations);
            // The heuristic reaches, or comes within one of, the game ceiling.
            assert!(
                r.games >= r.max_games - 1,
                "{}x{}: only {} of {} games",
                m,
                w,
                r.games,
                r.max_games
            );
        }
    }

    #[test]
    fn variety_emphasis_drives_same_gender_low_on_6x6() {
        // Floor is 3+3=6. The raw greedy sits around 15. Variety emphasis frees
        // the search to approach the floor (at some court-utilization cost).
        use crate::search::{optimize, EMPHASIS_VARIETY};
        let roster = Roster::new(6, 6);
        let s = optimize(roster, 3, 60_000, EMPHASIS_VARIETY, 3);
        let r = verify(&s, roster, 3);
        assert!(r.is_legal());
        assert_eq!(r.games, 18);
        let excess = r.man_repeat_excess + r.woman_repeat_excess;
        assert!(excess <= 9, "same-gender excess {} (floor 6)", excess);
        // Nobody should face the same opponent 3× when 2× suffices.
        assert!(r.man_max_meetings <= 2 && r.woman_max_meetings <= 2);
    }

    #[test]
    fn courts_emphasis_fills_courts_on_6x6() {
        // Courts emphasis should pack 18 games into 6 full rounds (0 byes).
        use crate::search::{optimize, EMPHASIS_COURTS};
        let roster = Roster::new(6, 6);
        let s = optimize(roster, 3, 60_000, EMPHASIS_COURTS, 3);
        let r = verify(&s, roster, 3);
        assert!(r.is_legal());
        assert_eq!(r.games, 18);
        assert!(r.rounds <= 7, "expected near-full packing, got {} rounds", r.rounds);
    }
}
