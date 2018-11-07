// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use abstutil::{deserialize_btreemap, serialize_btreemap, Error};
use map_model::{IntersectionID, LaneID, Map, TurnID, TurnType};
use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, Copy, PartialOrd)]
pub enum TurnPriority {
    Stop,
    Yield,
    Priority,
}

// This represents a single intersection controlled by a stop sign-like policy. The turns are
// partitioned into three groups:
//
// 1) Priority turns - these must be non-conflicting, and cars don't have to stop before doing this
//    turn.
// 2) Yields - cars can do this immediately if there are no previously accepted conflicting turns.
//    should maybe check that these turns originate from roads with priority turns.
// 3) Stops - cars must stop before doing this turn, and they are accepted with the lowest priority
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ControlStopSign {
    intersection: IntersectionID,
    #[serde(serialize_with = "serialize_btreemap")]
    #[serde(deserialize_with = "deserialize_btreemap")]
    turns: BTreeMap<TurnID, TurnPriority>,
    // TODO
    changed: bool,
}

impl ControlStopSign {
    pub fn new(map: &Map, intersection: IntersectionID) -> ControlStopSign {
        assert!(!map.get_i(intersection).has_traffic_signal);
        let ss = ControlStopSign::smart_assignment(map, intersection);
        ss.validate(map, intersection).unwrap();
        ss
    }

    fn smart_assignment(map: &Map, intersection: IntersectionID) -> ControlStopSign {
        if map.get_i(intersection).roads.len() <= 2 {
            return ControlStopSign::for_degenerate_and_deadend(map, intersection);
        }

        // Higher numbers are higher rank roads
        let mut rank_per_incoming_lane: HashMap<LaneID, usize> = HashMap::new();
        let mut ranks: HashSet<usize> = HashSet::new();
        let mut highest_rank = 0;
        // TODO should just be incoming, but because of weirdness with sidewalks...
        for l in map
            .get_i(intersection)
            .incoming_lanes
            .iter()
            .chain(map.get_i(intersection).outgoing_lanes.iter())
        {
            let r = map.get_parent(*l);
            let rank = if let Some(highway) = r.osm_tags.get("highway") {
                match highway.as_ref() {
                    "motorway" => 20,
                    "motorway_link" => 19,

                    "trunk" => 17,
                    "trunk_link" => 16,

                    "primary" => 15,
                    "primary_link" => 14,

                    "secondary" => 13,
                    "secondary_link" => 12,

                    "tertiary" => 10,
                    "tertiary_link" => 9,

                    "residential" => 5,

                    "footway" => 1,

                    "unclassified" => 0,
                    "road" => 0,
                    _ => panic!("Unknown OSM highway {}", highway),
                }
            } else {
                0
            };
            rank_per_incoming_lane.insert(*l, rank);
            highest_rank = highest_rank.max(rank);
            ranks.insert(rank);
        }
        if ranks.len() == 1 {
            return ControlStopSign::all_way_stop(map, intersection);
        }

        let mut ss = ControlStopSign {
            intersection,
            turns: BTreeMap::new(),
            changed: false,
        };
        for t in &map.get_i(intersection).turns {
            if rank_per_incoming_lane[&t.src] == highest_rank {
                // If it's the highest rank road, make the straight and right turns priority (if
                // possible) and other turns yield.
                let turn = map.get_t(*t);
                if (turn.is_right_turn(map) || turn.is_straight_turn(map))
                    && ss.could_be_priority_turn(*t, map)
                {
                    ss.turns.insert(*t, TurnPriority::Priority);
                } else {
                    ss.turns.insert(*t, TurnPriority::Yield);
                }
            } else {
                // Lower rank roads have to stop.
                ss.turns.insert(*t, TurnPriority::Stop);
            }
        }
        ss
    }

    fn all_way_stop(map: &Map, intersection: IntersectionID) -> ControlStopSign {
        let mut ss = ControlStopSign {
            intersection,
            turns: BTreeMap::new(),
            changed: false,
        };
        for t in &map.get_i(intersection).turns {
            ss.turns.insert(*t, TurnPriority::Stop);
        }
        ss
    }

    fn for_degenerate_and_deadend(map: &Map, i: IntersectionID) -> ControlStopSign {
        let mut ss = ControlStopSign {
            intersection: i,
            turns: BTreeMap::new(),
            changed: false,
        };
        for t in &map.get_i(i).turns {
            // Only the crosswalks should conflict with other turns.
            let priority = match map.get_t(*t).turn_type {
                TurnType::Crosswalk => TurnPriority::Stop,
                _ => TurnPriority::Priority,
            };
            ss.turns.insert(*t, priority);
        }

        // Due to a few observed issues (multiple driving lanes road (a temporary issue) and bad
        // intersection geometry), sometimes more turns conflict than really should. For now, just
        // detect and fallback to an all-way stop.
        if let Err(err) = ss.validate(map, i) {
            warn!("Giving up on for_degenerate_and_deadend({}): {}", i, err);
            return ControlStopSign::all_way_stop(map, i);
        }

        ss
    }

    pub fn get_priority(&self, turn: TurnID) -> TurnPriority {
        self.turns[&turn]
    }

    pub fn set_priority(&mut self, turn: TurnID, priority: TurnPriority, map: &Map) {
        if priority == TurnPriority::Priority {
            assert!(self.could_be_priority_turn(turn, map));
        }
        self.turns.insert(turn, priority);
        self.changed = true;
    }

    pub fn could_be_priority_turn(&self, id: TurnID, map: &Map) -> bool {
        for (t, pri) in &self.turns {
            if *pri == TurnPriority::Priority && map.get_t(id).conflicts_with(map.get_t(*t)) {
                return false;
            }
        }
        true
    }

    pub fn is_changed(&self) -> bool {
        // TODO detect edits that've been undone, equivalent to original
        self.changed
    }

    pub fn is_priority_lane(&self, lane: LaneID) -> bool {
        self.turns
            .iter()
            .find(|(turn, pri)| turn.src == lane && **pri > TurnPriority::Stop)
            .is_some()
    }

    fn validate(&self, map: &Map, intersection: IntersectionID) -> Result<(), Error> {
        // Does the assignment cover the correct set of turns?
        let all_turns = &map.get_i(intersection).turns;
        assert_eq!(self.turns.len(), all_turns.len());
        for t in all_turns {
            assert!(self.turns.contains_key(t));
        }

        // Do any of the priority turns conflict?
        let priority_turns: Vec<TurnID> = self
            .turns
            .iter()
            .filter_map(|(turn, pri)| {
                if *pri == TurnPriority::Priority {
                    Some(*turn)
                } else {
                    None
                }
            }).collect();
        for t1 in &priority_turns {
            for t2 in &priority_turns {
                if map.get_t(*t1).conflicts_with(map.get_t(*t2)) {
                    return Err(Error::new(format!(
                        "Stop sign has conflicting priority turns {:?} and {:?}",
                        t1, t2
                    )));
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn ordering() {
        use stop_signs::TurnPriority;
        assert!(TurnPriority::Priority > TurnPriority::Yield);
    }
}
