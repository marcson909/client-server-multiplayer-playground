use crate::interest_manager::InterestManager;
use bevy::prelude::*;
use bevy::utils::tracing::{debug, info, warn};
use bevy_renet::renet::transport::{NetcodeServerTransport, ServerAuthentication, ServerConfig};
use bevy_renet::renet::*;
use shared::actions::GameAction;
use shared::inventory::Inventory;
use shared::items::{ItemDefinition, ItemType};
use shared::netcode::{ClientMessage, DeltaType, EntityDelta, EntitySnapshot, ServerMessage};
use shared::pathfinding::Pathfinder;
use shared::skills::{SkillType, Skills};
use shared::tile_system::TilePosition;
use shared::trees::{Tree, TreeDefinition, TreeType};
use shared::*;
use std::collections::{HashMap, HashSet, VecDeque};
use std::net::{SocketAddr, UdpSocket};
use std::time::SystemTime;

pub mod interest_manager;

#[derive(Component)]
pub struct ActionQueue {
    pub actions: VecDeque<GameAction>,
    pub current_action: Option<ActionInProgress>,
}

#[derive(Clone, Debug)]
pub struct ActionInProgress {
    pub action: GameAction,
    pub started_at: f64,
    pub completion_time: f64,
    pub current_path_index: usize,
}

impl Default for ActionQueue {
    fn default() -> Self {
        Self {
            actions: VecDeque::new(),
            current_action: None,
        }
    }
}

#[derive(Resource)]
pub struct ServerState {
    pub players: HashMap<PlayerId, ServerPlayer>,
    pub entities: HashMap<u64, ServerEntity>,
    pub next_player_id: u64,
    pub next_entity_id: u64,
    pub server_tick: u64,
    pub tick_accumulator: f32,
    pub last_states: HashMap<u64, EntityLastState>,
    pub pathfinder: Pathfinder,
}

pub struct ServerPlayer {
    pub entity_id: u64,
    pub name: String,
}

pub struct ServerEntity {
    pub tile_pos: TilePosition,
    pub player_id: Option<PlayerId>,
    pub action_queue: ActionQueue,
    pub entity: Entity,
    pub is_obstacle: bool,
    pub inventory: Option<Inventory>,
    pub skills: Option<Skills>,
    pub tree: Option<Tree>,
}

#[derive(Default)]
pub struct EntityLastState {
    pub tile_pos: TilePosition,
    pub last_sent_tick: u64,
}

impl Default for ServerState {
    fn default() -> Self {
        let mut pathfinder = Pathfinder::new(false);

        // add boundary walls
        for x in -5..=5 {
            pathfinder.add_obstacle(TilePosition { x, y: 5 });
            pathfinder.add_obstacle(TilePosition { x, y: -5 });
        }
        for y in -5..=5 {
            pathfinder.add_obstacle(TilePosition { x: 5, y });
            pathfinder.add_obstacle(TilePosition { x: -5, y });
        }

        Self {
            players: HashMap::new(),
            entities: HashMap::new(),
            next_player_id: 1,
            next_entity_id: 1,
            server_tick: 0,
            tick_accumulator: 0.0,
            last_states: HashMap::new(),
            pathfinder,
        }
    }
}

pub fn setup_server(mut commands: Commands, mut state: ResMut<ServerState>) {
    let server_addr: SocketAddr = format!("127.0.0.1:{}", SERVER_PORT).parse().unwrap();
    let socket = UdpSocket::bind(server_addr).unwrap();
    let current_time = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();

    let server_config = ServerConfig {
        current_time,
        max_clients: 64,
        protocol_id: PROTOCOL_ID,
        public_addresses: vec![server_addr],
        authentication: ServerAuthentication::Unsecure,
    };

    let transport = NetcodeServerTransport::new(server_config, socket).unwrap();
    let server = RenetServer::new(ConnectionConfig::default());

    commands.insert_resource(server);
    commands.insert_resource(transport);

    spawn_trees(&mut state, &mut commands);

    info!("Server started on {}", server_addr);
    info!("Server configuration:");
    info!("Max clients: 64");
    info!("Protocol ID: {}", PROTOCOL_ID);
    info!("Tick rate: {}ms", (TICK_RATE * 1000.0) as u32);
    info!("View distance: {} tiles", VIEW_DISTANCE);
    info!(
        "Spawned {} entities (including {} trees)",
        state.entities.len(),
        state.entities.len()
    );
}

