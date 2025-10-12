use bevy::prelude::*;
use bevy::utils::tracing::{debug, info, warn};
use bevy_renet::renet::*;
use shared::actions::GameAction;

use shared::items::ItemDefinition;
use shared::netcode::{ClientMessage, DeltaType, EntitySnapshot, ServerMessage};

use shared::skills::SkillData;
use shared::tile_system::TilePosition;
use shared::trees::{TreeDefinition, TreeType};
use shared::*;

use crate::{ClientEntity, ClientState, LocalPlayer, NetworkedEntity};

pub fn client_update_system(
    mut client: ResMut<RenetClient>,
    mut client_state: ResMut<ClientState>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    camera_q: Query<(&Camera, &GlobalTransform)>,
    mut commands: Commands,
) {
    if client.is_connected() && !client_state.join_sent && client_state.my_player_id.is_none() {
        info!("  - Connected to server!");
        let msg = ClientMessage::Join {
            name: "Player".to_string(),
        };
        if let Ok(msg_bytes) = bincode::serialize(&msg) {
            client.send_message(DefaultChannel::ReliableOrdered, msg_bytes);
            client_state.join_sent = true; // Mark as sent to prevent duplicate
            info!("  - Sent join request to server");
        }
    }

    if client_state.my_player_id.is_some() {
        handle_tile_movement_input(&keyboard, &mut client, &mut client_state);
    }

    if let Ok(window) = windows.get_single() {
        if let Ok((camera, camera_transform)) = camera_q.get_single() {
            handle_mouse_pathfinding(
                &mouse,
                window,
                camera,
                camera_transform,
                &mut client,
                &mut client_state,
            );
        }
    }

    while let Some(message) = client.receive_message(DefaultChannel::ReliableOrdered) {
        debug!("  - Received reliable message: {} bytes", message.len());
        if let Ok(server_msg) = bincode::deserialize::<ServerMessage>(&message) {
            handle_server_message_reliable(server_msg, &mut client_state, &mut commands);
        }
    }

    while let Some(message) = client.receive_message(DefaultChannel::Unreliable) {
        debug!("  - Received unreliable message: {} bytes", message.len());
        if let Ok(server_msg) = bincode::deserialize::<ServerMessage>(&message) {
            handle_server_message_unreliable(server_msg, &mut client_state);
        }
    }
}

pub fn handle_tile_movement_input(
    keyboard: &ButtonInput<KeyCode>,
    client: &mut RenetClient,
    state: &mut ClientState,
) {
    let my_entity_id = match state.my_entity_id {
        Some(id) => id,
        None => return,
    };

    let my_pos = match state.visible_entities.get(&my_entity_id) {
        Some(e) => e.tile_position,
        None => return,
    };

    let mut target_pos = None;
    let mut direction = "";

    if keyboard.just_pressed(KeyCode::KeyW) {
        target_pos = Some(TilePosition {
            x: my_pos.x,
            y: my_pos.y + 1,
        });
        direction = "North";
    } else if keyboard.just_pressed(KeyCode::KeyS) {
        target_pos = Some(TilePosition {
            x: my_pos.x,
            y: my_pos.y - 1,
        });
        direction = "South";
    } else if keyboard.just_pressed(KeyCode::KeyA) {
        target_pos = Some(TilePosition {
            x: my_pos.x - 1,
            y: my_pos.y,
        });
        direction = "West";
    } else if keyboard.just_pressed(KeyCode::KeyD) {
        target_pos = Some(TilePosition {
            x: my_pos.x + 1,
            y: my_pos.y,
        });
        direction = "East";
    }

    if let Some(pos) = target_pos {
        if !state.pathfinder.is_walkable(&pos) {
            warn!("  -  Cannot walk {} to {:?} - blocked!", direction, pos);
            return;
        }

        info!("  - Moving {} from {:?} to {:?}", direction, my_pos, pos);
        let action = GameAction::Move { path: vec![pos] };
        let msg = ClientMessage::QueueAction { action };
        let msg_bytes = bincode::serialize(&msg).unwrap();
        client.send_message(DefaultChannel::ReliableOrdered, msg_bytes);

        state.path_preview = None;
        state.confirmed_path = None;
    }
}

