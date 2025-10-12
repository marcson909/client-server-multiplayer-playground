use serde::{Deserialize, Serialize};

use crate::{tile_system::TilePosition, PlayerId};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum GameAction {
    Move { path: Vec<TilePosition> },
    Attack { target: PlayerId },
    UseItem { item_id: u32 },
    Interact { entity_id: u64 },
    ChopTree { tree_entity_id: u64 },
}