pub fn spawn_trees(state: &mut ServerState, commands: &mut Commands) {
    let tree_positions = vec![
        (TilePosition { x: -3, y: -3 }, TreeType::Normal),
        (TilePosition { x: -2, y: -3 }, TreeType::Normal),
        (TilePosition { x: 3, y: 3 }, TreeType::Oak),
        (TilePosition { x: 2, y: 3 }, TreeType::Oak),
        (TilePosition { x: -3, y: 3 }, TreeType::Willow),
        (TilePosition { x: 0, y: -4 }, TreeType::Normal),
        (TilePosition { x: 1, y: -4 }, TreeType::Oak),
    ];

    for (pos, tree_type) in tree_positions {
        let entity_id = state.next_entity_id;
        state.next_entity_id += 1;

        let entity = commands
            .spawn((pos, Transform::from_translation(pos.to_world().extend(0.0))))
            .id();

        let server_entity = ServerEntity {
            tile_pos: pos,
            player_id: None,
            action_queue: ActionQueue::default(),
            entity,
            is_obstacle: false,
            inventory: None,
            skills: None,
            tree: Some(Tree::new(tree_type)),
        };

        state.entities.insert(entity_id, server_entity);
        state.pathfinder.add_obstacle(pos);
    }
}

pub fn server_update_system(
    mut server: ResMut<RenetServer>,
    mut server_state: ResMut<ServerState>,
    mut interest_manager: ResMut<InterestManager>,
    time: Res<Time>,
    mut commands: Commands,
) {
    server_state.tick_accumulator += time.delta_seconds();

    for client_id in server.clients_id() {
        while let Some(message) = server.receive_message(client_id, DefaultChannel::ReliableOrdered)
        {
            debug!(
                "Received message from ClientId({}), {} bytes",
                client_id.raw(),
                message.len()
            );
            if let Ok(client_msg) = bincode::deserialize::<ClientMessage>(&message) {
                info!(
                    "Processing message from PlayerId({}): {:?}",
                    client_id.raw(),
                    match &client_msg {
                        ClientMessage::Join { name } => format!("Join(name={})", name),
                        ClientMessage::QueueAction { action } =>
                            format!("QueueAction({:?})", action),
                        ClientMessage::CancelAction => "CancelAction".to_string(),
                        ClientMessage::RequestPath { start, goal } =>
                            format!("RequestPath({:?} -> {:?})", start, goal),
                    }
                );
                handle_client_message(
                    client_msg,
                    PlayerId(client_id.raw()),
                    &mut server_state,
                    &mut interest_manager,
                    &mut server,
                    &mut commands,
                );
            }
        }
    }

    handle_disconnections(
        &mut server,
        &mut server_state,
        &mut interest_manager,
        &mut commands,
    );

    while server_state.tick_accumulator >= TICK_RATE {
        server_state.tick_accumulator -= TICK_RATE;
        server_state.server_tick += 1;
        debug!("Server tick #{}", server_state.server_tick);
        process_server_tick(&mut server_state, &mut server, &mut interest_manager);
    }
}

