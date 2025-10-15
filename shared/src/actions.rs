use serde::{Deserialize, Serialize};

use crate::{tile_system::TilePosition, PlayerId, TICK_RATE};

/// Action priority levels
/// Strong > Normal > Weak
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ActionPriority {
    Weak = 0,    // gathering, repeating actions (cancelled by movement)
    Normal = 1,  // movement, combat, item use (can be replaced by same type)
    Strong = 2,  // teleports, damage, forced actions (cancels everything)
}

impl ActionPriority {
    /// returns true if this priority can cancel the other priority
    pub fn can_cancel(&self, other: &ActionPriority) -> bool {
        self > other
    }

    /// returns true if this priority should suspend (not cancel) the other
    /// Strong actions suspend Normal actions (can resume later)
    pub fn should_suspend(&self, other: &ActionPriority) -> bool {
        *self == ActionPriority::Strong && *other == ActionPriority::Normal
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum GameAction {
    Move { path: Vec<TilePosition> },
    Attack { target: PlayerId },
    UseItem { item_id: u32 },
    Interact { entity_id: u64 },
    ChopTree { tree_entity_id: u64 },
}

impl GameAction {
    pub fn priority(&self) -> ActionPriority {
        match self {
            GameAction::Move { .. } => ActionPriority::Normal,
            GameAction::Attack { .. } => ActionPriority::Normal,
            GameAction::UseItem { .. } => ActionPriority::Normal,
            GameAction::Interact { .. } => ActionPriority::Strong,
            GameAction::ChopTree { .. } => ActionPriority::Weak,
        }
    }

    /// get the base tick delay for this action in ticks
    pub fn tick_delay(&self) -> u32 {
        match self {
            GameAction::Move { .. } => 1,        // 1 tick per tile (0.6s)
            GameAction::Attack { .. } => 4,      // 4 ticks (2.4s) - typical weapon speed
            GameAction::UseItem { .. } => 1,     // 1 tick (0.6s) - eat/drink
            GameAction::Interact { .. } => 2,    // 2 ticks (1.2s) - interact delay
            GameAction::ChopTree { .. } => 4,    // 4 ticks (2.4s) - chop attempt
        }
    }

    pub fn duration_seconds(&self) -> f64 {
        self.tick_delay() as f64 * TICK_RATE as f64
    }

    pub fn replaces_same_type(&self, other: &GameAction) -> bool {
        match (self, other) {
            (GameAction::Move { .. }, GameAction::Move { .. }) => true,
            (GameAction::ChopTree { .. }, GameAction::ChopTree { .. }) => true,
            (GameAction::Attack { .. }, GameAction::Attack { .. }) => true,
            _ => false,
        }
    }

    /// repeating actions loop until cancelled or resource depleted
    pub fn is_repeating(&self) -> bool {
        matches!(self, GameAction::ChopTree { .. })
    }
}
