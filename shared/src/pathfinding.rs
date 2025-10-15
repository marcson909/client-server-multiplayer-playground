use std::cmp::Ordering;
use std::collections::BinaryHeap;

use bevy::utils::{HashMap, HashSet};

use crate::tile_system::TilePosition;

#[derive(Clone, Eq, PartialEq)]
pub struct PathNode {
    pub position: TilePosition,
    pub g_cost: i32,
    pub h_cost: i32,
    pub f_cost: i32,
}

impl Ord for PathNode {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .f_cost
            .cmp(&self.f_cost)
            .then_with(|| other.h_cost.cmp(&self.h_cost))
    }
}

impl PartialOrd for PathNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

pub struct Pathfinder {
    pub obstacles: HashSet<TilePosition>,
    pub allow_diagonal: bool,
}

impl Pathfinder {
    pub fn new(allow_diagonal: bool) -> Self {
        Self {
            obstacles: HashSet::new(),
            allow_diagonal,
        }
    }

    pub fn add_obstacle(&mut self, pos: TilePosition) {
        self.obstacles.insert(pos);
    }

    pub fn remove_obstacle(&mut self, pos: TilePosition) {
        self.obstacles.remove(&pos);
    }

    pub fn is_walkable(&self, pos: &TilePosition) -> bool {
        !self.obstacles.contains(pos)
    }

    pub fn find_path_a_star(
        &self,
        start: TilePosition,
        goal: TilePosition,
    ) -> Option<Vec<TilePosition>> {
        if start == goal {
            return Some(vec![goal]);
        }

        if !self.is_walkable(&goal) {
            return None;
        }

        let mut open_set = BinaryHeap::new();
        let mut came_from: HashMap<TilePosition, TilePosition> = HashMap::new();
        let mut g_score: HashMap<TilePosition, i32> = HashMap::new();

        g_score.insert(start, 0);
        open_set.push(PathNode {
            position: start,
            g_cost: 0,
            h_cost: Self::heuristic(&start, &goal),
            f_cost: Self::heuristic(&start, &goal),
        });

        while let Some(current_node) = open_set.pop() {
            let current = current_node.position;

            if current == goal {
                return Some(self.reconstruct_path(&came_from, current));
            }

            let neighbors = if self.allow_diagonal {
                current.neighbors_diagonal()
            } else {
                current.neighbors()
            };

            for neighbor in neighbors {
                if !self.is_walkable(&neighbor) {
                    continue;
                }

                let is_diagonal =
                    (current.x - neighbor.x).abs() + (current.y - neighbor.y).abs() == 2;
                let move_cost = if is_diagonal { 14 } else { 10 };

                let tentative_g_score = g_score.get(&current).unwrap_or(&i32::MAX) + move_cost;

                if tentative_g_score < *g_score.get(&neighbor).unwrap_or(&i32::MAX) {
                    came_from.insert(neighbor, current);
                    g_score.insert(neighbor, tentative_g_score);

                    let h_cost = Self::heuristic(&neighbor, &goal);
                    let f_cost = tentative_g_score + h_cost;

                    open_set.push(PathNode {
                        position: neighbor,
                        g_cost: tentative_g_score,
                        h_cost,
                        f_cost,
                    });
                }
            }
        }

        None
    }

    fn heuristic(a: &TilePosition, b: &TilePosition) -> i32 {
        ((a.x - b.x).abs() + (a.y - b.y).abs()) * 10
    }

    fn reconstruct_path(
        &self,
        came_from: &HashMap<TilePosition, TilePosition>,
        mut current: TilePosition,
    ) -> Vec<TilePosition> {
        let mut path = vec![current];
        while let Some(&prev) = came_from.get(&current) {
            current = prev;
            path.push(current);
        }
        path.reverse();
        path
    }
}