pub fn handle_client_message(
    message: ClientMessage,
    player_id: PlayerId,
    state: &mut ServerState,
    interest_manager: &mut InterestManager,
    server: &mut RenetServer,
    commands: &mut Commands,
) {
    match message {
        ClientMessage::Join { name } => {
            info!("Player {:?} joining with name '{}'", player_id, name);

            let spawn_pos = TilePosition { x: 0, y: 0 };
            let entity_id = state.next_entity_id;
            state.next_entity_id += 1;

            let mut inventory = Inventory::new(28);
            inventory.add_item(ItemType::BronzeAxe, 1);
            let skills = Skills::new();

            let entity = commands
                .spawn((
                    spawn_pos,
                    Transform::from_translation(spawn_pos.to_world().extend(0.0)),
                    ActionQueue::default(),
                ))
                .id();

            let server_entity = ServerEntity {
                tile_pos: spawn_pos,
                player_id: Some(player_id),
                action_queue: ActionQueue::default(),
                entity,
                is_obstacle: false,
                inventory: Some(inventory.clone()),
                skills: Some(skills.clone()),
                tree: None,
            };

            state.entities.insert(entity_id, server_entity);
            state.players.insert(
                player_id,
                ServerPlayer {
                    entity_id,
                    name: name.clone(),
                },
            );
            interest_manager
                .client_views
                .insert(player_id, HashSet::new());

            info!(
                "Player {:?} '{}' spawned at {:?} with entity_id={}",
                player_id, name, spawn_pos, entity_id
            );
            info!("Starting inventory: Bronze axe");
            info!("Active players: {}", state.players.len());

            let msg = ServerMessage::Welcome {
                player_id,
                spawn_position: spawn_pos,
            };
            send_message(server, player_id, &msg);

            let inv_msg = ServerMessage::InventoryUpdate { inventory };
            send_message(server, player_id, &inv_msg);

            for (skill_type, skill_data) in skills.skills {
                let skill_msg = ServerMessage::SkillUpdate {
                    skill: skill_type,
                    level: skill_data.level,
                    experience: skill_data.experience,
                };
                send_message(server, player_id, &skill_msg);
            }

            let obstacles: Vec<TilePosition> = state.pathfinder.obstacles.iter().copied().collect();
            info!(
                "Sending {} obstacles to player {:?}",
                obstacles.len(),
                player_id
            );
            let obstacle_msg = ServerMessage::ObstacleData { obstacles };
            send_message(server, player_id, &obstacle_msg);

            update_interest_for_player(player_id, state, interest_manager, server);
        }

        ClientMessage::QueueAction { action } => {
            if let Some(player) = state.players.get(&player_id) {
                info!(
                    "Player {:?} '{}' queuing action: {:?}",
                    player_id, player.name, action
                );

                if let GameAction::ChopTree { tree_entity_id } = action {

                    let validation_result = {
                        let player_entity = state.entities.get(&player.entity_id);
                        let tree_entity = state.entities.get(&tree_entity_id);

                        match (player_entity, tree_entity) {
                            (Some(p_entity), Some(t_entity)) => {
                                validate_woodcutting_action(p_entity, t_entity, server, player_id)
                            }
                            _ => {
                                warn!("Invalid woodcutting: entity not found (player={}, tree={})",
                                    player.entity_id, tree_entity_id);
                                false
                            }
                        }
                    };

                    if !validation_result {
                        return;
                    }
                }
                if let Some(entity) = state.entities.get_mut(&player.entity_id) {
                    entity.action_queue.actions.push_back(action.clone());
                    info!(
                        "Action queued for player {:?}. Queue size: {}",
                        player_id,
                        entity.action_queue.actions.len()
                    );
                    let msg = ServerMessage::ActionQueued { action };
                    send_message(server, player_id, &msg);
                }
            }
        }

        ClientMessage::CancelAction => {
            if let Some(player) = state.players.get(&player_id) {
                if let Some(entity) = state.entities.get_mut(&player.entity_id) {
                    let queue_size = entity.action_queue.actions.len();
                    entity.action_queue.current_action = None;
                    entity.action_queue.actions.clear();
                    info!(
                        "Player {:?} '{}' cancelled action. Cleared {} queued actions",
                        player_id, player.name, queue_size
                    );
                }
            }
        }

        ClientMessage::RequestPath { start, goal } => {
            info!(
                "Player {:?} requesting path from {:?} to {:?}",
                player_id, start, goal
            );

            if let Some(path) = state.pathfinder.find_path(start, goal) {
                info!("Path found: {} tiles", path.len());
                let msg = ServerMessage::PathFound { path: path.clone() };
                send_message(server, player_id, &msg);

                if let Some(player) = state.players.get(&player_id) {
                    if let Some(entity) = state.entities.get_mut(&player.entity_id) {
                        let move_action = GameAction::Move { path };
                        entity.action_queue.actions.push_back(move_action);
                    }
                }
            } else {
                warn!("No path found from {:?} to {:?}", start, goal);
                let msg = ServerMessage::PathNotFound;
                send_message(server, player_id, &msg);
            }
        }
    }
}

