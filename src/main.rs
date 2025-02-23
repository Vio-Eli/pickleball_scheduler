use std::collections::HashMap;
use itertools::Itertools;
use std::collections::HashSet;
use rand::seq::SliceRandom;
use rand::rng;

fn get_shared<T: Eq + std::hash::Hash + Clone>(sets: &[HashSet<T>]) -> HashSet<T> {
    sets.iter()
        .skip(1)
        .fold(sets.first().cloned().unwrap_or_default(), |acc, s| {
            acc.intersection(s).cloned().collect()
        })
}

fn remove_players(m: &str, w: &str, opp_m: &str, opp_w: &str, teams: &mut HashMap<&str, HashSet<&str>>, opps: &mut HashMap<&str, (HashSet<&str>, HashSet<&str>)>) {
    teams.get_mut(m).unwrap().remove(w);
    teams.get_mut(w).unwrap().remove(m);
    teams.get_mut(opp_m).unwrap().remove(opp_w);
    teams.get_mut(opp_w).unwrap().remove(opp_m);

    if let Some((m_o, w_o)) = opps.get_mut(m) {
        m_o.remove(opp_m);
        w_o.remove(opp_w);
    }
    if let Some((m_o, w_o)) = opps.get_mut(w) {
        m_o.remove(opp_m);
        w_o.remove(opp_w);
    }
    if let Some((m_o, w_o)) = opps.get_mut(opp_m) {
        m_o.remove(m);
        w_o.remove(w);
    }
    if let Some((m_o, w_o)) = opps.get_mut(opp_w) {
        m_o.remove(opp_m);
        w_o.remove(opp_w);
    }
}

fn remove_empty(teams: &mut HashMap<&str, HashSet<&str>>, opps: &mut HashMap<&str, (HashSet<&str>, HashSet<&str>)>, men: &mut HashSet<&str>) {
    let mut to_remove: HashSet<_> = teams.iter()
        .filter(|(_, v)| v.is_empty())
        .map(|(k, _)| *k)
        .chain(
            opps.iter()
                .filter(|(_, (m, w))| m.is_empty() || w.is_empty())
                .map(|(k, _)| *k)
        )
        .collect();

    while to_remove.len() > 0 {
        // Remove from `teams` and `opps`
        teams.retain(|k, _| !to_remove.contains(k));
        opps.retain(|k, _| !to_remove.contains(k));
        men.retain(|&k| !to_remove.contains(k));

        // Remove references to removed players in remaining sets
        for v in teams.values_mut() {
            v.retain(|player| !to_remove.contains(player));
        }
        for (m, w) in opps.values_mut() {
            m.retain(|player| !to_remove.contains(player));
            w.retain(|player| !to_remove.contains(player));
        }

        to_remove = teams.iter()
            .filter(|(_, v)| v.is_empty())
            .map(|(k, _)| *k)
            .chain(
                opps.iter()
                    .filter(|(_, (m, w))| m.is_empty() || w.is_empty())
                    .map(|(k, _)| *k)
            )
            .collect();
    }
}

