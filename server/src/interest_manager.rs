use bevy::prelude::*;
use shared::tile_system::TilePosition;
use shared::*;
use std::collections::{HashMap, HashSet};

#[derive(Resource, Default)]
pub struct InterestManager {
    pub client_views: HashMap<PlayerId, HashSet<u64>>,
}

impl InterestManager {
    pub fn update_view(
        &mut self,
        player_id: PlayerId,
        center: TilePosition,
        entities: &HashMap<u64, TilePosition>,
    ) -> (Vec<u64>, Vec<u64>) {
        let view = self
            .client_views
            .entry(player_id)
            .or_insert_with(HashSet::new);
        let mut now_visible = HashSet::new();

        for (entity_id, pos) in entities {
            if center.distance_to(pos) <= VIEW_DISTANCE {
                now_visible.insert(*entity_id);
            }
        }

        let entered: Vec<u64> = now_visible.difference(view).copied().collect();
        let left: Vec<u64> = view.difference(&now_visible).copied().collect();

        *view = now_visible;
        (entered, left)
    }
}