pub fn validate_woodcutting_action(
    player_entity: &ServerEntity,
    tree_entity: &ServerEntity,
    server: &mut RenetServer,
    player_id: PlayerId,
) -> bool {
    let tree = match &tree_entity.tree {
        Some(t) if !t.is_chopped => t,
        Some(t) if t.is_chopped => {
            warn!(
                "Player {:?} tried to chop already chopped tree",
                player_id
            );
            return false;
        }
        _ => {
            warn!("Player {:?} tried to chop invalid tree", player_id);
            return false;
        }
    };

    let tree_def = TreeDefinition::get(tree.tree_type);
    info!(
        "Validating woodcutting for player {:?}: tree={:?}, required_level={}",
        player_id, tree.tree_type, tree_def.level_required
    );

    if let Some(ref skills) = player_entity.skills {
        let wc_level = skills.get_level(SkillType::Woodcutting);
        if wc_level < tree_def.level_required {
            warn!(
                "Player {:?} insufficient level: has {}, needs {}",
                player_id, wc_level, tree_def.level_required
            );
            let msg = ServerMessage::NotEnoughLevel {
                skill: SkillType::Woodcutting,
                required: tree_def.level_required,
                current: wc_level,
            };
            send_message(server, player_id, &msg);
            return false;
        }
        info!("Level check passed: player has level {}", wc_level);
    }

    if let Some(ref inventory) = player_entity.inventory {
        if let Some(axe) = inventory.has_any_axe() {
            info!("Axe check passed: player has {:?}", axe);
        } else {
            warn!("Player {:?} has no axe", player_id);
            let msg = ServerMessage::NoAxeEquipped;
            send_message(server, player_id, &msg);
            return false;
        }
    }

    info!(
        "Woodcutting validation passed for player {:?}",
        player_id
    );
    true
}

pub fn process_server_tick(
    state: &mut ServerState,
    server: &mut RenetServer,
    interest_manager: &mut InterestManager,
) {
    let tick = state.server_tick;
    let current_time = tick as f64 * TICK_RATE as f64;

    let mut completed_actions = Vec::new();
    let mut woodcutting_completions = Vec::new();

    for (entity_id, entity) in state.entities.iter_mut() {
        if let Some(ref current_action) = entity.action_queue.current_action {
            if let GameAction::ChopTree { tree_entity_id } = current_action.action {
                if current_time >= current_action.completion_time {
                    woodcutting_completions.push((*entity_id, tree_entity_id));
                }
            }
        }

        process_action_queue(&mut entity.action_queue, &mut entity.tile_pos, current_time);

        if let Some(ref action_in_progress) = entity.action_queue.current_action {
            if current_time >= action_in_progress.completion_time {
                if !matches!(action_in_progress.action, GameAction::ChopTree { .. }) {
                    completed_actions.push(*entity_id);
                }
            }
        }
    }

    if !woodcutting_completions.is_empty() {
        info!(
            "Processing {} woodcutting completions",
            woodcutting_completions.len()
        );
    }

    for (player_entity_id, tree_entity_id) in woodcutting_completions {
        handle_woodcutting_completion(player_entity_id, tree_entity_id, state, server);
    }

    for entity_id in completed_actions {
        if let Some(entity) = state.entities.get_mut(&entity_id) {
            entity.action_queue.current_action = None;

            if let Some(player_id) = entity.player_id {
                debug!("Action completed for player {:?}", player_id);
                let msg = ServerMessage::ActionCompleted { entity_id };
                send_message(server, player_id, &msg);
            }
        }
    }

    // update tree respawn timers
    let mut respawned_trees = Vec::new();
    for (tree_entity_id, tree_entity) in state.entities.iter_mut() {
        if let Some(ref mut tree) = tree_entity.tree {
            if tree.is_chopped {
                tree.respawn_timer += TICK_RATE as f64;

                let tree_def = TreeDefinition::get(tree.tree_type);
                if tree.respawn_timer >= tree_def.respawn_time {
                    tree.is_chopped = false;
                    tree.respawn_timer = 0.0;
                    respawned_trees.push((*tree_entity_id, tree.tree_type));

                    let msg = ServerMessage::TreeRespawned {
                        tree_entity_id: *tree_entity_id,
                    };
                    broadcast_message(server, &msg);
                }
            }
        }
    }

    for (tree_id, tree_type) in respawned_trees {
        info!("Tree {} ({:?}) respawned", tree_id, tree_type);
    }

    for (player_id, _) in state.players.iter() {
        update_interest_for_player(*player_id, state, interest_manager, server);
    }

    send_delta_updates(state, interest_manager, server, tick);
}