fn scheduler() {
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

    let mut men_vec = vec!["m1", "m2", "m3", "m4", "m5"];
    let mut counter: usize = 0;
    let mut women_vec = vec!["w1", "w2", "w3", "w4", "w5", "w6"];

    // shuffle men and women
    men_vec.shuffle(&mut rng());
    women_vec.shuffle(&mut rng());

    // cast to HashSet
    let mut men: HashSet<&str> = HashSet::from_iter(men_vec);
    let women: HashSet<&str> = HashSet::from_iter(women_vec);

    let mut teams: HashMap<&str, HashSet<&str>> = HashMap::new();
    let mut opps: HashMap<&str, (HashSet<&str>, HashSet<&str>)> = HashMap::new();

    for m in &men {
        let mut p_m = men.clone();
        p_m.remove(m);
        teams.insert(m, women.clone());
        opps.insert(m, (p_m.clone(), women.clone()));
    }

    for w in &women {
        let mut w_m = women.clone();
        w_m.remove(w);
        teams.insert(w, men.clone());
        opps.insert(w, (men.clone(), w_m.clone()));
    }

    println!("Teams: {:?}", teams);
    println!("Opps: {:?}", opps);

    let mut games: Vec<((&str, &str), (&str, &str))> = vec![];

    'out: while !teams.is_empty() && !opps.is_empty() {
        let mut local_counter: usize = counter;

        'p1: loop {

            // let mut m_iter = men.clone().into_iter();
            let men_vec: Vec<&&str> = men.iter().collect();
            if men_vec.is_empty() {
                break 'out;
            }
            let m = *men_vec[local_counter % men_vec.len()];
            local_counter += 1;

            println!("BEGIN MEN: {:?}", men);
            println!("m: {:?}", m);
            let mut w_itr = teams.get(m).unwrap().iter();

            'p2: while let Some(w) = w_itr.next() {
                // let mut w = w_itr.next().cloned().unwrap(); // get teammate from possible teammates hashmap

                println!("dude: {:?}, girl: {:?}", m, w);

                // get opponents from possible opponents hashmap for both m and w
                let mm_opps = opps.get(m).unwrap();
                let mut ww_opps = opps.get(w).unwrap();

                println!("mm_opps: {:?}, ww_opps: {:?}", mm_opps, ww_opps);

                // get the intersection of the opponents for m and w
                let mut shared_m_opps = get_shared(&[mm_opps.0.clone(), ww_opps.0.clone()]);
                let mut shared_w_opps = get_shared(&[mm_opps.1.clone(), ww_opps.1.clone()]);

                // remove m and w from the shared opponents
                shared_m_opps.remove(m);
                shared_w_opps.remove(w);

                println!("shared_m_opps: {:?}, shared_w_opps: {:?}", shared_m_opps, shared_w_opps);

                if shared_m_opps.is_empty() || shared_w_opps.is_empty() {
                    continue 'p2;
                }

                // get the next male opponent
                let mut opp_m_itr = shared_m_opps.iter();
                // let mut opp_m = opp_m_itr.next().cloned().unwrap();
                'p3: while let Some(opp_m) = opp_m_itr.next() {
                    println!("opp_m: {:?}", opp_m);

                    // get the possible women teammates for opp_m
                    let mut opp_m_team = teams.get(opp_m).unwrap();

                    println!("opp_m_team: {:?}", opp_m_team);

                    // get the intersection of the possible women teammates
                    let mut opp_w_team = get_shared(&[opp_m_team.clone(), shared_w_opps.clone()]);

                    println!("opp_w_team: {:?}", opp_w_team);

                    if opp_w_team.is_empty() {
                        continue 'p3;
                    }

                    // get the next women opponent
                    let opp_w = opp_w_team.iter().next().cloned().unwrap();

                    println!("opp_w: {:?}", opp_w);

                    // add the game to the games list
                    games.push(((m, w), (opp_m, opp_w)));

                    // remove the players from the possible teammates and opponents
                    remove_players(m, w, opp_m, opp_w, &mut teams, &mut opps);

                    // If any player in teams has no possible teammates, remove them from the teams hashmap
                    remove_empty(&mut teams, &mut opps, &mut men);

                    println!("Games: {:?}", games);
                    println!("Teams: {:?}", teams);
                    println!("Opps: {:?}", opps);

                    if teams.is_empty() || opps.is_empty() {
                        break 'out;
                    }

                    continue 'out;
                }

                println!("Could not find a valid opponent for {:?}", m);
            }

            println!("Could not find a valid teammate for {:?}", m);

            if local_counter % men_vec.len() == counter % men_vec.len() {
                println!("All options exhausted. Terminating.");
                break 'out;
            }
        }
    }

    println!("FINAL Games: {:?}", games);
}


fn main() {
    scheduler();
}