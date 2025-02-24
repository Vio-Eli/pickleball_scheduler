use std::collections::HashMap;
use itertools::Itertools;
use std::collections::HashSet;
use rand::seq::SliceRandom;
use rand::rng;
use std::cmp::max;
use std::fmt;

#[derive(Debug, Clone)]
struct Team {
    p1: u32,
    p2: u32,
}

impl fmt::Display for Team {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} & {}", self.p1, self.p2)
    }
}

#[derive(Debug, Clone)]
struct Game {
    team1: Team,
    team2: Team,
}

impl fmt::Display for Game {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} vs {}", self.team1, self.team2)
    }
}

#[derive(Debug, Clone)]
struct Round {
    games: Vec<Game>,
    byes: Vec<u32>,
}

#[derive(Debug, Clone)]
struct Rounds {
    rounds: Vec<Round>,
    num_courts: usize,
}

impl fmt::Display for Rounds {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let num_courts = self.num_courts;

        // Determine column widths dynamically
        let mut court_widths = vec![10; num_courts];

        for round in &self.rounds {
            for (j, game) in round.games.iter().enumerate() {
                let game_str = format!("{}", game);
                if j < court_widths.len() {
                    court_widths[j] = std::cmp::max(court_widths[j], game_str.len());
                }
            }
        }

        let bye_width = self.rounds
            .iter()
            .map(|round| round.byes.iter().map(|p| p.to_string()).collect::<Vec<_>>().join(", ").len())
            .max()
            .unwrap_or(10);

        // Print Header
        write!(f, "{:<6} ", "Round")?;
        for i in 1..=num_courts {
            write!(f, "{:<width$} ", format!("Court{}", i), width = court_widths[i - 1])?;
        }
        writeln!(f, "{:<width$}", "Byes", width = bye_width)?;

        // Print Separator
        write!(f, "{:-<6}-", "")?;
        for &width in &court_widths {
            write!(f, "{:-<width$}-", "", width = width)?;
        }
        writeln!(f, "{:-<width$}", "", width = bye_width)?;

        // Print Each Round
        for (i, round) in self.rounds.iter().enumerate() {
            write!(f, "{:<6} ", i + 1)?;

            for j in 0..num_courts {
                if j < round.games.len() {
                    write!(f, "{:<width$} ", format!("{}", round.games[j]), width = court_widths[j])?;
                } else {
                    write!(f, "{:<width$} ", "", width = court_widths[j]); // Empty court slot
                }
            }

            let bye_str = if round.byes.is_empty() {
                "-".to_string()
            } else {
                round.byes.iter().map(|p| p.to_string()).collect::<Vec<_>>().join(", ")
            };
            writeln!(f, "{:<width$}", bye_str, width = bye_width)?;
        }

        Ok(())
    }
}


fn get_shared<T: Eq + std::hash::Hash + Clone>(sets: &[HashSet<T>]) -> HashSet<T> {
    if sets.is_empty() {
        return HashSet::new();
    }

    sets.iter()
        .skip(1)
        .fold(sets[0].clone(), |acc, s| acc.intersection(s).cloned().collect())
}

fn remove_players(m: u32, w: u32, opp_m: u32, opp_w: u32,
                  teams: &mut HashMap<u32, HashSet<u32>>,
                  opps: &mut HashMap<u32, (HashSet<u32>, HashSet<u32>)>) {

    for (player, teammate) in [(m, w), (w, m), (opp_m, opp_w), (opp_w, opp_m)] {
        if let Some(teammates) = teams.get_mut(&player) {
            teammates.remove(&teammate);
        }
    }

    for (player, opp1, opp2) in [(m, opp_m, opp_w), (w, opp_m, opp_w), (opp_m, m, w), (opp_w, opp_m, opp_w)] {
        if let Some((m_opps, w_opps)) = opps.get_mut(&player) {
            m_opps.remove(&opp1);
            w_opps.remove(&opp2);
        }
    }
}

fn remove_empty(teams: &mut HashMap<u32, HashSet<u32>>,
                opps: &mut HashMap<u32, (HashSet<u32>, HashSet<u32>)>,
                ) {

    let mut to_remove: HashSet<u32> = teams.iter()
        .filter_map(|(k, v)| if v.is_empty() { Some(*k) } else { None })
        .chain(opps.iter()
            .filter_map(|(k, (m, w))| if m.is_empty() || w.is_empty() { Some(*k) } else { None }))
        .collect();

    while !to_remove.is_empty() {
        // Remove players from teams, opps, and men
        teams.retain(|k, _| !to_remove.contains(k));
        opps.retain(|k, _| !to_remove.contains(k));
        // men.retain(|k| !to_remove.contains(k));

        // Remove references to removed players in remaining sets
        for teammates in teams.values_mut() {
            teammates.retain(|player| !to_remove.contains(player));
        }
        for (m_opps, w_opps) in opps.values_mut() {
            m_opps.retain(|player| !to_remove.contains(player));
            w_opps.retain(|player| !to_remove.contains(player));
        }

        // Recompute players that now need to be removed
        to_remove = teams.iter()
            .filter_map(|(k, v)| if v.is_empty() { Some(*k) } else { None })
            .chain(opps.iter()
                .filter_map(|(k, (m, w))| if m.is_empty() || w.is_empty() { Some(*k) } else { None }))
            .collect();
    }
}


