use bevy::prelude::*;
use bevy::utils::tracing::{debug, info, warn};
use bevy_renet::renet::*;
use shared::actions::GameAction;

use shared::items::ItemDefinition;
use shared::messages::{ClientMessage, DeltaType, EntitySnapshot, ServerMessage};

use shared::skills::SkillData;
use shared::tile_system::TilePosition;
use shared::trees::{TreeDefinition, TreeType};
use shared::*;

use crate::{ClientEntity, ClientState, LocalPlayer, NetworkedEntity, PendingInput, PositionSnapshot};

pub fn client_update_system(
    mut client: ResMut<RenetClient>,
    mut client_state: ResMut<ClientState>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    camera_q: Query<(&Camera, &GlobalTransform)>,
    time: Res<Time>,
    mut commands: Commands,
) {
    if client.is_connected() && !client_state.join_sent && client_state.my_player_id.is_none() {
        info!("Connected to server!");
        let msg = ClientMessage::Join {
            name: "Player".to_string(),
        };
        if let Ok(msg_bytes) = bincode::serialize(&msg) {
            client.send_message(DefaultChannel::ReliableOrdered, msg_bytes);
            client_state.join_sent = true; // Mark as sent to prevent duplicate
            info!("Sent join request to server");
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
        debug!("Received reliable message: {} bytes", message.len());
        if let Ok(server_msg) = bincode::deserialize::<ServerMessage>(&message) {
            handle_server_message_reliable(server_msg, &mut client_state, &mut commands);
        }
    }

    while let Some(message) = client.receive_message(DefaultChannel::Unreliable) {
        debug!("Received unreliable message: {} bytes", message.len());
        if let Ok(server_msg) = bincode::deserialize::<ServerMessage>(&message) {
            handle_server_message_unreliable(server_msg, &mut client_state, &time);
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
            warn!(" Cannot walk {} to {:?} - blocked!", direction, pos);
            return;
        }

        info!("Moving {} from {:?} to {:?}", direction, my_pos, pos);

        // Server will automatically replace any in-progress move action
        let action = GameAction::Move { path: vec![pos] };
        let input_sequence_number = state.input_sequence_number;
        state.input_sequence_number += 1;
        let msg = ClientMessage::QueueAction {
            action: action.clone(),
            input_sequence_number,
        };
        let msg_bytes = bincode::serialize(&msg).unwrap();
        client.send_message(DefaultChannel::ReliableOrdered, msg_bytes);

        // client-side prediction: apply the input immediately
        if state.client_side_prediction {
            if let Some(my_entity) = state.visible_entities.get_mut(&my_entity_id) {
                apply_action_to_position(&action, &mut my_entity.tile_position);
                debug!("Predicted position: {:?}", my_entity.tile_position);
            }
        }

        // store this input for later reconciliation
        state.pending_inputs.push(PendingInput {
            input_sequence_number,
            action,
        });

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
                            let tree_pos = entity.tile_position;

                            info!(
                                "Click: Attempting to chop {:?} at {:?}",
                                tree.tree_type, tree_pos
                            );
                            info!(
                                "Required level: {}, XP: {}",
                                tree_def.level_required, tree_def.experience
                            );

                            // Cancel any current action first
                            let cancel_msg = ClientMessage::CancelAction;
                            let cancel_bytes = bincode::serialize(&cancel_msg).unwrap();
                            client.send_message(DefaultChannel::ReliableOrdered, cancel_bytes);

                            // Check if we need to move to the tree first
                            if let Some(my_entity_id) = state.my_entity_id {
                                if let Some(my_entity) = state.visible_entities.get(&my_entity_id) {
                                    let my_pos = my_entity.tile_position;

                                    // Check if we're adjacent to the tree (within 1 tile, including diagonals)
                                    let dx = (my_pos.x - tree_pos.x).abs();
                                    let dy = (my_pos.y - tree_pos.y).abs();
                                    let is_adjacent = dx <= 1 && dy <= 1 && !(dx == 0 && dy == 0);

                                    let input_sequence_number = state.input_sequence_number;
                                    state.input_sequence_number += 1;

                                    if is_adjacent {
                                        // We're adjacent, just chop
                                        info!("Adjacent to tree, chopping directly");

                                        let action = GameAction::ChopTree {
                                            tree_entity_id: hover_entity_id,
                                        };

                                        let msg = ClientMessage::QueueAction {
                                            action: action.clone(),
                                            input_sequence_number,
                                        };
                                        let msg_bytes = bincode::serialize(&msg).unwrap();
                                        client.send_message(DefaultChannel::ReliableOrdered, msg_bytes);

                                        state.pending_inputs.push(PendingInput {
                                            input_sequence_number,
                                            action,
                                        });
                                    } else {
                                        // Need to move to tree first, then chop
                                        info!("Not adjacent to tree, will move then chop");

                                        // Find an adjacent walkable tile
                                        let mut best_adjacent: Option<TilePosition> = None;
                                        let mut min_distance = i32::MAX;

                                        for dx in -1..=1 {
                                            for dy in -1..=1 {
                                                if dx == 0 && dy == 0 { continue; }

                                                let adjacent = TilePosition {
                                                    x: tree_pos.x + dx,
                                                    y: tree_pos.y + dy,
                                                };

                                                if state.pathfinder.is_walkable(&adjacent) {
                                                    let dist = (adjacent.x - my_pos.x).abs() + (adjacent.y - my_pos.y).abs();
                                                    if dist < min_distance {
                                                        min_distance = dist;
                                                        best_adjacent = Some(adjacent);
                                                    }
                                                }
                                            }
                                        }

                                        if let Some(move_to) = best_adjacent {
                                            // Find path to adjacent tile
                                            if let Some(path) = state.pathfinder.find_path_a_star(my_pos, move_to) {
                                                let move_action = GameAction::Move { path: path.clone() };
                                                let chop_action = GameAction::ChopTree {
                                                    tree_entity_id: hover_entity_id,
                                                };

                                                // Send both actions as a chain
                                                let msg = ClientMessage::QueueActions {
                                                    actions: vec![move_action.clone(), chop_action],
                                                    input_sequence_number,
                                                };
                                                let msg_bytes = bincode::serialize(&msg).unwrap();
                                                client.send_message(DefaultChannel::ReliableOrdered, msg_bytes);

                                                // For prediction, predict the movement
                                                if state.client_side_prediction {
                                                    if let Some(my_entity_mut) = state.visible_entities.get_mut(&my_entity_id) {
                                                        apply_action_to_position(&move_action, &mut my_entity_mut.tile_position);
                                                        debug!("Predicted move to: {:?}", my_entity_mut.tile_position);
                                                    }
                                                }

                                                state.pending_inputs.push(PendingInput {
                                                    input_sequence_number,
                                                    action: move_action,
                                                });

                                                state.confirmed_path = Some(path);

                                                info!("Queued: Move to {:?} then chop tree", move_to);
                                            } else {
                                                warn!("No path found to tree!");
                                            }
                                        } else {
                                            warn!("No walkable tiles adjacent to tree!");
                                        }
                                    }
                                }
                            }

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
                        "Click: Requesting path from {:?} to {:?}",
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
                            .find_path_a_star(my_entity.tile_position, target_tile);
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
            info!("Welcome! Assigned player ID: {:?}", player_id);
            info!("Spawn position: {:?}", spawn_pos);
        }

        ServerMessage::EntitiesEntered { entities } => {
            info!("{} entities entered view", entities.len());
            for snapshot in entities {
                if snapshot.tree.is_some() {
                    debug!(
                        "Tree entity {} at {:?}",
                        snapshot.entity_id, snapshot.tile_position
                    );
                } else if snapshot.player_id.is_some() {
                    info!(
                        "Player entity {} at {:?}",
                        snapshot.entity_id, snapshot.tile_position
                    );
                }
                spawn_client_entity(snapshot, state, commands);
            }
        }

        ServerMessage::EntitiesLeft { entity_ids } => {
            info!("{} entities left view", entity_ids.len());
            for entity_id in entity_ids {
                if let Some(client_entity) = state.visible_entities.remove(&entity_id) {
                    commands.entity(client_entity.entity).despawn();
                    debug!(" Despawned entity {}", entity_id);
                }
            }
        }

        ServerMessage::ActionQueued { action } => {
            info!("Action queued: {:?}", action);
        }

        ServerMessage::ActionCompleted { entity_id } => {
            debug!("Action completed for entity {}", entity_id);
        }

        ServerMessage::PathFound { path } => {
            info!("Path found with {} tiles", path.len());
            state.confirmed_path = Some(path);
        }

        ServerMessage::PathNotFound => {
            warn!("No path found to target!");
            state.confirmed_path = None;
        }

        ServerMessage::ObstacleData { obstacles } => {
            state.pathfinder.obstacles = obstacles.into_iter().collect();
            info!(
                "Received {} obstacles from server",
                state.pathfinder.obstacles.len()
            );
        }

        ServerMessage::InventoryUpdate { inventory } => {
            state.inventory = inventory;
            debug!("Inventory updated");
        }

        ServerMessage::ItemAdded {
            item_type,
            quantity,
        } => {
            let def = ItemDefinition::get(item_type);
            let total = state.inventory.count_item(item_type);
            info!("Received {} x{} (total: {})", def.name, quantity, total);
        }

        ServerMessage::ItemRemoved {
            item_type,
            quantity,
        } => {
            let def = ItemDefinition::get(item_type);
            info!("Removed {} x{}", def.name, quantity);
        }

        ServerMessage::SkillUpdate {
            skill,
            level,
            experience,
        } => {
            state.skills.insert(skill, SkillData { level, experience });
            debug!("{:?}: Level {} (XP: {})", skill, level, experience);
        }

        ServerMessage::LevelUp { skill, new_level } => {
            info!("LEVEL UP! {:?} is now level {}!", skill, new_level);
        }

        ServerMessage::ExperienceGained { skill, amount } => {
            if let Some(skill_data) = state.skills.get(&skill) {
                info!(
                    "{} {:?} XP (total: {})",
                    amount, skill, skill_data.experience
                );
            }
        }

        ServerMessage::TreeChopped { tree_entity_id } => {
            if let Some(entity) = state.visible_entities.get_mut(&tree_entity_id) {
                if let Some(ref mut tree) = entity.tree {
                    tree.is_chopped = true;
                    info!("Tree {} chopped!", tree_entity_id);
                }
            }
        }

        ServerMessage::TreeRespawned { tree_entity_id } => {
            if let Some(entity) = state.visible_entities.get_mut(&tree_entity_id) {
                if let Some(ref mut tree) = entity.tree {
                    tree.is_chopped = false;
                    info!("Tree {} respawned!", tree_entity_id);
                }
            }
        }

        ServerMessage::NotEnoughLevel {
            skill,
            required,
            current,
        } => {
            warn!(
                "Need level {} {:?} (current: {})",
                required, skill, current
            );
        }

        ServerMessage::NoAxeEquipped => {
            warn!("You need an axe to chop this tree!");
        }

        _ => {}
    }
}

