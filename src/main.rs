//! CLI entry point.
//!
//! Usage: `pickleball_scheduler [men] [women] [courts] [restarts] [seed]`
//! (defaults: 6 6 3 500 3735928559)

use pickleball_scheduler::greedy::greedy;
use pickleball_scheduler::model::Roster;
use pickleball_scheduler::report::{print_report, print_schedule};
use pickleball_scheduler::verify::verify;

fn arg<T: std::str::FromStr>(args: &[String], i: usize, default: T) -> T {
    args.get(i).and_then(|s| s.parse().ok()).unwrap_or(default)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let men: u16 = arg(&args, 1, 6);
    let women: u16 = arg(&args, 2, 6);
    let courts: u16 = arg(&args, 3, 3);
    let restarts: u32 = arg(&args, 4, 500);
    let seed: u64 = arg(&args, 5, 0xDEAD_BEEF);

    let roster = Roster::new(men, women);
    let schedule = greedy(roster, courts, restarts, seed);

    print_schedule(&schedule, roster, courts);
    let report = verify(&schedule, roster, courts);
    print_report(&report);
}
