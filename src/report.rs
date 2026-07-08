//! Human-readable output: the round-by-round court grid and the quality report.

use crate::model::{Player, Roster, Schedule};
use crate::verify::Report;
use std::cmp::max;
use std::collections::HashSet;

/// Print the schedule as a `Round × Court` grid with a byes column.
pub fn print_schedule(schedule: &Schedule, roster: Roster, courts: u16) {
    let courts = courts as usize;

    // All players, for computing byes each round.
    let everyone: Vec<Player> = roster
        .men_iter()
        .map(Player::M)
        .chain(roster.women_iter().map(Player::W))
        .collect();

    // Pre-render byes per round and measure column widths.
    let mut court_widths = vec![12usize; courts];
    let mut bye_strings = Vec::with_capacity(schedule.rounds.len());

    for round in &schedule.rounds {
        let active: HashSet<Player> = round.active_players().into_iter().collect();
        let byes: Vec<String> = everyone
            .iter()
            .filter(|p| !active.contains(p))
            .map(|p| p.to_string())
            .collect();
        for (j, game) in round.games.iter().enumerate() {
            if j < court_widths.len() {
                court_widths[j] = max(court_widths[j], game.to_string().len());
            }
        }
        bye_strings.push(if byes.is_empty() { "-".to_string() } else { byes.join(", ") });
    }

    let bye_width = bye_strings.iter().map(|s| s.len()).max().unwrap_or(4).max(4);

    // Header.
    print!("{:<6} ", "Round");
    for i in 1..=courts {
        print!("{:<width$} ", format!("Court {}", i), width = court_widths[i - 1]);
    }
    println!("{:<width$}", "Byes", width = bye_width);

    // Separator.
    print!("{:-<6}-", "");
    for &w in &court_widths {
        print!("{:-<width$}-", "", width = w);
    }
    println!("{:-<width$}", "", width = bye_width);

    // Body.
    for (i, round) in schedule.rounds.iter().enumerate() {
        print!("{:<6} ", i + 1);
        for j in 0..courts {
            match round.games.get(j) {
                Some(g) => print!("{:<width$} ", g.to_string(), width = court_widths[j]),
                None => print!("{:<width$} ", "", width = court_widths[j]),
            }
        }
        println!("{:<width$}", bye_strings[i], width = bye_width);
    }
}

/// Print the verifier's quality report: legality, distance from the game
/// ceiling, and distance from the same-gender repeat floor.
pub fn print_report(report: &Report) {
    println!();
    println!(
        "Roster: {} men × {} women   |   {} courts",
        report.roster.men, report.roster.women, report.courts
    );
    println!("{}", "=".repeat(56));

    // Structural soundness is always required; the once-rules are hard for
    // Part 1 (shown as LEGAL) but soft for Part 2 (shown via the repeat lines).
    if !report.is_structurally_valid() {
        println!("Structure:        INVALID");
        for v in &report.violations {
            println!("  ! {:?}", v);
        }
    } else if report.is_legal() {
        println!("Legality:         LEGAL (no partnership or opponent repeats)");
    } else {
        println!("Structure:        valid (once-rules relaxed — see repeats below)");
    }

    println!(
        "Games:            {} / {} max{}",
        report.games,
        report.max_games,
        if report.hits_game_ceiling() { "   ← ceiling ✓" } else { "" }
    );
    // Partnership / mixed-opposition repeats — 0 for Part 1, meaningful when a
    // Part 2 target pushes past the ceiling.
    if report.partner_repeat_excess > 0 || report.partner_repeat_floor > 0 {
        println!(
            "Partner repeats:  {} extra (floor {}){}",
            report.partner_repeat_excess,
            report.partner_repeat_floor,
            if report.partner_repeat_excess == report.partner_repeat_floor { " ✓" } else { "" }
        );
        println!(
            "Opponent repeats: {} extra (floor {}){}   (mixed man vs woman)",
            report.mixed_repeat_excess,
            report.mixed_repeat_floor,
            if report.mixed_repeat_excess == report.mixed_repeat_floor { " ✓" } else { "" }
        );
    }
    println!(
        "Rounds:           {}   (court utilization {:.0}%)",
        report.rounds,
        report.court_utilization * 100.0
    );
    println!(
        "Man–man repeats:  {} extra (floor {}){}   worst pair meets {}×",
        report.man_repeat_excess,
        report.man_repeat_floor,
        if report.man_repeat_excess == report.man_repeat_floor { " ✓" } else { "" },
        report.man_max_meetings
    );
    println!(
        "Woman–woman rpts: {} extra (floor {}){}   worst pair meets {}×",
        report.woman_repeat_excess,
        report.woman_repeat_floor,
        if report.woman_repeat_excess == report.woman_repeat_floor { " ✓" } else { "" },
        report.woman_max_meetings
    );

    let (min_p, max_p) = participation_range(report);
    println!(
        "Games per player: {}–{}   (spread {})",
        min_p,
        max_p,
        report.participation_spread()
    );
    println!("{}", "=".repeat(56));
}

fn participation_range(report: &Report) -> (usize, usize) {
    let all = report
        .games_per_man
        .iter()
        .chain(report.games_per_woman.iter());
    let min = all.clone().copied().min().unwrap_or(0);
    let max = all.copied().max().unwrap_or(0);
    (min, max)
}