pub fn handle_mouse_pathfinding(
    mouse: &ButtonInput<MouseButton>,
    window: &Window,
    camera: &Camera,
    camera_transform: &GlobalTransform,
    client: &mut RenetClient,
    state: &mut ClientState,
) {
    let cursor_pos = window
        .cursor_position()
        .and_then(|cursor| camera.viewport_to_world(camera_transform, cursor))
        .map(|ray| ray.origin.truncate());

    if let Some(world_pos) = cursor_pos {
        let target_tile = TilePosition::from_world(world_pos);

        state.hover_entity = None;
        for (entity_id, entity) in &state.visible_entities {
            if entity.tile_position == target_tile && entity.tree.is_some() {
                state.hover_entity = Some(*entity_id);
                break;
            }
        }

        if mouse.just_pressed(MouseButton::Left) {
            if let Some(hover_entity_id) = state.hover_entity {
                if let Some(entity) = state.visible_entities.get(&hover_entity_id) {
                    if let Some(ref tree) = entity.tree {
                        if !tree.is_chopped {
                            let tree_def = TreeDefinition::get(tree.tree_type);
                            info!(
                                "  - Click: Attempting to chop {:?} at {:?}",
                                tree.tree_type, entity.tile_position
                            );
                            info!(
                                "  - Required level: {}, XP: {}",
                                tree_def.level_required, tree_def.experience
                            );

                            let action = GameAction::ChopTree {
                                tree_entity_id: hover_entity_id,
                            };
                            let msg = ClientMessage::QueueAction { action };
                            let msg_bytes = bincode::serialize(&msg).unwrap();
                            client.send_message(DefaultChannel::ReliableOrdered, msg_bytes);
                            return;
                        } else {
                            debug!("Tree already chopped, waiting for respawn");
                        }
                    }
                }
            }

            if let Some(my_entity_id) = state.my_entity_id {
                if let Some(my_entity) = state.visible_entities.get(&my_entity_id) {
                    info!(
                        "  -  Click: Requesting path from {:?} to {:?}",
                        my_entity.tile_position, target_tile
                    );
                    let msg = ClientMessage::RequestPath {
                        start: my_entity.tile_position,
                        goal: target_tile,
                    };
                    let msg_bytes = bincode::serialize(&msg).unwrap();
                    client.send_message(DefaultChannel::ReliableOrdered, msg_bytes);
                }
            }
        } else {
            if state.hover_entity.is_none() {
                if let Some(my_entity_id) = state.my_entity_id {
                    if let Some(my_entity) = state.visible_entities.get(&my_entity_id) {
                        state.path_preview = state
                            .pathfinder
                            .find_path(my_entity.tile_position, target_tile);
                    }
                }
            } else {
                state.path_preview = None;
            }
        }
    } else {
        state.path_preview = None;
        state.hover_entity = None;
    }
}

pub fn handle_pathfinding_update(
    mouse: &ButtonInput<MouseButton>,
    window: &Window,
    camera: &Camera,
    camera_transform: &GlobalTransform,
    client: &mut RenetClient,
    state: &mut ClientState,
) {
    let cursor_pos = window
        .cursor_position()
        .and_then(|cursor| camera.viewport_to_world(camera_transform, cursor))
        .map(|ray| ray.origin.truncate());

    if let Some(world_pos) = cursor_pos {
        let target_tile = TilePosition::from_world(world_pos);

        state.hover_entity = None;
        for (entity_id, entity) in &state.visible_entities {
            if entity.tile_position == target_tile && entity.tree.is_some() {
                state.hover_entity = Some(*entity_id);
                break;
            }
        }

        if mouse.just_pressed(MouseButton::Left) {
            if let Some(hover_entity_id) = state.hover_entity {
                if let Some(entity) = state.visible_entities.get(&hover_entity_id) {
                    if let Some(ref tree) = entity.tree {
                        if !tree.is_chopped {
                            let tree_def = TreeDefinition::get(tree.tree_type);
                            info!(
                                "  - Click: Attempting to chop {:?} at {:?}",
                                tree.tree_type, entity.tile_position
                            );
                            info!(
                                "  - Required level: {}, XP: {}",
                                tree_def.level_required, tree_def.experience
                            );

                            let action = GameAction::ChopTree {
                                tree_entity_id: hover_entity_id,
                            };
                            let msg = ClientMessage::QueueAction { action };
                            let msg_bytes = bincode::serialize(&msg).unwrap();
                            client.send_message(DefaultChannel::ReliableOrdered, msg_bytes);
                            return;
                        } else {
                            debug!("Tree already chopped, waiting for respawn");
                        }
                    }
                }
            }

            if let Some(my_entity_id) = state.my_entity_id {
                if let Some(my_entity) = state.visible_entities.get(&my_entity_id) {
                    info!(
                        "  -  Click: Requesting path from {:?} to {:?}",
                        my_entity.tile_position, target_tile
                    );
                    let msg = ClientMessage::RequestPath {
                        start: my_entity.tile_position,
                        goal: target_tile,
                    };
                    let msg_bytes = bincode::serialize(&msg).unwrap();
                    client.send_message(DefaultChannel::ReliableOrdered, msg_bytes);
                }
            }
        } else {
            if state.hover_entity.is_none() {
                if let Some(my_entity_id) = state.my_entity_id {
                    if let Some(my_entity) = state.visible_entities.get(&my_entity_id) {
                        state.path_preview = state
                            .pathfinder
                            .find_path(my_entity.tile_position, target_tile);
                    }
                }
            } else {
                state.path_preview = None;
            }
        }
    } else {
        state.path_preview = None;
        state.hover_entity = None;
    }
}

