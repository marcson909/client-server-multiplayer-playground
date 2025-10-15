use serde::{Deserialize, Serialize};

pub mod actions;
pub mod inventory;
pub mod items;
pub mod messages;
pub mod pathfinding;
pub mod skills;
pub mod tile_system;
pub mod trees;

pub const TILE_SIZE: f32 = 32.0;
pub const PROTOCOL_ID: u64 = 7;
pub const SERVER_PORT: u16 = 5000;
pub const TICK_RATE: f32 = 0.6; // 600ms per tick
pub const VIEW_DISTANCE: i32 = 5;
pub const INTERPOLATION_DELAY: f32 = 0.1;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PlayerId(pub u64);
