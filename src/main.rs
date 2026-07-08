//! CLI entry point.
//!
//! Usage: `pickleball_scheduler [men] [women] [courts] [emphasis] [ls_iters] [seed]`
//! (defaults: 6 6 3 balanced 40000)
//!
//! `emphasis` is `courts` | `balanced` | `variety`, trading court utilization
//! against fewer same-gender repeats.

use pickleball_scheduler::model::Roster;
use pickleball_scheduler::report::{print_report, print_schedule};
use pickleball_scheduler::search::{optimize, EMPHASIS_BALANCED, EMPHASIS_COURTS, EMPHASIS_VARIETY};
use pickleball_scheduler::verify::verify;

fn arg<T: std::str::FromStr>(args: &[String], i: usize, default: T) -> T {
    args.get(i).and_then(|s| s.parse().ok()).unwrap_or(default)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let men: u16 = arg(&args, 1, 6);
    let women: u16 = arg(&args, 2, 6);
    let courts: u16 = arg(&args, 3, 3);
    let emphasis = args.get(4).map(String::as_str).unwrap_or("balanced");
    let ls_iters: u32 = arg(&args, 5, 40_000);
    let seed: u64 = arg(&args, 6, 0xDEAD_BEEF);

    let round_weight = match emphasis {
        "courts" => EMPHASIS_COURTS,
        "variety" => EMPHASIS_VARIETY,
        _ => EMPHASIS_BALANCED,
    };

    let roster = Roster::new(men, women);
    let schedule = optimize(roster, courts, ls_iters, round_weight, seed);

    println!("(emphasis: {})", emphasis);
    print_schedule(&schedule, roster, courts);
    let report = verify(&schedule, roster, courts);
    print_report(&report);
}