pub fn handle_server_message_reliable(
    msg: ServerMessage,
    state: &mut ClientState,
    commands: &mut Commands,
) {
    match msg {
        ServerMessage::Welcome {
            player_id,
            spawn_position: spawn_pos,
        } => {
            state.my_player_id = Some(player_id);
            info!("  - Welcome! Assigned player ID: {:?}", player_id);
            info!("  - Spawn position: {:?}", spawn_pos);
        }

        ServerMessage::EntitiesEntered { entities } => {
            info!("  -  {} entities entered view", entities.len());
            for snapshot in entities {
                if snapshot.tree.is_some() {
                    debug!(
                        "  - Tree entity {} at {:?}",
                        snapshot.entity_id, snapshot.tile_position
                    );
                } else if snapshot.player_id.is_some() {
                    info!(
                        "  - Player entity {} at {:?}",
                        snapshot.entity_id, snapshot.tile_position
                    );
                }
                spawn_client_entity(snapshot, state, commands);
            }
        }

        ServerMessage::EntitiesLeft { entity_ids } => {
            info!("  - {} entities left view", entity_ids.len());
            for entity_id in entity_ids {
                if let Some(client_entity) = state.visible_entities.remove(&entity_id) {
                    commands.entity(client_entity.entity).despawn();
                    debug!("  -  Despawned entity {}", entity_id);
                }
            }
        }

        ServerMessage::ActionQueued { action } => {
            info!("  - Action queued: {:?}", action);
        }

        ServerMessage::ActionCompleted { entity_id } => {
            debug!("  - Action completed for entity {}", entity_id);
        }

        ServerMessage::PathFound { path } => {
            info!("  -  Path found with {} tiles", path.len());
            state.confirmed_path = Some(path);
        }

        ServerMessage::PathNotFound => {
            warn!("  -  No path found to target!");
            state.confirmed_path = None;
        }

        ServerMessage::ObstacleData { obstacles } => {
            state.pathfinder.obstacles = obstacles.into_iter().collect();
            info!(
                "  -  Received {} obstacles from server",
                state.pathfinder.obstacles.len()
            );
        }

        ServerMessage::InventoryUpdate { inventory } => {
            state.inventory = inventory;
            debug!("  - Inventory updated");
        }

        ServerMessage::ItemAdded {
            item_type,
            quantity,
        } => {
            let def = ItemDefinition::get(item_type);
            let total = state.inventory.count_item(item_type);
            info!("  - Received {} x{} (total: {})", def.name, quantity, total);
        }

        ServerMessage::ItemRemoved {
            item_type,
            quantity,
        } => {
            let def = ItemDefinition::get(item_type);
            info!("  -  Removed {} x{}", def.name, quantity);
        }

        ServerMessage::SkillUpdate {
            skill,
            level,
            experience,
        } => {
            state.skills.insert(skill, SkillData { level, experience });
            debug!("  - {:?}: Level {} (XP: {})", skill, level, experience);
        }

        ServerMessage::LevelUp { skill, new_level } => {
            info!("  - LEVEL UP! {:?} is now level {}!", skill, new_level);
        }

        ServerMessage::ExperienceGained { skill, amount } => {
            if let Some(skill_data) = state.skills.get(&skill) {
                info!(
                    "  - {} {:?} XP (total: {})",
                    amount, skill, skill_data.experience
                );
            }
        }

        ServerMessage::TreeChopped { tree_entity_id } => {
            if let Some(entity) = state.visible_entities.get_mut(&tree_entity_id) {
                if let Some(ref mut tree) = entity.tree {
                    tree.is_chopped = true;
                    info!("  - Tree {} chopped!", tree_entity_id);
                }
            }
        }

        ServerMessage::TreeRespawned { tree_entity_id } => {
            if let Some(entity) = state.visible_entities.get_mut(&tree_entity_id) {
                if let Some(ref mut tree) = entity.tree {
                    tree.is_chopped = false;
                    info!("  - Tree {} respawned!", tree_entity_id);
                }
            }
        }

        ServerMessage::NotEnoughLevel {
            skill,
            required,
            current,
        } => {
            warn!(
                "  - Need level {} {:?} (current: {})",
                required, skill, current
            );
        }

        ServerMessage::NoAxeEquipped => {
            warn!("  -  You need an axe to chop this tree!");
        }

        _ => {}
    }
}

