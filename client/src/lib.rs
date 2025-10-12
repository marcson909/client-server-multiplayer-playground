use std::net::{SocketAddr, UdpSocket};
use std::time::SystemTime;

use bevy::prelude::*;
use bevy::utils::tracing::info;
use bevy::utils::HashMap;
use bevy_renet::renet::transport::{ClientAuthentication, NetcodeClientTransport};
use bevy_renet::renet::*;

use shared::inventory::Inventory;
use shared::pathfinding::Pathfinder;
use shared::skills::{SkillData, SkillType};
use shared::tile_system::TilePosition;
use shared::trees::Tree;
use shared::*;

pub mod camera;
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
}

pub struct ClientEntity {
    pub tile_position: TilePosition,
    pub player_id: Option<PlayerId>,
    pub entity: Entity,
    pub tree: Option<Tree>,
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
