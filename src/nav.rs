//! Bot navigation for any level, built from its blocks: a grid of places a fighter can stand
//! (one every metre, on every surface with head room), linked by how to get from one to the next
//! (walk, jump up, drop down, jump a gap, dash a wider gap), and the shortest way between two
//! places over it (A*). Built once per level and shared.

use crate::math::*;
use crate::world::World;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};
use std::sync::{Arc, Mutex, OnceLock};

/// Grid spacing (metres).
pub const CELL: f32 = 1.0;
const RADIUS: f32 = 0.36;
const HEIGHT: f32 = 1.8;
/// Walked up without a jump (the game steps up 0.55 m).
const STEP: f32 = 0.55;
/// The highest ledge a jump reaches (a jump peaks near 1.9 m).
const JUMP_UP: f32 = 1.7;
/// Gaps a running jump clears (same height), and what a jump and a dash clear.
const JUMP_GAP: f32 = 4.2;
const DASH_GAP: f32 = 7.6;
/// The furthest drop taken.
const MAX_DROP: f32 = 9.0;

/// How a link is crossed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Move {
    Walk,
    /// Jump up onto something, or across a gap.
    Jump,
    /// Step off and fall.
    Drop,
    /// Jump, then dash across a gap a jump alone won't clear.
    Dash,
}

#[derive(Clone, Copy, Debug)]
pub struct Link {
    pub to: u32,
    pub how: Move,
    pub cost: f32,
}

pub struct Nav {
    /// Where a fighter stands (feet).
    pub nodes: Vec<V3>,
    pub links: Vec<Vec<Link>>,
    cells: HashMap<(i32, i32), Vec<u32>>,
}

fn cell_of(p: V3) -> (i32, i32) {
    ((p.x / CELL).floor() as i32, (p.z / CELL).floor() as i32)
}

/// A level's navigation (built the first time it's asked for, then shared).
pub fn for_world(w: &World) -> Arc<Nav> {
    static CACHE: OnceLock<Mutex<Vec<(u64, Arc<Nav>)>>> = OnceLock::new();
    let key = fingerprint(w);
    let cache = CACHE.get_or_init(|| Mutex::new(Vec::new()));
    if let Some(n) = cache.lock().unwrap().iter().find(|(k, _)| *k == key).map(|(_, n)| n.clone()) {
        return n;
    }
    let nav = Arc::new(Nav::build(w));
    let mut c = cache.lock().unwrap();
    if c.len() > 8 {
        c.remove(0);
    }
    c.push((key, nav.clone()));
    nav
}

fn fingerprint(w: &World) -> u64 {
    let mut h: u64 = 1469598103934665603;
    let mut mix = |v: f32| {
        h ^= v.to_bits() as u64;
        h = h.wrapping_mul(1099511628211);
    };
    mix(w.half);
    for b in &w.blocks {
        for v in [b.b.min.x, b.b.min.y, b.b.min.z, b.b.max.x, b.b.max.y, b.b.max.z] {
            mix(v);
        }
    }
    h
}

/// A fighter's body standing at `p` (a little slimmer, so it fits where the game lets it).
fn body(p: V3, lift: f32) -> Aabb {
    let r = RADIUS * 0.9;
    Aabb::new(v3(p.x - r, p.y + lift, p.z - r), v3(p.x + r, p.y + HEIGHT, p.z + r))
}

impl Nav {
    pub fn build(w: &World) -> Nav {
        let h = w.half;
        let n = (2.0 * h / CELL).round() as i32;
        let mut nodes: Vec<V3> = Vec::new();
        let mut cells: HashMap<(i32, i32), Vec<u32>> = HashMap::new();
        for ix in 0..n {
            for iz in 0..n {
                let x = -h + (ix as f32 + 0.5) * CELL;
                let z = -h + (iz as f32 + 0.5) * CELL;
                let mut tops: Vec<f32> = Vec::new();
                for bl in &w.blocks {
                    let b = &bl.b;
                    if x >= b.min.x - 0.02 && x <= b.max.x + 0.02 && z >= b.min.z - 0.02 && z <= b.max.z + 0.02 {
                        let y = b.max.y;
                        if tops.iter().any(|t| (t - y).abs() < 0.3) {
                            continue;
                        }
                        // head room: nothing where the body would be
                        let room = body(v3(x, y, z), 0.05);
                        if w.blocks.iter().any(|o| o.b.overlaps(&room)) {
                            continue;
                        }
                        tops.push(y);
                    }
                }
                for y in tops {
                    let id = nodes.len() as u32;
                    nodes.push(v3(x, y, z));
                    cells.entry(cell_of(v3(x, y, z))).or_default().push(id);
                }
            }
        }
        let mut nav = Nav { links: vec![Vec::new(); nodes.len()], nodes, cells };
        nav.link(w);
        nav
    }