pub fn handle_server_message_unreliable(msg: ServerMessage, state: &mut ClientState) {
    if let ServerMessage::DeltaUpdate { tick: _, deltas } = msg {
        for delta in deltas {
            match delta.delta_type {
                DeltaType::FullState {
                    tile_pos,
                    player_id,
                } => {
                    if let Some(entity) = state.visible_entities.get_mut(&delta.entity_id) {
                        entity.tile_position = tile_pos;
                        entity.player_id = player_id;

                        if player_id == state.my_player_id {
                            state.my_entity_id = Some(delta.entity_id);
                            state.pending_move = None;
                        }
                    }
                }
                DeltaType::PositionOnly { tile_pos } => {
                    if let Some(entity) = state.visible_entities.get_mut(&delta.entity_id) {
                        entity.tile_position = tile_pos;

                        if Some(delta.entity_id) == state.my_entity_id {
                            state.pending_move = None;

                            if let Some(ref path) = state.confirmed_path {
                                if let Some(last_tile) = path.last() {
                                    if *last_tile == tile_pos {
                                        state.confirmed_path = None;
                                    }
                                }
                            }
                        }
                    }
                }

                DeltaType::ActionStarted { action: _ } => {}

                DeltaType::Removed => {
                    state.visible_entities.remove(&delta.entity_id);
                }
            }
        }
    }
}

pub fn spawn_client_entity(
    snapshot: EntitySnapshot,
    state: &mut ClientState,
    commands: &mut Commands,
) {
    let is_local = snapshot.player_id == state.my_player_id;

    let (color, size) = if let Some(ref tree) = snapshot.tree {
        let tree_color = match tree.tree_type {
            TreeType::Normal => Color::srgb(0.4, 0.6, 0.3),
            TreeType::Oak => Color::srgb(0.5, 0.4, 0.2),
            TreeType::Willow => Color::srgb(0.6, 0.7, 0.4),
        };
        let tree_color = if tree.is_chopped {
            Color::srgb(0.3, 0.3, 0.3)
        } else {
            tree_color
        };
        (tree_color, Vec2::new(TILE_SIZE * 1.2, TILE_SIZE * 1.5))
    } else if is_local {
        (
            Color::srgb(0.25, 0.75, 0.25),
            Vec2::new(TILE_SIZE * 0.8, TILE_SIZE * 0.8),
        )
    } else {
        (
            Color::srgb(0.75, 0.25, 0.25),
            Vec2::new(TILE_SIZE * 0.8, TILE_SIZE * 0.8),
        )
    };

    let mut entity_commands = commands.spawn((
        SpriteBundle {
            sprite: Sprite {
                color,
                custom_size: Some(size),
                ..default()
            },
            transform: Transform::from_translation(snapshot.tile_position.to_world().extend(0.0)),
            ..default()
        },
        snapshot.tile_position,
        NetworkedEntity {
            entity_id: snapshot.entity_id,
        },
    ));

    if is_local {
        entity_commands.insert(LocalPlayer);
        state.my_entity_id = Some(snapshot.entity_id);
        info!(
            "  - Spawned local player entity at {:?}",
            snapshot.tile_position
        );
    } else if snapshot.tree.is_some() {
        debug!(
            "  - Spawned tree entity {} at {:?}",
            snapshot.entity_id, snapshot.tile_position
        );
    } else {
        info!(
            "  - Spawned remote player entity {} at {:?}",
            snapshot.entity_id, snapshot.tile_position
        );
    }

    let entity = entity_commands.id();

    state.visible_entities.insert(
        snapshot.entity_id,
        ClientEntity {
            tile_position: snapshot.tile_position,
            player_id: snapshot.player_id,
            entity,
            tree: snapshot.tree,
        },
    );
}

pub fn update_confirmed_path(mut client_state: ResMut<ClientState>) {
    if let Some(my_entity_id) = client_state.my_entity_id {
        let current_position = client_state
            .visible_entities
            .get(&my_entity_id)
            .map(|entity| entity.tile_position);
        if let Some(current_pos) = current_position {
            if let Some(ref mut path) = client_state.confirmed_path {
                // remove all tiles from the path that we've already passed
                path.retain(|tile| *tile != current_pos);

                // also remove any tiles that are no longer connected to our current position
                // this handles cases where the player might have deviated from the path
                if let Some(first_tile_index) = path.iter().position(|tile| {
                    // check if this tile is adjacent to our current position
                    let dx = (tile.x - current_pos.x).abs();
                    let dy = (tile.y - current_pos.y).abs();
                    (dx <= 1 && dy == 0) || (dx == 0 && dy <= 1)
                }) {
                    // keep only tiles from the first adjacent tile onward
                    *path = path[first_tile_index..].to_vec();
                } else {
                    // if no tiles are adjacent, clear the path
                    path.clear();
                }
                if path.is_empty() {
                    client_state.confirmed_path = None;
                }
            }
        }
    }
}