fn scheduler(num_men: u32, num_women: u32) -> Vec<Game> {
    /* Pickleball scheduler
     *
     * Input is 2 lists of players (Male + Female)
     * Output is a list of doubles games to play ((m, f), (m, f))
     *
     * No two players should play together more than once
     * No two players should play against each other more than once
     * A player cannot play against themselves
     *
     */

    let mut rng = rng();

    // Generate player lists
    let mut men: Vec<u32> = (1..=num_men).collect();
    let mut women: Vec<u32> = (num_men + 1..=num_men + num_women).collect();

    // Shuffle players for randomness
    men.shuffle(&mut rng);
    women.shuffle(&mut rng);

    // Create HashSets for quick lookup
    let mut men_set: HashSet<u32> = men.iter().cloned().collect();
    let mut women_set: HashSet<u32> = women.iter().cloned().collect();

    // Initialize teams and opponents
    let mut teams: HashMap<u32, HashSet<u32>> = HashMap::new();
    let mut opps: HashMap<u32, (HashSet<u32>, HashSet<u32>)> = HashMap::new();

    for m in men {
        let possible_teammates = women_set.clone();
        let mut possible_opponents = men_set.clone();
        possible_opponents.remove(&m);
        teams.insert(m, possible_teammates.clone());
        opps.insert(m, (possible_opponents.clone(), possible_teammates));
    }

    for w in women {
        let possible_teammates = men_set.clone();
        let mut possible_opponents = women_set.clone();
        possible_opponents.remove(&w);
        teams.insert(w, possible_teammates.clone());
        opps.insert(w, (possible_teammates, possible_opponents));
    }

    let mut games: Vec<Game> = vec![];

    'outer: while !teams.is_empty() {
        let mut men_vec: Vec<u32> = teams.keys().cloned().collect();
        if men_vec.is_empty() {
            break;
        }

        men_vec.shuffle(&mut rng);
        for m in men_vec {
            if !teams.contains_key(&m) {
                continue;
            }

            let possible_women = teams.get(&m).unwrap();
            if possible_women.is_empty() {
                continue;
            }

            let &w = possible_women.iter().next().unwrap();

            let (m_opps, w_opps) = opps.get(&m).unwrap();
            let shared_m_opps: HashSet<_> = m_opps.intersection(&opps.get(&w).unwrap().0).cloned().collect();
            let shared_w_opps: HashSet<_> = w_opps.intersection(&opps.get(&w).unwrap().1).cloned().collect();

            if shared_m_opps.is_empty() || shared_w_opps.is_empty() {
                continue;
            }

            let &opp_m = shared_m_opps.iter().next().unwrap();
            let &opp_w = shared_w_opps.iter().next().unwrap();

            games.push(Game {
                team1: Team { p1: m, p2: w },
                team2: Team { p1: opp_m, p2: opp_w },
            });

            // Remove assigned players
            remove_players(m, w, opp_m, opp_w, &mut teams, &mut opps);

            // Remove empty teams and players
            remove_empty(&mut teams, &mut opps);

            if teams.is_empty() {
                break 'outer;
            }
        }
    }

    games
}

fn games_to_courts(mut games: Vec<Game>, num_courts: u32) -> Vec<Round> {
    let mut rounds = vec![];

    // Collect all unique players
    let mut all_players: HashSet<u32> = games
        .iter()
        .flat_map(|game| vec![game.team1.p1, game.team1.p2, game.team2.p1, game.team2.p2])
        .collect();

    while !games.is_empty() {
        let mut current_games = vec![];
        let mut current_players = HashSet::new();

        games.retain(|game| {
            if current_games.len() >= num_courts as usize {
                return true; // Stop if courts are full
            }

            let players = vec![game.team1.p1, game.team1.p2, game.team2.p1, game.team2.p2];

            if players.iter().all(|p| !current_players.contains(p)) {
                current_players.extend(players);
                current_games.push(game.clone());
                false // Remove from `games`
            } else {
                true // Keep in `games`
            }
        });

        let current_byes: Vec<u32> = all_players.difference(&current_players).cloned().collect();
        rounds.push(Round { games: current_games, byes: current_byes });
    }

    rounds
}

fn main() {
    let num_men = 6;
    let num_women = 6;
    let num_courts = 3;

    let games = scheduler(num_men, num_women);
    let rounds = Rounds {
        rounds: games_to_courts(games, num_courts),
        num_courts: num_courts as usize,
    };
    println!("{}", rounds);
}