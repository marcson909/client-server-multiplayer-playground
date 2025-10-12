use bevy::prelude::*;

use shared::*;
use shared::{tile_system::TilePosition, trees::TreeType};

use crate::{ClientState, NetworkedEntity};

pub fn update_entity_positions(
    client_state: Res<ClientState>,
    mut query: Query<(&NetworkedEntity, &mut Transform)>,
) {
    for (networked, mut transform) in query.iter_mut() {
        if let Some(entity) = client_state.visible_entities.get(&networked.entity_id) {
            let target = entity.tile_position.to_world().extend(0.0);
            transform.translation = transform.translation.lerp(target, 0.2);
        }
    }
}

pub fn update_tree_visuals(
    client_state: Res<ClientState>,
    mut query: Query<(&NetworkedEntity, &mut Sprite)>,
) {
    for (networked, mut sprite) in query.iter_mut() {
        if let Some(entity) = client_state.visible_entities.get(&networked.entity_id) {
            if let Some(ref tree) = entity.tree {
                let tree_color = match tree.tree_type {
                    TreeType::Normal => Color::srgb(0.4, 0.6, 0.3),
                    TreeType::Oak => Color::srgb(0.5, 0.4, 0.2),
                    TreeType::Willow => Color::srgb(0.6, 0.7, 0.4),
                };

                sprite.color = if tree.is_chopped {
                    Color::srgb(0.3, 0.3, 0.3)
                } else {
                    tree_color
                };
            }
        }
    }
}
pub fn draw_tile_grid(mut gizmos: Gizmos, client_state: Res<ClientState>) {
    let grid_size = 20;
    let color = Color::srgba(1.0, 1.0, 1.0, 0.1);

    for x in -grid_size..=grid_size {
        let start = Vec2::new(x as f32 * TILE_SIZE, -grid_size as f32 * TILE_SIZE);
        let end = Vec2::new(x as f32 * TILE_SIZE, grid_size as f32 * TILE_SIZE);
        gizmos.line_2d(start, end, color);
    }

    for y in -grid_size..=grid_size {
        let start = Vec2::new(-grid_size as f32 * TILE_SIZE, y as f32 * TILE_SIZE);
        let end = Vec2::new(grid_size as f32 * TILE_SIZE, y as f32 * TILE_SIZE);
        gizmos.line_2d(start, end, color);
    }

    for obstacle in &client_state.pathfinder.obstacles {
        let position = obstacle.to_world();
        let size = TILE_SIZE * 0.9;
        gizmos.rect_2d(
            position,
            0.0,
            Vec2::new(size, size),
            Color::srgb(0.5, 0.3, 0.3),
        );
    }

    if let Some(hover_entity_id) = client_state.hover_entity {
        if let Some(entity) = client_state.visible_entities.get(&hover_entity_id) {
            if entity.tree.is_some() {
                let position = entity.tile_position.to_world();
                let size = TILE_SIZE * 1.3;
                gizmos.rect_2d(
                    position,
                    0.0,
                    Vec2::new(size, size),
                    Color::srgb(1.0, 1.0, 0.0),
                );
            }
        }
    }

    if let Some(ref path) = client_state.path_preview {
        draw_path(&mut gizmos, path, Color::srgba(0.5, 0.5, 1.0, 0.3), false);
    }

    if let Some(ref path) = client_state.confirmed_path {
        draw_path(&mut gizmos, path, Color::srgba(0.2, 1.0, 0.2, 0.6), true);
    }
}

pub fn draw_path(gizmos: &mut Gizmos, path: &[TilePosition], color: Color, draw_arrows: bool) {
    for tile in path {
        let position = tile.to_world();
        let size = TILE_SIZE * 0.6;
        gizmos.rect_2d(position, 0.0, Vec2::new(size, size), color);
    }

    if draw_arrows && path.len() > 1 {
        for window in path.windows(2) {
            let from = window[0].to_world();
            let to = window[1].to_world();

            gizmos.line_2d(from, to, color);

            let direction = (to - from).normalize();
            let perp = Vec2::new(-direction.y, direction.x);
            let arrow_size = 8.0;

            let arrow_tip = to - direction * (TILE_SIZE * 0.3);
            let arrow_left = arrow_tip - direction * arrow_size + perp * arrow_size * 0.5;
            let arrow_right = arrow_tip - direction * arrow_size - perp * arrow_size * 0.5;

            gizmos.line_2d(arrow_tip, arrow_left, color);
            gizmos.line_2d(arrow_tip, arrow_right, color);
        }
    }
}

pub fn camera_follow_player(
    client_state: Res<ClientState>,
    mut camera_q: Query<&mut Transform, With<Camera>>,
) {
    if let Some(my_entity_id) = client_state.my_entity_id {
        if let Some(my_entity) = client_state.visible_entities.get(&my_entity_id) {
            if let Ok(mut camera_transform) = camera_q.get_single_mut() {
                let target = my_entity
                    .tile_position
                    .to_world()
                    .extend(camera_transform.translation.z);
                camera_transform.translation = camera_transform.translation.lerp(target, 0.1);
            }
        }
    }
}
