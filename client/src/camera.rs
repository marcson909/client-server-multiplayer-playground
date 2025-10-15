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
            // use interpolated position for remote entities if available
            let display_position = if Some(networked.entity_id) == client_state.my_entity_id {
                // for our own entity, use the predicted position
                entity.tile_position
            } else if let Some(interp_pos) = entity.interpolated_position {
                // for remote entities, use interpolated position
                interp_pos
            } else {
                // fallback to actual tile position
                entity.tile_position
            };

            let target = display_position.to_world().extend(0.0);
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
/// draw prediction and interpolation ghosts for debugging
pub fn draw_netcode_ghosts(mut gizmos: Gizmos, client_state: Res<ClientState>) {
    if client_state.show_prediction_ghosts {
        if let Some(my_entity_id) = client_state.my_entity_id {
            if let Some(my_entity) = client_state.visible_entities.get(&my_entity_id) {
                // draw server's authoritative position as a ghost
                let server_pos = my_entity.server_position.to_world();
                let ghost_size = TILE_SIZE * 0.8;

                // semi-transparent blue circle for server position
                gizmos.circle_2d(
                    server_pos,
                    ghost_size * 0.4,
                    Color::srgba(0.3, 0.5, 1.0, 0.5),
                );

                // line from server position to predicted position
                let predicted_pos = my_entity.tile_position.to_world();
                if server_pos != predicted_pos {
                    gizmos.line_2d(server_pos, predicted_pos, Color::srgba(1.0, 1.0, 0.0, 0.7));
                }

                // label
                gizmos.rect_2d(
                    server_pos + Vec2::new(0.0, TILE_SIZE * 0.6),
                    0.0,
                    Vec2::new(20.0, 4.0),
                    Color::srgba(0.3, 0.5, 1.0, 0.8),
                );
            }
        }
    }

    // draw interpolation ghosts and buffer endpoints for remote players
    if client_state.show_interpolation_ghosts {
        let my_entity_id = client_state.my_entity_id;

        for (entity_id, entity) in client_state.visible_entities.iter() {
            // skip local player and trees
            if Some(*entity_id) == my_entity_id || entity.tree.is_some() {
                continue;
            }

            // draw interpolation buffer positions
            if entity.position_buffer.len() >= 2 {
                let buffer = &entity.position_buffer;

                // draw first position (oldest)
                let pos0 = buffer[0].position.to_world();
                gizmos.circle_2d(pos0, TILE_SIZE * 0.3, Color::srgba(1.0, 0.5, 0.0, 0.4));

                // draw last position (newest)
                let pos1 = buffer[buffer.len() - 1].position.to_world();
                gizmos.circle_2d(pos1, TILE_SIZE * 0.3, Color::srgba(0.0, 1.0, 0.5, 0.4));

                // line between them
                gizmos.line_2d(pos0, pos1, Color::srgba(0.5, 0.5, 0.5, 0.3));

                // show interpolated position
                if let Some(interp_pos) = entity.interpolated_position {
                    let interp_world = interp_pos.to_world();
                    gizmos.circle_2d(
                        interp_world,
                        TILE_SIZE * 0.25,
                        Color::srgba(1.0, 0.0, 1.0, 0.6),
                    );
                }
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
