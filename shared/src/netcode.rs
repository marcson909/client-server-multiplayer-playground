use serde::{Deserialize, Serialize};

use crate::{
    actions::GameAction, inventory::Inventory, items::ItemType, skills::SkillType,
    tile_system::TilePosition, trees::Tree, PlayerId,
};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ClientMessage {
    Join {
        name: String,
    },
    QueueAction {
        action: GameAction,
    },
    CancelAction,
    RequestPath {
        start: TilePosition,
        goal: TilePosition,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ServerMessage {
    Welcome {
        player_id: PlayerId,
        spawn_position: TilePosition,
    },
    DeltaUpdate {
        tick: u64,
        deltas: Vec<EntityDelta>,
    },
    EntitiesEntered {
        entities: Vec<EntitySnapshot>,
    },
    EntitiesLeft {
        entity_ids: Vec<u64>,
    },
    ActionQueued {
        action: GameAction,
    },
    ActionCompleted {
        entity_id: u64,
    },
    PathFound {
        path: Vec<TilePosition>,
    },
    PathNotFound,
    ObstacleData {
        obstacles: Vec<TilePosition>,
    },
    InventoryUpdate {
        inventory: Inventory,
    },
    ItemAdded {
        item_type: ItemType,
        quantity: u32,
    },
    ItemRemoved {
        item_type: ItemType,
        quantity: u32,
    },
    SkillUpdate {
        skill: SkillType,
        level: u32,
        experience: u32,
    },
    LevelUp {
        skill: SkillType,
        new_level: u32,
    },
    ExperienceGained {
        skill: SkillType,
        amount: u32,
    },
    TreeChopped {
        tree_entity_id: u64,
    },
    TreeRespawned {
        tree_entity_id: u64,
    },
    NotEnoughLevel {
        skill: SkillType,
        required: u32,
        current: u32,
    },
    NoAxeEquipped,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EntitySnapshot {
    pub entity_id: u64,
    pub tile_position: TilePosition,
    pub player_id: Option<PlayerId>,
    pub tree: Option<Tree>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EntityDelta {
    pub entity_id: u64,
    pub delta_type: DeltaType,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum DeltaType {
    FullState {
        tile_pos: TilePosition,
        player_id: Option<PlayerId>,
    },
    PositionOnly {
        tile_pos: TilePosition,
    },
    ActionStarted {
        action: GameAction,
    },
    Removed,
}
