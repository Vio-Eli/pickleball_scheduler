//! CLI entry point.
//!
//! Part 1 (maximize games): `pickleball_scheduler [men] [women] [courts] [emphasis] [ls_iters] [seed]`
//!   (defaults: 6 6 3 balanced 40000) — `emphasis` is `courts` | `balanced` | `variety`.
//!
//! Part 2 (target a fixed amount of play): add a token `each=N` or `total=G`,
//!   e.g. `pickleball_scheduler 8 8 4 each=6` or `... total=30`.

use pickleball_scheduler::model::Roster;
use pickleball_scheduler::report::{print_report, print_schedule};
use pickleball_scheduler::search::{optimize, EMPHASIS_BALANCED, EMPHASIS_COURTS, EMPHASIS_VARIETY};
use pickleball_scheduler::target::{by_games_per_player, by_total_games};
use pickleball_scheduler::verify::verify;

fn arg<T: std::str::FromStr>(args: &[String], i: usize, default: T) -> T {
    args.get(i).and_then(|s| s.parse().ok()).unwrap_or(default)
}

/// Scan for a Part 2 target token `each=N` / `total=G`.
fn part2_target(args: &[String]) -> Option<(&'static str, u32)> {
    for a in args {
        if let Some(v) = a.strip_prefix("each=") {
            if let Ok(n) = v.parse() {
                return Some(("each", n));
            }
        }
        if let Some(v) = a.strip_prefix("total=") {
            if let Ok(n) = v.parse() {
                return Some(("total", n));
            }
        }
    }
    None
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let men: u16 = arg(&args, 1, 6);
    let women: u16 = arg(&args, 2, 6);
    let courts: u16 = arg(&args, 3, 3);
    let seed: u64 = arg(&args, 6, 0xDEAD_BEEF);
    let roster = Roster::new(men, women);

    let schedule = match part2_target(&args) {
        Some(("each", n)) => {
            println!("(Part 2: each player plays ~{} games)", n);
            by_games_per_player(roster, courts, n, seed)
        }
        Some(("total", g)) => {
            println!("(Part 2: cap at {} total games)", g);
            by_total_games(roster, courts, g as usize, seed)
        }
        _ => {
            let emphasis = args.get(4).map(String::as_str).unwrap_or("balanced");
            let ls_iters: u32 = arg(&args, 5, 40_000);
            let round_weight = match emphasis {
                "courts" => EMPHASIS_COURTS,
                "variety" => EMPHASIS_VARIETY,
                _ => EMPHASIS_BALANCED,
            };
            println!("(Part 1: maximize games — emphasis: {})", emphasis);
            optimize(roster, courts, ls_iters, round_weight, seed)
        }
    };

    print_schedule(&schedule, roster, courts);
    let report = verify(&schedule, roster, courts);
    print_report(&report);
}