    fn at(&self, c: (i32, i32)) -> &[u32] {
        self.cells.get(&c).map_or(&[], |v| v.as_slice())
    }

    fn link(&mut self, w: &World) {
        let dirs = [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)];
        let up = v3(0.0, 1.0, 0.0);
        // a body-wide line clear (the middle and both sides)
        let wide = |a: V3, b: V3| -> bool {
            let d = (b - a).flat();
            let side = if d.len() > 1e-4 { v3(-d.z, 0.0, d.x).norm() * 0.3 } else { V3::ZERO };
            w.line_clear(a, b) && w.line_clear(a + side, b + side) && w.line_clear(a - side, b - side)
        };
        for a in 0..self.nodes.len() {
            let pa = self.nodes[a];
            let ca = cell_of(pa);
            let mut out = Vec::new();
            for (dx, dz) in dirs {
                // the next cell over
                for &b in self.at((ca.0 + dx, ca.1 + dz)) {
                    let pb = self.nodes[b as usize];
                    let dy = pb.y - pa.y;
                    let dist = (pb - pa).flat().len();
                    let top = pa.y.max(pb.y);
                    let how = if dy.abs() <= STEP {
                        let ok = wide(v3(pa.x, top + 0.9, pa.z), v3(pb.x, top + 0.9, pb.z)) && w.line_clear(v3(pa.x, top + 1.6, pa.z), v3(pb.x, top + 1.6, pb.z));
                        ok.then_some(Move::Walk)
                    } else if dy > STEP && dy <= JUMP_UP {
                        let ok = w.line_clear(pa + up * 0.9, pa + up * (dy + 1.7)) && wide(v3(pa.x, pb.y + 0.9, pa.z), v3(pb.x, pb.y + 0.9, pb.z));
                        ok.then_some(Move::Jump)
                    } else if dy < -STEP && -dy <= MAX_DROP {
                        let edge = v3(pb.x, pa.y + 0.9, pb.z);
                        let ok = wide(pa + up * 0.9, edge) && w.line_clear(edge, pb + up * 0.9);
                        ok.then_some(Move::Drop)
                    } else {
                        None
                    };
                    if let Some(how) = how {
                        let extra = match how {
                            Move::Walk => 0.0,
                            Move::Jump => 0.6,
                            Move::Drop => 0.3,
                            Move::Dash => 2.0,
                        };
                        out.push(Link { to: b, how, cost: dist + dy.abs() * 0.3 + extra });
                    }
                }
                // gaps: the first place across one at about the same height (or lower)
                'gap: for k in 2..=7i32 {
                    let c = (ca.0 + dx * k, ca.1 + dz * k);
                    // anything walkable on the way at this height means it isn't a gap
                    let between_solid = (1..k).any(|m| self.at((ca.0 + dx * m, ca.1 + dz * m)).iter().any(|&o| (self.nodes[o as usize].y - pa.y).abs() < 0.6));
                    if between_solid {
                        break;
                    }
                    for &b in self.at(c) {
                        let pb = self.nodes[b as usize];
                        let dy = pb.y - pa.y;
                        let dist = (pb - pa).flat().len();
                        // across and a little up (a short hop up onto a roof), or level, or down
                        if (dy > 0.6 && !(dy <= 1.3 && dist <= 2.9)) || dy < -2.5 {
                            continue;
                        }
                        let how = if dist <= JUMP_GAP {
                            Move::Jump
                        } else if dist <= DASH_GAP {
                            Move::Dash
                        } else {
                            continue;
                        };
                        let y = pa.y.max(pb.y);
                        if wide(v3(pa.x, y + 0.9, pa.z), v3(pb.x, y + 0.9, pb.z)) && w.line_clear(v3(pa.x, y + 1.8, pa.z), v3(pb.x, y + 1.8, pb.z)) {
                            out.push(Link { to: b, how, cost: dist + if how == Move::Dash { 3.0 } else { 1.2 } });
                            break 'gap;
                        }
                    }
                }
            }
            self.links[a] = out;
        }
    }

    /// The place to start from (or aim for) near `p`: the nearest node no higher than a step
    /// above it, preferring ones at its own level.
    pub fn nearest(&self, p: V3) -> Option<u32> {
        let c = cell_of(p);
        let mut best: Option<(f32, u32)> = None;
        for r in 0..=3i32 {
            for dx in -r..=r {
                for dz in -r..=r {
                    if dx.abs() != r && dz.abs() != r {
                        continue;
                    }
                    for &id in self.at((c.0 + dx, c.1 + dz)) {
                        let q = self.nodes[id as usize];
                        let dy = q.y - p.y;
                        if dy > 0.7 || dy < -3.5 {
                            continue;
                        }
                        let score = (q - p).flat().len() + dy.abs() * 1.5;
                        if best.map_or(true, |(s, _)| score < s) {
                            best = Some((score, id));
                        }
                    }
                }
            }
            if best.is_some() && r >= 1 {
                break;
            }
        }
        best.map(|(_, id)| id)
    }

    /// The way from `from` to `to`: each place to pass and how to get to it (A*).
    pub fn path(&self, from: V3, to: V3) -> Option<Vec<(V3, Move)>> {
        let s = self.nearest(from)?;
        let g = self.nearest(to)?;
        let goal = self.nodes[g as usize];
        #[derive(PartialEq)]
        struct Open(f32, u32);
        impl Eq for Open {}
        impl PartialOrd for Open {
            fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
                Some(self.cmp(o))
            }
        }
        impl Ord for Open {
            fn cmp(&self, o: &Self) -> Ordering {
                o.0.total_cmp(&self.0)
            }
        }
        let n = self.nodes.len();
        let mut cost = vec![f32::INFINITY; n];
        let mut came: Vec<Option<(u32, Move)>> = vec![None; n];
        let mut open = BinaryHeap::new();
        cost[s as usize] = 0.0;
        open.push(Open((self.nodes[s as usize] - goal).len(), s));
        while let Some(Open(_, a)) = open.pop() {
            if a == g {
                let mut out = Vec::new();
                let mut cur = g;
                while let Some((prev, how)) = came[cur as usize] {
                    out.push((self.nodes[cur as usize], how));
                    cur = prev;
                }
                out.reverse();
                return Some(out);
            }
            let ca = cost[a as usize];
            for l in &self.links[a as usize] {
                let c = ca + l.cost;
                if c < cost[l.to as usize] {
                    cost[l.to as usize] = c;
                    came[l.to as usize] = Some((a, l.how));
                    open.push(Open(c + (self.nodes[l.to as usize] - goal).len(), l.to));
                }
            }
        }
        None
    }

    /// Every node reachable from `start`.
    #[cfg(test)]
    pub fn reachable(&self, start: u32) -> Vec<bool> {
        let mut seen = vec![false; self.nodes.len()];
        let mut stack = vec![start];
        seen[start as usize] = true;
        while let Some(a) = stack.pop() {
            for l in &self.links[a as usize] {
                if !seen[l.to as usize] {
                    seen[l.to as usize] = true;
                    stack.push(l.to);
                }
            }
        }
        seen
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::world::{MapId, World};

    /// Places on Varrow Bridgeworks that matter: every lane, every level, both bases.
    pub const BRIDGEWORKS_SPOTS: [(&str, [f32; 3]); 24] = [
        ("riverside quay", [-24.5, 0.0, -10.5]),
        ("footbridge", [-24.5, 0.0, 0.5]),
        ("blue quay", [-24.5, 0.0, 10.5]),
        ("stone bridge", [4.5, 0.0, 0.5]),
        ("red plaza", [-4.5, 0.0, -12.5]),
        ("blue plaza", [4.5, 0.0, 10.5]),
        ("workshop corridor", [21.5, 0.0, -7.5]),
        ("covered bridge", [22.5, 0.0, -2.5]),
        ("workshop hall", [17.5, 0.0, -11.5]),
        ("wall-run passage", [28.5, 0.0, -8.5]),
        ("red rampart", [-15.5, 3.5, -18.5]),
        ("blue rampart", [15.5, 3.5, 18.5]),
        ("warehouse balcony", [-18.5, 3.5, -8.5]),
        ("red warehouse roof", [-14.5, 7.0, -9.5]),
        ("blue warehouse roof", [-14.5, 7.0, 9.5]),
        ("tower top", [-10.5, 7.0, -5.5]),
        ("east platform", [11.5, 3.5, -6.5]),
        ("workshop roof", [22.5, 3.9, -10.5]),
        ("blue workshop roof", [22.5, 3.9, 10.5]),
        ("barracks roof", [-15.5, 3.9, -26.5]),
        ("riverbed", [-12.5, -3.0, 0.5]),
        ("under the stone bridge", [0.5, -3.0, 2.5]),
        ("red flag room", [0.5, 0.0, -27.5]),
        ("blue flag room", [0.5, 0.0, 27.5]),
    ];

    /// Every lane and level of the team map can be reached from both teams' spawns, and every
    /// place leads back (no dead end you can't climb out of); nearly the whole grid is one piece.
    #[test]
    fn bridgeworks_every_lane_and_level_is_reachable() {
        let w = World::build(MapId::Bridgeworks);
        let t0 = std::time::Instant::now();
        let nav = Nav::build(&w);
        println!("navigation: {} places, {} links, built in {:?}", nav.nodes.len(), nav.links.iter().map(|l| l.len()).sum::<usize>(), t0.elapsed());
        let n = w.spawn_points.len();
        for (team, spawn) in [("red", w.spawn_points[0].0), ("blue", w.spawn_points[n / 2].0)] {
            let s = nav.nearest(spawn).expect("a spawn on the grid");
            let seen = nav.reachable(s);
            let share = seen.iter().filter(|&&b| b).count() as f32 / seen.len() as f32;
            println!("{team}: {:.1}% of the places reachable", share * 100.0);
            assert!(share > 0.9, "{team}: only {:.0}% reachable", share * 100.0);
            for (name, p) in BRIDGEWORKS_SPOTS {
                let p = v3(p[0], p[1], p[2]);
                let id = nav.nearest(p + v3(0.0, 0.2, 0.0)).unwrap_or_else(|| panic!("{name}: on the grid"));
                assert!((nav.nodes[id as usize] - p).len() < 1.3, "{name}: the grid is there ({:?})", nav.nodes[id as usize]);
                assert!(seen[id as usize], "{team} can reach the {name}");
                assert!(nav.path(p, spawn).is_some(), "the {name} leads back to {team}'s spawn");
            }
        }
        // the dash gap over the covered bridge and the warehouse climb are part of it
        assert!(nav.links.iter().flatten().any(|l| l.how == Move::Dash), "a dash gap");
        assert!(nav.links.iter().flatten().any(|l| l.how == Move::Jump), "jumps");
    }

    /// No spawn point on the team map can be seen from anywhere a fighter can stand outside its
    /// own base (the lanes, the middle, the river, the enemy's base): not the head, not the chest.
    #[test]
    fn bridgeworks_spawns_are_out_of_enemy_sight() {
        let w = World::build(MapId::Bridgeworks);
        let nav = Nav::build(&w);
        let mut checked = 0;
        let mut seen: Vec<String> = Vec::new();
        for (sp, _) in &w.spawn_points {
            let red = sp.z < 0.0;
            for node in &nav.nodes {
                let outside = if red { node.z > -17.0 } else { node.z < 17.0 };
                if !outside {
                    continue;
                }
                let from = *node + v3(0.0, 1.6, 0.0);
                for h in [1.6f32, 1.1] {
                    checked += 1;
                    if w.line_clear(from, *sp + v3(0.0, h, 0.0)) {
                        seen.push(format!("spawn ({}, {}) from ({}, {}, {})", sp.x, sp.z, node.x, node.y, node.z));
                    }
                }
            }
        }
        println!("{checked} sight lines checked");
        seen.dedup();
        assert!(seen.is_empty(), "{} spawns seen: {:?}", seen.len(), &seen[..seen.len().min(20)]);
    }

    /// The other maps get navigation too: each spawn reaches the other side's.
    #[test]
    fn every_map_has_a_way_between_the_spawns() {
        for map in MapId::ALL {
            let w = World::build(map);
            let nav = for_world(&w);
            let n = w.spawn_points.len();
            for k in 0..n / 2 {
                assert!(nav.path(w.spawn_points[k].0, w.spawn_points[k + n / 2].0).is_some(), "{map:?}: spawn {k} to the other side");
            }
        }
    }
}