pub fn process_action_queue(
    queue: &mut ActionQueue,
    tile_pos: &mut TilePosition,
    current_time: f64,
) {
    if let Some(ref mut action_in_progress) = queue.current_action {
        if current_time >= action_in_progress.completion_time {
            if let GameAction::Move { ref path } = action_in_progress.action {
                action_in_progress.current_path_index += 1;

                if action_in_progress.current_path_index < path.len() {
                    *tile_pos = path[action_in_progress.current_path_index];
                    action_in_progress.completion_time = current_time + TICK_RATE as f64;
                } else {
                    queue.current_action = None;
                }
            }
        }
        return;
    }

    if let Some(action) = queue.actions.pop_front() {
        let (duration, start_index) = match &action {
            GameAction::Move { path } => {
                if !path.is_empty() {
                    *tile_pos = path[0];
                }
                (TICK_RATE as f64, 0)
            }
            GameAction::ChopTree { .. } => (3.0, 0),
            GameAction::Attack { .. } => (2.4, 0),
            GameAction::UseItem { .. } => (0.6, 0),
            GameAction::Interact { .. } => (1.2, 0),
        };

        queue.current_action = Some(ActionInProgress {
            action: action.clone(),
            started_at: current_time,
            completion_time: current_time + duration,
            current_path_index: start_index,
        });
    }
}

pub fn handle_woodcutting_completion(
    player_entity_id: u64,
    tree_entity_id: u64,
    state: &mut ServerState,
    server: &mut RenetServer,
) {
    let tree_def = if let Some(tree_entity) = state.entities.get(&tree_entity_id) {
        if let Some(ref tree) = tree_entity.tree {
            let def = TreeDefinition::get(tree.tree_type);
            info!(
                "Processing woodcutting completion: tree={:?}, xp={}, logs={:?}",
                tree.tree_type, def.experience, def.logs_given
            );
            def
        } else {
            return;
        }
    } else {
        return;
    };

    if let Some(tree_entity) = state.entities.get_mut(&tree_entity_id) {
        if let Some(ref mut tree) = tree_entity.tree {
            tree.is_chopped = true;
            tree.respawn_timer = 0.0;
            info!(
                "Tree {} chopped! Will respawn in {}s",
                tree_entity_id, tree_def.respawn_time
            );
        }
    }

    let player_entity = match state.entities.get_mut(&player_entity_id) {
        Some(e) => e,
        None => return,
    };

    let player_id = match player_entity.player_id {
        Some(id) => id,
        None => return,
    };

    if let Some(ref mut inventory) = player_entity.inventory {
        if inventory.add_item(tree_def.logs_given, 1) {
            let def = ItemDefinition::get(tree_def.logs_given);
            info!(
                "Player {:?} received: {} x1 (total: {})",
                player_id,
                def.name,
                inventory.count_item(tree_def.logs_given)
            );

            let msg = ServerMessage::ItemAdded {
                item_type: tree_def.logs_given,
                quantity: 1,
            };
            send_message(server, player_id, &msg);

            let inv_msg = ServerMessage::InventoryUpdate {
                inventory: inventory.clone(),
            };
            send_message(server, player_id, &inv_msg);
        } else {
            warn!(
                " Player {:?} inventory full! Could not add logs",
                player_id
            );
        }
    }

    if let Some(ref mut skills) = player_entity.skills {
        let old_level = skills.get_level(SkillType::Woodcutting);
        let old_xp = skills.get_experience(SkillType::Woodcutting);
        let leveled_up = skills.add_experience(SkillType::Woodcutting, tree_def.experience);
        let new_xp = skills.get_experience(SkillType::Woodcutting);

        info!(
            "Player {:?} gained {} Woodcutting XP ({} -> {})",
            player_id, tree_def.experience, old_xp, new_xp
        );

        let xp_msg = ServerMessage::ExperienceGained {
            skill: SkillType::Woodcutting,
            amount: tree_def.experience,
        };
        send_message(server, player_id, &xp_msg);

        let skill_data = &skills.skills[&SkillType::Woodcutting];
        let skill_msg = ServerMessage::SkillUpdate {
            skill: SkillType::Woodcutting,
            level: skill_data.level,
            experience: skill_data.experience,
        };
        send_message(server, player_id, &skill_msg);

        if leveled_up {
            info!(
                "LEVEL UP! Player {:?} Woodcutting: {} -> {}",
                player_id, old_level, skill_data.level
            );
            let levelup_msg = ServerMessage::LevelUp {
                skill: SkillType::Woodcutting,
                new_level: skill_data.level,
            };
            send_message(server, player_id, &levelup_msg);
        }
    }

    player_entity.action_queue.current_action = None;

    let completion_msg = ServerMessage::ActionCompleted {
        entity_id: player_entity_id,
    };
    send_message(server, player_id, &completion_msg);

    let chopped_msg = ServerMessage::TreeChopped { tree_entity_id };
    broadcast_message(server, &chopped_msg);
    info!(
        "Broadcasted tree {} chopped to all players",
        tree_entity_id
    );
}

