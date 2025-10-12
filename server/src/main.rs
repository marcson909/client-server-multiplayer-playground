use bevy::log::tracing_subscriber;
use bevy::prelude::*;
use bevy_renet::transport::NetcodeServerPlugin;
use bevy_renet::*;
use server::interest_manager::InterestManager;
use server::{server_update_system, setup_server, ServerState};

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    App::new()
        .add_plugins(MinimalPlugins)
        .add_plugins(RenetServerPlugin)
        .add_plugins(NetcodeServerPlugin)
        .init_resource::<ServerState>()
        .init_resource::<InterestManager>()
        .add_systems(Startup, setup_server)
        .add_systems(Update, server_update_system)
        .run();
}