pub fn handle_server_message_unreliable(msg: ServerMessage, state: &mut ClientState, time: &Time) {
    if let ServerMessage::DeltaUpdate { tick: _, deltas } = msg {
        for delta in deltas {
            match delta.delta_type {
                DeltaType::FullState {
                    tile_pos,
                    player_id,
                    last_processed_input,
                } => {
                    let is_my_player = player_id == state.my_player_id;
                    let current_time = time.elapsed_seconds_f64();

                    if let Some(entity) = state.visible_entities.get_mut(&delta.entity_id) {
                        entity.server_position = tile_pos;
                        entity.player_id = player_id;

                        if is_my_player {
                            state.my_entity_id = Some(delta.entity_id);
                            entity.tile_position = tile_pos;
                        } else {
                            // other player - add to position buffer for interpolation
                            if state.entity_interpolation {
                                entity.position_buffer.push(PositionSnapshot {
                                    timestamp: current_time,
                                    position: tile_pos,
                                });
                            } else {
                                entity.tile_position = tile_pos;
                            }
                        }
                    }

                    if is_my_player {
                        if state.server_reconciliation {
                            if let Some(last_input) = last_processed_input {
                                reconcile_client_state(state, delta.entity_id, last_input);
                            }
                        } else {
                            state.pending_inputs.clear();
                        }
                        state.pending_move = None;
                    }
                }
                DeltaType::PositionOnly { tile_pos, last_processed_input } => {
                    let is_my_entity = Some(delta.entity_id) == state.my_entity_id;
                    let current_time = time.elapsed_seconds_f64();

                    if let Some(entity) = state.visible_entities.get_mut(&delta.entity_id) {
                        entity.server_position = tile_pos;

                        if is_my_entity {
                            entity.tile_position = tile_pos;
                        } else {
                            // other entity - add to position buffer for interpolation
                            if state.entity_interpolation {
                                entity.position_buffer.push(PositionSnapshot {
                                    timestamp: current_time,
                                    position: tile_pos,
                                });
                            } else {
                                entity.tile_position = tile_pos;
                            }
                        }
                    }

                    if is_my_entity {
                        if state.server_reconciliation {
                            if let Some(last_input) = last_processed_input {
                                reconcile_client_state(state, delta.entity_id, last_input);
                            }
                        } else {
                            state.pending_inputs.clear();
                        }
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
                DeltaType::ActionStarted { action: _ } => {}
                DeltaType::Removed => {
                    state.visible_entities.remove(&delta.entity_id);
                }
            }
        }
    }
}

/// server reconciliation: re-apply inputs that the server hasn't processed yet
fn reconcile_client_state(state: &mut ClientState, entity_id: u64, last_processed_input: u32) {
    // remove all inputs that have been processed by the server
    state.pending_inputs.retain(|input| input.input_sequence_number > last_processed_input);

    info!(
        "Reconciliation: server processed up to input #{}, {} inputs remaining",
        last_processed_input,
        state.pending_inputs.len()
    );

    // re-apply all remaining inputs on top of the server's authoritative state
    if let Some(entity) = state.visible_entities.get_mut(&entity_id) {
        for pending_input in &state.pending_inputs {
            apply_action_to_position(&pending_input.action, &mut entity.tile_position);
            info!(
                "Re-applied input #{}: {:?} -> {:?}",
                pending_input.input_sequence_number, pending_input.action, entity.tile_position
            );
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
            "Spawned local player entity at {:?}",
            snapshot.tile_position
        );
    } else if snapshot.tree.is_some() {
        debug!(
            "Spawned tree entity {} at {:?}",
            snapshot.entity_id, snapshot.tile_position
        );
    } else {
        info!(
            "Spawned remote player entity {} at {:?}",
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
            position_buffer: Vec::new(),
            server_position: snapshot.tile_position,
            interpolated_position: None,
        },
    );
}

/// Interpolation system - computes smooth positions for remote entities
pub fn interpolate_entities(mut client_state: ResMut<ClientState>, time: Res<Time>) {
    if !client_state.entity_interpolation {
        return;
    }

    let current_time = time.elapsed_seconds_f64();
    let render_timestamp = current_time - client_state.interpolation_delay;
    let my_entity_id = client_state.my_entity_id;

    for (entity_id, entity) in client_state.visible_entities.iter_mut() {
        if Some(*entity_id) == my_entity_id {
            continue;
        }
        if entity.tree.is_some() {
            continue;
        }

        let buffer = &mut entity.position_buffer;

        // drop old positions that are older than we need
        buffer.retain(|snapshot| snapshot.timestamp >= render_timestamp - 1.0);

        // if we don't have enough data, just use the server position
        if buffer.len() < 2 {
            entity.interpolated_position = Some(entity.server_position);
            continue;
        }

        // find the two positions surrounding the render timestamp
        let mut p0: Option<&PositionSnapshot> = None;
        let mut p1: Option<&PositionSnapshot> = None;

        for i in 0..buffer.len() - 1 {
            if buffer[i].timestamp <= render_timestamp && render_timestamp <= buffer[i + 1].timestamp {
                p0 = Some(&buffer[i]);
                p1 = Some(&buffer[i + 1]);
                break;
            }
        }

        if let (Some(snap0), Some(snap1)) = (p0, p1) {
            // linear interpolation between the two positions
            let t0 = snap0.timestamp;
            let t1 = snap1.timestamp;
            let pos0 = snap0.position;
            let pos1 = snap1.position;

            let interpolation_factor = if (t1 - t0).abs() > 0.0001 {
                ((render_timestamp - t0) / (t1 - t0)).clamp(0.0, 1.0)
            } else {
                0.0
            };

            // for tile-based movement, snap to nearest tile
            entity.interpolated_position = if interpolation_factor < 0.5 {
                Some(pos0)
            } else {
                Some(pos1)
            };
        } else {
            // fallback to latest server position
            entity.interpolated_position = Some(entity.server_position);
        }
    }
}

/// helper function to apply an action to a position for prediction and reconciliation
fn apply_action_to_position(action: &GameAction, position: &mut TilePosition) {
    match action {
        GameAction::Move { path } => {
            if let Some(first_pos) = path.first() {
                *position = *first_pos;
            }
        }
        _ => {
            // other actions don't change position immediately
        }
    }
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
