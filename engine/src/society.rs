//! Society (Stage 5) and conflict (Stage 6).
//!
//! Settlements are DERIVED observations of real building/population clusters — never founded by
//! a generator (Test C). Beliefs originate in individuals from real experiences and spread only
//! through real conversations (replacing prophet-by-threshold, AUDIT §6.6). Violence arises from
//! need and grievance held in real memories; every raid is causally chained; some seeds produce
//! none (Test D). No probability ever gates an outcome whose reasons are not present in state.

use crate::core::ids::{BeliefId, EntityRef, EventId, PersonId, SettlementId};
use crate::history::{Cause, EventKind, StateKind, StateRef};
use crate::humans::{Humans, Memory, MemoryKind};
use crate::sim::Sim;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct Settlement {
    pub name: String,
    pub center: (i32, i32),
    pub founded: u64,
    pub founded_ev: EventId,
    pub abandoned: Option<u64>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Belief {
    pub name: String,
    pub originator: PersonId,
    pub origin_day: u64,
    pub origin_ev: EventId,
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Society {
    pub settlements: Vec<Settlement>,
    pub beliefs: Vec<Belief>,
}

impl Society {
    pub fn settlement_id(idx: usize) -> SettlementId {
        SettlementId::from_index(idx)
    }
    pub fn belief_id(idx: usize) -> BeliefId {
        BeliefId::from_index(idx)
    }
}

/// Monthly society pass + daily conflict pass.
pub fn tick(sim: &mut Sim) {
    let day = sim.clock.day;
    if day % 30 == 13 {
        derive_settlements(sim);
        originate_beliefs(sim);
    }
    raids(sim);
}

/// A settlement exists because buildings and people actually cluster. Identity persists once
/// recognized; abandonment is observed, never deleted.
fn derive_settlements(sim: &mut Sim) {
    let day = sim.clock.day;
    // cluster existing buildings by proximity (radius 8, transitive)
    let blds: Vec<(usize, i32, i32)> = sim
        .objects
        .buildings
        .iter()
        .enumerate()
        .filter(|(_, b)| b.exists)
        .map(|(i, b)| {
            let (x, y) = sim.grid.xy(b.cell as usize);
            (i, x, y)
        })
        .collect();
    let n = blds.len();
    let mut comp: Vec<usize> = (0..n).collect();
    fn find(comp: &mut Vec<usize>, i: usize) -> usize {
        let mut r = i;
        while comp[r] != r {
            r = comp[r];
        }
        let mut c = i;
        while comp[c] != c {
            let nx = comp[c];
            comp[c] = r;
            c = nx;
        }
        r
    }
    for i in 0..n {
        for j in (i + 1)..n {
            let d = (blds[i].1 - blds[j].1).abs().max((blds[i].2 - blds[j].2).abs());
            if d <= 8 {
                let (a, b) = (find(&mut comp, i), find(&mut comp, j));
                if a != b {
                    comp[a] = b;
                }
            }
        }
    }
    use std::collections::BTreeMap;
    let mut clusters: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for i in 0..n {
        let r = find(&mut comp, i);
        clusters.entry(r).or_default().push(i);
    }
    for (_, members) in clusters {
        if members.len() < 3 {
            continue;
        }
        let cx = members.iter().map(|&m| blds[m].1 as i64).sum::<i64>() / members.len() as i64;
        let cy = members.iter().map(|&m| blds[m].2 as i64).sum::<i64>() / members.len() as i64;
        let center = (cx as i32, cy as i32);
        // residents actually here?
        let residents = sim
            .humans
            .list
            .iter()
            .filter(|h| h.alive && (h.x - center.0).abs().max((h.y - center.1).abs()) <= 10)
            .count();
        if residents < 5 {
            continue;
        }
        // already recognized?
        let known = sim
            .society
            .settlements
            .iter()
            .any(|s| {
                s.abandoned.is_none()
                    && (s.center.0 - center.0).abs().max((s.center.1 - center.1).abs()) <= 10
            });
        if known {
            continue;
        }
        // name from the culture of the plurality of residents
        let name = {
            let mut counts = [0usize; 8];
            for h in sim.humans.list.iter().filter(|h| {
                h.alive && (h.x - center.0).abs().max((h.y - center.1).abs()) <= 10
            }) {
                counts[(h.culture as usize) % 8] += 1;
            }
            let cul = counts
                .iter()
                .enumerate()
                .max_by_key(|(_, c)| **c)
                .map(|(i, _)| i as u8)
                .unwrap_or(0);
            let mut rng = crate::core::rng::Rng::entity(
                sim.cfg.seed,
                "settlement_names",
                sim.society.settlements.len() as u64,
            );
            crate::humans::make_name(&mut rng, cul)
        };
        let idx = sim.society.settlements.len();
        let sid = Society::settlement_id(idx);
        let ev = sim.history.push(
            day,
            EntityRef::Settlement(sid),
            EventKind::SettlementFounded { settlement: EntityRef::Settlement(sid) },
            sim.grid.idx(center.0, center.1).map(|i| i as u32),
            members
                .iter()
                .take(4)
                .map(|&m| {
                    Cause::Event(
                        // the founding is caused by the building acts that clustered here
                        sim.history
                            .of_entity(EntityRef::Person(
                                sim.objects.buildings[blds[m].0].builder,
                            ))
                            .first()
                            .copied()
                            .unwrap_or(EventId(1)),
                    )
                })
                .collect(),
        );
        sim.society.settlements.push(Settlement {
            name,
            center,
            founded: day,
            founded_ev: ev,
            abandoned: None,
        });
    }
    // abandonment: recognized settlements with no residents left
    for si in 0..sim.society.settlements.len() {
        if sim.society.settlements[si].abandoned.is_some() {
            continue;
        }
        let c = sim.society.settlements[si].center;
        let residents = sim
            .humans
            .list
            .iter()
            .filter(|h| h.alive && (h.x - c.0).abs().max((h.y - c.1).abs()) <= 10)
            .count();
        if residents == 0 {
            sim.society.settlements[si].abandoned = Some(day);
            let sid = Society::settlement_id(si);
            sim.history.push(
                day,
                EntityRef::Settlement(sid),
                EventKind::SettlementAbandoned { settlement: EntityRef::Settlement(sid) },
                None,
                vec![],
            );
        }
    }
}

/// Beliefs are born in individuals from what actually happened to them: heavy grief or terror
/// in a deeply pious mind crystallizes into a story about the world. No population-scaled dice,
/// no piety scan: the *memory* is the cause and is cited.
fn originate_beliefs(sim: &mut Sim) {
    let day = sim.clock.day;
    let mut originations: Vec<(usize, f32)> = Vec::new();
    for (hi, h) in sim.humans.list.iter().enumerate() {
        if !h.alive || h.piety < 0.8 || !h.beliefs.is_empty() {
            continue;
        }
        let burden: f32 = h
            .memories
            .iter()
            .filter(|m| {
                matches!(m.kind, MemoryKind::KinDied { .. } | MemoryKind::FledFire)
            })
            .map(|m| m.weight)
            .sum();
        if burden > 2.0 {
            originations.push((hi, burden));
        }
    }
    for (hi, burden) in originations {
        let bidx = sim.society.beliefs.len();
        let bid = Society::belief_id(bidx);
        let name = {
            let mut rng =
                crate::core::rng::Rng::entity(sim.cfg.seed, "beliefs", bidx as u64);
            format!("The Way of {}", crate::humans::make_name(&mut rng, sim.humans.list[hi].culture))
        };
        let ev = sim.history.push(
            day,
            EntityRef::Person(Humans::id_of(hi)),
            EventKind::OriginatedBelief { belief: EntityRef::Belief(bid) },
            None,
            vec![Cause::State(StateRef { what: StateKind::Grievance, value: burden })],
        );
        sim.society.beliefs.push(Belief {
            name,
            originator: Humans::id_of(hi),
            origin_day: day,
            origin_ev: ev,
        });
        sim.humans.list[hi].beliefs.push((bidx as u32, 1.0));
    }
}

/// Belief transmission during real conversations — called from humans::socialize.
pub fn share_belief(sim: &mut Sim, from: usize, to: usize) {
    let Some(&(bid, strength)) = sim.humans.list[from].beliefs.first() else { return };
    if strength < 0.5 || sim.humans.list[to].beliefs.iter().any(|(b, _)| *b == bid) {
        return;
    }
    let bond = sim.humans.rel_strength(from, Humans::id_of(to));
    let receptive = {
        let t = &mut sim.humans.list[to];
        let p = 0.05 + t.piety * 0.2 + bond * 0.1;
        t.rng.chance(p)
    };
    if receptive {
        sim.humans.list[to].beliefs.push((bid, 0.6));
        let origin_ev = sim.society.beliefs[bid as usize].origin_ev;
        sim.history.push(
            sim.clock.day,
            EntityRef::Person(Humans::id_of(to)),
            EventKind::AdoptedBelief {
                belief: EntityRef::Belief(Society::belief_id(bid as usize)),
                from: EntityRef::Person(Humans::id_of(from)),
            },
            None,
            vec![Cause::Event(origin_ev)],
        );
    }
}

/// Violence from need and grudge — never from a scheduler. A desperate, brave person who can
/// see an out-group granary may raid it; kin of anyone killed carry the memory, and that memory
/// (cited as cause) can drive retaliation years later. Some seeds never see a raid.
fn raids(sim: &mut Sim) {
    let day = sim.clock.day;
    let n = sim.humans.list.len();
    let mut raids_to_run: Vec<(usize, usize)> = Vec::new(); // raider, building
    for hi in 0..n {
        let h = &sim.humans.list[hi];
        if !h.alive {
            continue;
        }
        let desperate = h.days_starving >= 6 && h.hunger > 1.2;
        let grudge: f32 = h
            .memories
            .iter()
            .filter(|m| matches!(m.kind, MemoryKind::KinDied { .. }))
            .map(|m| m.weight * 0.5)
            .sum();
        if !desperate && grudge < 1.5 {
            continue;
        }
        if h.traits[1] < 0.55 {
            continue; // it takes courage to cross that line
        }
        // a real target: an out-culture building with food, actually within sight
        let mut target = None;
        for (bi, b) in sim.objects.buildings.iter().enumerate() {
            if !b.exists || b.food_store < 1.0 {
                continue;
            }
            let (bx, by) = sim.grid.xy(b.cell as usize);
            if (bx - h.x).abs().max((by - h.y).abs()) > 9 {
                continue;
            }
            let owner_culture = sim
                .humans
                .get(b.builder)
                .map(|o| o.culture)
                .unwrap_or(255);
            if owner_culture != h.culture {
                target = Some(bi);
                break;
            }
        }
        if let Some(bi) = target {
            raids_to_run.push((hi, bi));
        }
    }
    for (hi, bi) in raids_to_run {
        if !sim.humans.list[hi].alive || !sim.objects.buildings[bi].exists {
            continue;
        }
        let (bx, by) = sim.grid.xy(sim.objects.buildings[bi].cell as usize);
        let dist = {
            let h = &sim.humans.list[hi];
            (bx - h.x).abs().max((by - h.y).abs())
        };
        if dist > 1 {
            continue; // approach happens through normal movement on later days
        }
        // the raid: take food; a defender who is actually present may fight
        let hunger_now = sim.humans.list[hi].hunger;
        let grudge_ev = sim.humans.list[hi]
            .memories
            .iter()
            .filter(|m| matches!(m.kind, MemoryKind::KinDied { .. }))
            .map(|m| m.day)
            .next();
        let take = {
            let b = &mut sim.objects.buildings[bi];
            let t = b.food_store.min(4.0);
            b.food_store -= t;
            t
        };
        let mut causes = vec![Cause::State(StateRef { what: StateKind::Hunger, value: hunger_now })];
        if let Some(gd) = grudge_ev {
            // find the kin-death event on that day for the causal chain
            if let Some(ev) = sim
                .history
                .events
                .iter()
                .rev()
                .find(|e| e.day == gd && matches!(e.kind, EventKind::Died { .. }))
            {
                causes.push(Cause::Event(ev.id));
            }
        }
        let owner = sim.objects.buildings[bi].builder;
        let raid_ev = sim.history.push(
            day,
            EntityRef::Person(Humans::id_of(hi)),
            EventKind::RaidCarriedOut { against: EntityRef::Person(owner) },
            sim.grid.idx(bx, by).map(|i| i as u32),
            causes,
        );
        {
            let h = &mut sim.humans.list[hi];
            h.carried_food += take;
            h.hunger = (h.hunger - take.min(1.0)).max(0.0);
        }
        // a present defender fights; someone can die, and that death breeds the next grudge
        let defender = (0..n).find(|&oi| {
            let o = &sim.humans.list[oi];
            o.alive
                && oi != hi
                && o.culture != sim.humans.list[hi].culture
                && (o.x - bx).abs().max((o.y - by).abs()) <= 1
        });
        if let Some(di) = defender {
            let p_raider_wins = {
                let r = &sim.humans.list[hi];
                let d = &sim.humans.list[di];
                let ra = 0.4 + r.traits[1] * 0.4 + r.skills[crate::humans::SK_FIGHT] * 0.4;
                let da = 0.4 + d.traits[1] * 0.4 + d.skills[crate::humans::SK_FIGHT] * 0.4;
                (ra / (ra + da)).clamp(0.15, 0.85)
            };
            let roll = sim.humans.list[hi].rng.chance(p_raider_wins * 0.4); // most raids end in flight
            if roll {
                let kill_ev = sim.history.push(
                    day,
                    EntityRef::Person(Humans::id_of(di)),
                    EventKind::Killed { by: EntityRef::Person(Humans::id_of(hi)) },
                    sim.grid.idx(bx, by).map(|i| i as u32),
                    vec![Cause::Event(raid_ev)],
                );
                crate::humans::kill_human(
                    sim,
                    di,
                    crate::history::DeathCause::Murder,
                    vec![Cause::Event(kill_ev)],
                );
            } else {
                let h = &mut sim.humans.list[hi];
                h.fear = (h.fear + 0.8).min(2.0);
            }
        }
    }
}

/// Derived observation: belief clusters ("religions") with adherent counts.
pub fn belief_clusters(sim: &Sim) -> Vec<(String, usize)> {
    let mut counts = vec![0usize; sim.society.beliefs.len()];
    for h in &sim.humans.list {
        if !h.alive {
            continue;
        }
        for (b, s) in &h.beliefs {
            if *s > 0.2 {
                counts[*b as usize] += 1;
            }
        }
    }
    sim.society
        .beliefs
        .iter()
        .zip(counts)
        .map(|(b, c)| (b.name.clone(), c))
        .collect()
}

/// Derived observation: settlements alive now.
pub fn living_settlements(sim: &Sim) -> Vec<(String, (i32, i32))> {
    sim.society
        .settlements
        .iter()
        .filter(|s| s.abandoned.is_none())
        .map(|s| (s.name.clone(), s.center))
        .collect()
}

// keep Memory import used
#[allow(dead_code)]
fn _t(_m: &Memory) {}
