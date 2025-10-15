use std::net::{SocketAddr, UdpSocket};
use std::time::SystemTime;

use bevy::prelude::*;
use bevy::utils::tracing::info;
use bevy::utils::HashMap;
use bevy_renet::renet::transport::{ClientAuthentication, NetcodeClientTransport};
use bevy_renet::renet::*;

use shared::actions::GameAction;
use shared::inventory::Inventory;
use shared::pathfinding::Pathfinder;
use shared::skills::{SkillData, SkillType};
use shared::tile_system::TilePosition;
use shared::trees::Tree;
use shared::*;

pub mod camera;
pub mod debug_ui;
pub mod systems;

#[derive(Component)]
pub struct LocalPlayer;

#[derive(Component)]
pub struct NetworkedEntity {
    pub entity_id: u64,
}

#[derive(Resource)]
pub struct ClientState {
    pub my_player_id: Option<PlayerId>,
    pub my_entity_id: Option<u64>,
    pub visible_entities: HashMap<u64, ClientEntity>,
    pub current_position: Option<TilePosition>,
    pub pending_move: Option<TilePosition>,
    pub pathfinder: Pathfinder,
    pub path_preview: Option<Vec<TilePosition>>,
    pub confirmed_path: Option<Vec<TilePosition>>,
    pub inventory: Inventory,
    pub skills: HashMap<SkillType, SkillData>,
    pub hover_entity: Option<u64>,
    pub join_sent: bool,
    pub input_sequence_number: u32,
    pub pending_inputs: Vec<PendingInput>,
    pub client_side_prediction: bool,
    pub server_reconciliation: bool,
    pub entity_interpolation: bool,
    pub interpolation_delay: f64, // delay in seconds (render timestamp = now - delay)
    pub show_debug_ui: bool,
    pub show_prediction_ghosts: bool,
    pub show_interpolation_ghosts: bool,
}

#[derive(Clone, Debug)]
pub struct PendingInput {
    pub input_sequence_number: u32,
    pub action: GameAction,
}

#[derive(Clone, Debug)]
pub struct PositionSnapshot {
    pub timestamp: f64,  // time in seconds since startup
    pub position: TilePosition,
}

pub struct ClientEntity {
    pub tile_position: TilePosition,
    pub player_id: Option<PlayerId>,
    pub entity: Entity,
    pub tree: Option<Tree>,
    pub position_buffer: Vec<PositionSnapshot>,
    pub server_position: TilePosition,
    pub interpolated_position: Option<TilePosition>,
}

impl Default for ClientState {
    fn default() -> Self {
        Self {
            my_player_id: None,
            my_entity_id: None,
            visible_entities: HashMap::new(),
            current_position: None,
            pending_move: None,
            pathfinder: Pathfinder::new(false),
            path_preview: None,
            confirmed_path: None,
            inventory: Inventory::new(28),
            skills: HashMap::new(),
            hover_entity: None,
            join_sent: false,
            input_sequence_number: 0,
            pending_inputs: Vec::new(),
            client_side_prediction: true,
            server_reconciliation: true,
            entity_interpolation: true,
            interpolation_delay: 0.1,
            show_debug_ui: true,
            show_prediction_ghosts: true,
            show_interpolation_ghosts: true,
        }
    }
}

pub fn setup_client(mut commands: Commands) {
    commands.spawn(Camera2dBundle::default());

    let server_addr: SocketAddr = format!("127.0.0.1:{}", SERVER_PORT).parse().unwrap();
    let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
    let current_time = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();
    let client_id = current_time.as_millis() as u64;

    let authentication = ClientAuthentication::Unsecure {
        client_id,
        protocol_id: PROTOCOL_ID,
        server_addr,
        user_data: None,
    };

    let transport = NetcodeClientTransport::new(current_time, authentication, socket).unwrap();
    let client = RenetClient::new(ConnectionConfig::default());

    commands.insert_resource(client);
    commands.insert_resource(transport);

    info!("Client starting...");
    info!("Connecting to server at {}", server_addr);
    info!("Client ID: {}", client_id);
    info!("Protocol ID: {}", PROTOCOL_ID);
    info!("");
    info!("Controls:");
    info!("  WASD - Move one tile");
    info!("  Click - Walk to tile or chop tree");
    info!("  Trees: Green=Normal, Brown=Oak, Light Green=Willow");
}
