use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::items::{ItemDefinition, ItemStack, ItemType};

#[derive(Serialize, Deserialize, Clone, Debug, Component)]
pub struct Inventory {
    pub slots: Vec<Option<ItemStack>>,
    pub max_slots: usize,
}

impl Inventory {
    pub fn new(max_slots: usize) -> Self {
        Self {
            slots: vec![None; max_slots],
            max_slots,
        }
    }

    pub fn add_item(&mut self, item_type: ItemType, quantity: u32) -> bool {
        let def = ItemDefinition::get(item_type);

        if def.stackable {
            for slot in &mut self.slots {
                if let Some(stack) = slot {
                    if stack.item_type == item_type {
                        stack.quantity += quantity;
                        return true;
                    }
                }
            }
        }

        for slot in &mut self.slots {
            if slot.is_none() {
                *slot = Some(ItemStack {
                    item_type,
                    quantity,
                });
                return true;
            }
        }

        false
    }

    pub fn remove_item(&mut self, item_type: ItemType, quantity: u32) -> bool {
        for slot in &mut self.slots {
            if let Some(stack) = slot {
                if stack.item_type == item_type && stack.quantity >= quantity {
                    stack.quantity -= quantity;
                    if stack.quantity == 0 {
                        *slot = None;
                    }
                    return true;
                }
            }
        }
        false
    }

    pub fn has_item(&self, item_type: ItemType, quantity: u32) -> bool {
        self.count_item(item_type) >= quantity
    }

    pub fn count_item(&self, item_type: ItemType) -> u32 {
        self.slots
            .iter()
            .filter_map(|slot| slot.as_ref())
            .filter(|stack| stack.item_type == item_type)
            .map(|stack| stack.quantity)
            .sum()
    }

    pub fn has_any_axe(&self) -> Option<ItemType> {
        let axes = [ItemType::SteelAxe, ItemType::IronAxe, ItemType::BronzeAxe];
        for axe in axes {
            if self.has_item(axe, 1) {
                return Some(axe);
            }
        }
        None
    }
}
