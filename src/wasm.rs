//! Browser bindings: a thin wrapper exposing the engine to JavaScript.
//!
//! The real work stays in the tested core (`search`, `target`, `construct`,
//! `verify`). This module just dispatches by mode and serializes the result to
//! a JSON string the web page renders. The serialization lives in the plain
//! [`generate_json`] function (unit-tested on the host); the `#[wasm_bindgen]`
//! entry point is a one-line shim over it, compiled only under the `wasm`
//! feature.

use crate::model::{Roster, Schedule};
use crate::search::{optimize, EMPHASIS_BALANCED, EMPHASIS_COURTS, EMPHASIS_VARIETY};
use crate::target::{by_games_per_player, by_total_games};
use crate::verify::{verify, Report};

/// Guard against inputs that would freeze the browser (the heuristic is
/// `O(M²W²)` per court). Realistic pickleball sessions are well within this.
const MAX_SIDE: u16 = 30;

/// Modes: `0` courts, `1` balanced, `2` variety (Part 1); `3` each=param,
/// `4` total=param (Part 2). `param` is ignored for Part 1.
pub fn generate_json(men: u16, women: u16, courts: u16, mode: u8, param: u32, seed: u32) -> String {
    if men > MAX_SIDE || women > MAX_SIDE {
        return format!(
            "{{\"error\":\"Too many players — cap each side at {} for the browser.\"}}",
            MAX_SIDE
        );
    }
    let roster = Roster::new(men, women);
    let courts = courts.max(1);
    let seed = seed as u64;
    let iters = 20_000u32;

    let sched = match mode {
        0 => optimize(roster, courts, iters, EMPHASIS_COURTS, seed),
        2 => optimize(roster, courts, iters, EMPHASIS_VARIETY, seed),
        3 => by_games_per_player(roster, courts, param, seed),
        4 => by_total_games(roster, courts, param as usize, seed),
        _ => optimize(roster, courts, iters, EMPHASIS_BALANCED, seed),
    };
    let report = verify(&sched, roster, courts);
    to_json(&sched, &report)
}

fn json_usize_arr(v: &[usize]) -> String {
    let mut s = String::from("[");
    for (i, x) in v.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&x.to_string());
    }
    s.push(']');
    s
}

fn to_json(sched: &Schedule, r: &Report) -> String {
    let mut out = String::new();
    out.push('{');
    out.push_str(&format!(
        "\"men\":{},\"women\":{},\"courts\":{},",
        r.roster.men, r.roster.women, r.courts
    ));

    // rounds: [[ [ma,wa,mb,wb], ... ], ...]  (0-indexed)
    out.push_str("\"rounds\":[");
    for (ri, round) in sched.rounds.iter().enumerate() {
        if ri > 0 {
            out.push(',');
        }
        out.push('[');
        for (gi, g) in round.games.iter().enumerate() {
            if gi > 0 {
                out.push(',');
            }
            out.push_str(&format!(
                "[{},{},{},{}]",
                g.a.man.0, g.a.woman.0, g.b.man.0, g.b.woman.0
            ));
        }
        out.push(']');
    }
    out.push_str("],");

    out.push_str("\"report\":{");
    out.push_str(&format!(
        "\"games\":{},\"maxGames\":{},\"rounds\":{},",
        r.games, r.max_games, r.rounds
    ));
    out.push_str(&format!(
        "\"legal\":{},\"structurallyValid\":{},",
        r.is_legal(),
        r.is_structurally_valid()
    ));
    out.push_str(&format!(
        "\"partnerExcess\":{},\"partnerFloor\":{},",
        r.partner_repeat_excess, r.partner_repeat_floor
    ));
    out.push_str(&format!(
        "\"mixedExcess\":{},\"mixedFloor\":{},",
        r.mixed_repeat_excess, r.mixed_repeat_floor
    ));
    out.push_str(&format!(
        "\"manExcess\":{},\"manFloor\":{},\"manMax\":{},",
        r.man_repeat_excess, r.man_repeat_floor, r.man_max_meetings
    ));
    out.push_str(&format!(
        "\"womanExcess\":{},\"womanFloor\":{},\"womanMax\":{},",
        r.woman_repeat_excess, r.woman_repeat_floor, r.woman_max_meetings
    ));
    out.push_str(&format!(
        "\"courtUtil\":{:.4},\"spread\":{},",
        r.court_utilization,
        r.participation_spread()
    ));
    out.push_str(&format!(
        "\"gamesPerMan\":{},\"gamesPerWoman\":{}",
        json_usize_arr(&r.games_per_man),
        json_usize_arr(&r.games_per_woman)
    ));
    out.push('}');

    out.push('}');
    out
}

#[cfg(feature = "wasm")]
mod bindings {
    use wasm_bindgen::prelude::*;

    /// Generate a schedule and return it as a JSON string.
    #[wasm_bindgen]
    pub fn generate(men: u16, women: u16, courts: u16, mode: u8, param: u32, seed: u32) -> String {
        super::generate_json(men, women, courts, mode, param, seed)
    }
}