pub fn update_interest_for_player(
    player_id: PlayerId,
    state: &ServerState,
    interest_manager: &mut InterestManager,
    server: &mut RenetServer,
) {
    let player_entity_id = match state.players.get(&player_id) {
        Some(p) => p.entity_id,
        None => return,
    };

    let player_pos = match state.entities.get(&player_entity_id) {
        Some(e) => e.tile_pos,
        None => return,
    };

    let entity_positions: HashMap<u64, TilePosition> = state
        .entities
        .iter()
        .map(|(id, e)| (*id, e.tile_pos))
        .collect();

    let (entered, left) = interest_manager.update_view(player_id, player_pos, &entity_positions);

    if !entered.is_empty() {
        let snapshots: Vec<EntitySnapshot> = entered
            .iter()
            .filter_map(|id| {
                state.entities.get(id).map(|e| EntitySnapshot {
                    entity_id: *id,
                    tile_position: e.tile_pos,
                    player_id: e.player_id,
                    tree: e.tree.clone(),
                })
            })
            .collect();

        let msg = ServerMessage::EntitiesEntered {
            entities: snapshots,
        };
        send_message(server, player_id, &msg);
    }

    if !left.is_empty() {
        let msg = ServerMessage::EntitiesLeft { entity_ids: left };
        send_message(server, player_id, &msg);
    }
}

pub fn send_delta_updates(
    state: &mut ServerState,
    interest_manager: &InterestManager,
    server: &mut RenetServer,
    tick: u64,
) {
    let mut client_deltas: HashMap<PlayerId, Vec<EntityDelta>> = HashMap::new();

    for (entity_id, entity) in state.entities.iter() {
        let last_state = state
            .last_states
            .entry(*entity_id)
            .or_insert(EntityLastState {
                tile_pos: entity.tile_pos,
                last_sent_tick: 0,
            });

        let changed = last_state.tile_pos != entity.tile_pos || last_state.last_sent_tick == 0;

        if changed {
            let delta = EntityDelta {
                entity_id: *entity_id,
                delta_type: if last_state.last_sent_tick == 0 {
                    DeltaType::FullState {
                        tile_pos: entity.tile_pos,
                        player_id: entity.player_id,
                    }
                } else {
                    DeltaType::PositionOnly {
                        tile_pos: entity.tile_pos,
                    }
                },
            };

            for (player_id, view) in interest_manager.client_views.iter() {
                if view.contains(entity_id) {
                    client_deltas
                        .entry(*player_id)
                        .or_insert_with(Vec::new)
                        .push(delta.clone());
                }
            }

            last_state.tile_pos = entity.tile_pos;
            last_state.last_sent_tick = tick;
        }
    }

    for (player_id, deltas) in client_deltas {
        if !deltas.is_empty() {
            debug!(
                "Sending {} deltas to player {:?}",
                deltas.len(),
                player_id
            );
            let msg = ServerMessage::DeltaUpdate { tick, deltas };
            let msg_bytes = bincode::serialize(&msg).unwrap();
            server.send_message(
                ClientId::from_raw(player_id.0),
                DefaultChannel::Unreliable,
                msg_bytes,
            );
        }
    }
}

