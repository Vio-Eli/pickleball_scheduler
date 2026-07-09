//! Mixed-doubles round-robin scheduler.
//!
//! See [`model`] for the precise problem statement and the provable bounds.
//! The pipeline is: construct a schedule (currently [`greedy`]), then
//! [`verify`] it against those bounds, then [`report`] it.

pub mod construct;
pub mod greedy;
pub mod model;
pub mod report;
pub mod search;
pub mod tables;
pub mod target;
pub mod verify;
pub mod wasm;

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
    fn reflection_is_legal_and_full_for_all_even_n() {
        use crate::construct::reflection;
        for n in [4u16, 6, 8, 10, 12, 14, 16] {
            let roster = Roster::new(n, n);
            let s = reflection(roster).expect("reflection applies to even n");
            let r = verify(&s, roster, n / 2);
            assert!(r.is_legal(), "reflection illegal for n={}: {:?}", n, r.violations);
            assert_eq!(r.games, r.max_games, "reflection missed ceiling for n={}", n);
            assert_eq!(r.rounds, n as usize, "reflection not fully packed for n={}", n);
        }
        // Not a balanced-even case.
        assert!(reflection(Roster::new(5, 5)).is_none());
        assert!(reflection(Roster::new(6, 8)).is_none());
    }

    #[test]
    fn hsolssom_hits_full_target_on_10x10() {
        use crate::construct::hsolssom;
        let roster = Roster::new(10, 10);
        let s = hsolssom(roster).expect("HSOLSSOM exists for n=10");
        let r = verify(&s, roster, 5);
        assert!(r.is_legal(), "HSOLSSOM illegal: {:?}", r.violations);
        assert_eq!(r.games, 50);
        assert_eq!(r.rounds, 10);
        assert!((r.court_utilization - 1.0).abs() < 1e-9, "not full courts");
        assert_eq!(r.man_repeat_excess, r.man_repeat_floor, "man not at floor");
        assert_eq!(r.woman_repeat_excess, r.woman_repeat_floor, "woman not at floor");
        // n < 10 is not this constructor's domain.
        assert!(hsolssom(Roster::new(8, 8)).is_none());
    }

    #[test]
    fn cached_tables_all_hit_full_target() {
        // Every embedded table must survive our own verifier: all four optima.
        use crate::construct::hsolssom;
        let mut checked = 0;
        for n in (10u16..=64).step_by(2) {
            if crate::tables::cached(n as usize).is_none() {
                continue;
            }
            let roster = Roster::new(n, n);
            let s = hsolssom(roster).expect("cached table builds");
            let r = verify(&s, roster, n / 2);
            assert!(r.is_legal(), "cached n={} illegal: {:?}", n, r.violations);
            assert_eq!(r.games, r.max_games, "cached n={} games", n);
            assert_eq!(r.rounds, n as usize, "cached n={} rounds", n);
            assert!((r.court_utilization - 1.0).abs() < 1e-9, "cached n={} util", n);
            assert_eq!(r.man_repeat_excess, r.man_repeat_floor, "cached n={} man floor", n);
            assert_eq!(r.woman_repeat_excess, r.woman_repeat_floor, "cached n={} woman floor", n);
            checked += 1;
        }
        assert!(checked >= 3, "expected at least the n=10/14/18 tables, checked {}", checked);
    }

    #[test]
    fn optimize_returns_optimal_construction_on_10x10() {
        use crate::search::{optimize, EMPHASIS_BALANCED};
        let roster = Roster::new(10, 10);
        let s = optimize(roster, 5, 5_000, EMPHASIS_BALANCED, 1);
        let r = verify(&s, roster, 5);
        // The optimizer should short-circuit to the optimal construction.
        assert!(r.is_legal());
        assert_eq!(r.games, 50);
        assert_eq!(r.rounds, 10);
        assert_eq!(r.man_repeat_excess, r.man_repeat_floor);
        assert_eq!(r.woman_repeat_excess, r.woman_repeat_floor);
    }

    #[test]
    fn wasm_json_has_expected_shape() {
        use crate::wasm::generate_json;
        // Part 1 balanced, 6×6/3 → 18 games, legal, well-formed JSON.
        let j = generate_json(6, 6, 3, 1, 0, 7);
        assert!(j.contains("\"games\":18"), "{}", j);
        assert!(j.contains("\"maxGames\":18"));
        assert!(j.contains("\"rounds\":[["));
        assert!(j.contains("\"legal\":true"));
        assert!(j.contains("\"gamesPerMan\":["));
        // Part 2 each=8 (above ceiling) → forced partner repeats at floor 12.
        let j2 = generate_json(6, 6, 3, 3, 8, 1);
        assert!(j2.contains("\"partnerExcess\":12"), "{}", j2);
        // Oversized input is rejected, not run.
        assert!(generate_json(40, 40, 5, 1, 0, 1).contains("\"error\""));
    }

    #[test]
    fn part2_below_ceiling_is_legal_and_fair() {
        use crate::target::by_games_per_player;
        // 6×6, each plays 4 → 12 games, below the 18 ceiling: no forced repeats.
        let roster = Roster::new(6, 6);
        let s = by_games_per_player(roster, 3, 4, 1);
        let r = verify(&s, roster, 3);
        assert!(r.is_structurally_valid());
        assert!(r.is_legal(), "below ceiling should have no partner/opp repeats");
        assert_eq!(r.games, 12);
        assert_eq!(r.rounds, 4);
        assert_eq!(r.participation_spread(), 0, "balanced full courts ⇒ everyone plays 4");
    }

    #[test]
    fn part2_above_ceiling_hits_all_floors() {
        use crate::target::by_games_per_player;
        // 6×6, each plays 8 → 24 games, past the 18 ceiling: repeats forced but
        // every ledger should land exactly on its floor.
        let roster = Roster::new(6, 6);
        let s = by_games_per_player(roster, 3, 8, 1);
        let r = verify(&s, roster, 3);
        assert!(r.is_structurally_valid());
        assert_eq!(r.games, 24);
        assert_eq!(r.participation_spread(), 0);
        assert_eq!(r.partner_repeat_excess, r.partner_repeat_floor, "partner floor");
        assert_eq!(r.mixed_repeat_excess, r.mixed_repeat_floor, "mixed floor");
        assert_eq!(r.man_repeat_excess, r.man_repeat_floor, "man floor");
        assert_eq!(r.woman_repeat_excess, r.woman_repeat_floor, "woman floor");
    }

    #[test]
    fn part2_total_cap_is_exact() {
        use crate::target::by_total_games;
        let roster = Roster::new(6, 6);
        for g in [7usize, 9, 10, 15] {
            let s = by_total_games(roster, 3, g, 2);
            let r = verify(&s, roster, 3);
            assert!(r.is_structurally_valid());
            assert_eq!(r.games, g, "total cap not exact for g={}", g);
        }
    }

    #[test]
    fn part2_full_target_delegates_to_optimum() {
        use crate::target::by_games_per_player;
        // each = n at full courts is the full round-robin → Part 1 optimum.
        let roster = Roster::new(10, 10);
        let s = by_games_per_player(roster, 5, 10, 1);
        let r = verify(&s, roster, 5);
        assert!(r.is_legal());
        assert_eq!(r.games, 50);
        assert_eq!(r.man_repeat_excess, r.man_repeat_floor);
        assert_eq!(r.woman_repeat_excess, r.woman_repeat_floor);
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
