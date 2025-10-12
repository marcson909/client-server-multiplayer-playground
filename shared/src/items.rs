use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ItemType {
    BronzeAxe,
    IronAxe,
    SteelAxe,
    Logs,
    OakLogs,
    WillowLogs,
    Shrimp,
    Salmon,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ItemDefinition {
    pub item_type: ItemType,
    pub name: &'static str,
    pub stackable: bool,
    pub description: &'static str,
}

impl ItemDefinition {
    pub fn get(item_type: ItemType) -> Self {
        match item_type {
            ItemType::BronzeAxe => ItemDefinition {
                item_type,
                name: "Bronze axe",
                stackable: false,
                description: "A woodcutter's axe made of bronze.",
            },
            ItemType::IronAxe => ItemDefinition {
                item_type,
                name: "Iron axe",
                stackable: false,
                description: "A woodcutter's axe made of iron.",
            },
            ItemType::SteelAxe => ItemDefinition {
                item_type,
                name: "Steel axe",
                stackable: false,
                description: "A woodcutter's axe made of steel.",
            },
            ItemType::Logs => ItemDefinition {
                item_type,
                name: "Logs",
                stackable: true,
                description: "Logs cut from a tree.",
            },
            ItemType::OakLogs => ItemDefinition {
                item_type,
                name: "Oak logs",
                stackable: true,
                description: "Logs cut from an oak tree.",
            },
            ItemType::WillowLogs => ItemDefinition {
                item_type,
                name: "Willow logs",
                stackable: true,
                description: "Logs cut from a willow tree.",
            },
            ItemType::Shrimp => ItemDefinition {
                item_type,
                name: "Shrimp",
                stackable: true,
                description: "Some nicely cooked shrimp.",
            },
            ItemType::Salmon => ItemDefinition {
                item_type,
                name: "Salmon",
                stackable: true,
                description: "Some nicely cooked salmon.",
            },
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ItemStack {
    pub item_type: ItemType,
    pub quantity: u32,
}