pub fn send_message(server: &mut RenetServer, player_id: PlayerId, msg: &ServerMessage) {
    let msg_type = match msg {
        ServerMessage::Welcome { .. } => "Welcome",
        ServerMessage::DeltaUpdate { .. } => "DeltaUpdate",
        ServerMessage::EntitiesEntered { .. } => "EntitiesEntered",
        ServerMessage::EntitiesLeft { .. } => "EntitiesLeft",
        ServerMessage::ActionQueued { .. } => "ActionQueued",
        ServerMessage::ActionCompleted { .. } => "ActionCompleted",
        ServerMessage::PathFound { .. } => "PathFound",
        ServerMessage::PathNotFound => "PathNotFound",
        ServerMessage::ObstacleData { .. } => "ObstacleData",
        ServerMessage::InventoryUpdate { .. } => "InventoryUpdate",
        ServerMessage::ItemAdded { .. } => "ItemAdded",
        ServerMessage::ItemRemoved { .. } => "ItemRemoved",
        ServerMessage::SkillUpdate { .. } => "SkillUpdate",
        ServerMessage::LevelUp { .. } => "LevelUp",
        ServerMessage::ExperienceGained { .. } => "ExperienceGained",
        ServerMessage::TreeChopped { .. } => "TreeChopped",
        ServerMessage::TreeRespawned { .. } => "TreeRespawned",
        ServerMessage::NotEnoughLevel { .. } => "NotEnoughLevel",
        ServerMessage::NoAxeEquipped => "NoAxeEquipped",
    };

    let msg_bytes = bincode::serialize(msg).unwrap();
    debug!(
        "Sending {} to player {:?} ({} bytes)",
        msg_type,
        player_id,
        msg_bytes.len()
    );
    server.send_message(
        ClientId::from_raw(player_id.0),
        DefaultChannel::ReliableOrdered,
        msg_bytes,
    );
}

pub fn broadcast_message(server: &mut RenetServer, msg: &ServerMessage) {
    let msg_type = match msg {
        ServerMessage::TreeChopped { .. } => "TreeChopped",
        ServerMessage::TreeRespawned { .. } => "TreeRespawned",
        ServerMessage::EntitiesLeft { .. } => "EntitiesLeft",
        _ => "Unknown",
    };

    let msg_bytes = bincode::serialize(msg).unwrap();
    debug!(
        "Broadcasting {} to all players ({} bytes)",
        msg_type,
        msg_bytes.len()
    );
    server.broadcast_message(DefaultChannel::ReliableOrdered, msg_bytes);
}

pub fn handle_disconnections(
    server: &mut RenetServer,
    state: &mut ServerState,
    interest_manager: &mut InterestManager,
    commands: &mut Commands,
) {
    // Get list of currently connected clients
    let connected_clients: HashSet<u64> =
        server.clients_id().into_iter().map(|id| id.raw()).collect();

    // Find players that are no longer connected
    let disconnected_players: Vec<PlayerId> = state
        .players
        .keys()
        .filter(|player_id| !connected_clients.contains(&player_id.0))
        .copied()
        .collect();

    // Clean up each disconnected player
    for player_id in disconnected_players {
        if let Some(player) = state.players.remove(&player_id) {
            info!("Player {:?} disconnected", player_id);

            // Remove player entity from world
            if let Some(entity_data) = state.entities.remove(&player.entity_id) {
                commands.entity(entity_data.entity).despawn();
            }

            // Remove from interest manager
            interest_manager.client_views.remove(&player_id);

            // Remove from last states
            state.last_states.remove(&player.entity_id);

            // Notify other clients that this player left
            let msg = ServerMessage::EntitiesLeft {
                entity_ids: vec![player.entity_id],
            };
            broadcast_message(server, &msg);
        }
    }
}
