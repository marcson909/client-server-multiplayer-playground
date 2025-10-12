use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::items::ItemType;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum TreeType {
    Normal,
    Oak,
    Willow,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TreeDefinition {
    pub tree_type: TreeType,
    pub name: &'static str,
    pub level_required: u32,
    pub chop_time: f64,
    pub logs_given: ItemType,
    pub experience: u32,
    pub respawn_time: f64,
}

impl TreeDefinition {
    pub fn get(tree_type: TreeType) -> Self {
        match tree_type {
            TreeType::Normal => TreeDefinition {
                tree_type,
                name: "Tree",
                level_required: 1,
                chop_time: 3.0,
                logs_given: ItemType::Logs,
                experience: 25,
                respawn_time: 5.0,
            },
            TreeType::Oak => TreeDefinition {
                tree_type,
                name: "Oak",
                level_required: 15,
                chop_time: 5.0,
                logs_given: ItemType::OakLogs,
                experience: 37,
                respawn_time: 8.0,
            },
            TreeType::Willow => TreeDefinition {
                tree_type,
                name: "Willow",
                level_required: 30,
                chop_time: 4.0,
                logs_given: ItemType::WillowLogs,
                experience: 67,
                respawn_time: 10.0,
            },
        }
    }
}

#[derive(Component, Serialize, Deserialize, Clone, Debug)]
pub struct Tree {
    pub tree_type: TreeType,
    pub is_chopped: bool,
    pub respawn_timer: f64,
}

impl Tree {
    pub fn new(tree_type: TreeType) -> Self {
        Self {
            tree_type,
            is_chopped: false,
            respawn_timer: 0.0,
        }
    }
}
