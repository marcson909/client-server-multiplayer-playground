use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::TILE_SIZE;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, Component, Default)]
pub struct TilePosition {
    pub x: i32,
    pub y: i32,
}

impl TilePosition {
    pub fn to_world(&self) -> Vec2 {
        Vec2::new(self.x as f32 * TILE_SIZE, self.y as f32 * TILE_SIZE)
    }

    pub fn from_world(pos: Vec2) -> Self {
        Self {
            x: (pos.x / TILE_SIZE).round() as i32,
            y: (pos.y / TILE_SIZE).round() as i32,
        }
    }

    pub fn distance_to(&self, other: &TilePosition) -> i32 {
        (self.x - other.x).abs() + (self.y - other.y).abs()
    }

    pub fn neighbors(&self) -> Vec<TilePosition> {
        vec![
            TilePosition {
                x: self.x + 1,
                y: self.y,
            },
            TilePosition {
                x: self.x - 1,
                y: self.y,
            },
            TilePosition {
                x: self.x,
                y: self.y + 1,
            },
            TilePosition {
                x: self.x,
                y: self.y - 1,
            },
        ]
    }

    pub fn neighbors_diagonal(&self) -> Vec<TilePosition> {
        vec![
            TilePosition {
                x: self.x + 1,
                y: self.y,
            },
            TilePosition {
                x: self.x - 1,
                y: self.y,
            },
            TilePosition {
                x: self.x,
                y: self.y + 1,
            },
            TilePosition {
                x: self.x,
                y: self.y - 1,
            },
            TilePosition {
                x: self.x + 1,
                y: self.y + 1,
            },
            TilePosition {
                x: self.x + 1,
                y: self.y - 1,
            },
            TilePosition {
                x: self.x - 1,
                y: self.y + 1,
            },
            TilePosition {
                x: self.x - 1,
                y: self.y - 1,
            },
        ]
    }
}
