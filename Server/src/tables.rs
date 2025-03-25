use spacetimedb::{table, ScheduleAt};

use crate::{despawn_npcs, move_npcs, spawn_npcs, terrain_generation::check_chunks_for_all_players};

#[derive(Copy, Clone)]
#[table(name = config, public)]
pub struct Config {
    #[unique]
    // always 0 for now
    pub version: u32,

    // Maximum amount of pockets the player can hold
    pub max_player_inventory_slots: u32,
    // Maximum amount of pockets the player can offer in a trade
    pub trading_slots: u32,
    // Terrain points in each direction per chunk
    pub chunk_terrain_resolution: u32,
    // Image resolution of the splats
    pub chunk_splat_resolution: u32,
    // In game size of the terrain chunk
    pub chunk_size: f64,
    pub terrain_seed: u32,
    // The amount of entities that can be placed on the terrain in each direction
    pub entity_density: u32,
    // The minimum range from a player or npc a npc can spawn
    pub min_spawn_range: f32,
    // The maximum range from a player a npc can spawn or remain spawned
    pub max_spawn_range: f32,
    // Range at which the npc will start reacting to the player
    pub npc_detection_range: f32,
}

#[derive(Copy, Clone)]
#[table(name = server_globals, public)]
pub struct ServerGlobals {
    #[unique]
    // always 0
    pub version: u32,

    // these are used for resources and trade sessions
    pub entity_id_counter: u64,
}

#[table(name = player_chat_message, public)]
pub struct PlayerChatMessage {
    pub player_id: u64,
    pub msg_time: u64,
    pub message: String,
}

#[table(name = check_chunks_for_all_players_timer, scheduled(check_chunks_for_all_players))]
pub struct CheckChunkForAllPlayersTimer {
    #[primary_key]
    #[auto_inc]
    pub scheduled_id: u64,
    pub scheduled_at: ScheduleAt,
}

#[table(name = spawn_npcs_timer, scheduled(spawn_npcs))]
pub struct SpawnNpcsTimer {
    #[primary_key]
    #[auto_inc]
    pub scheduled_id: u64,
    pub scheduled_at: ScheduleAt,
}

#[table(name = despawn_npcs_timer, scheduled(despawn_npcs))]
pub struct DespawnNpcsTimer {
    #[primary_key]
    #[auto_inc]
    pub scheduled_id: u64,
    pub scheduled_at: ScheduleAt,
}

#[table(name = move_npcs_timer, scheduled(move_npcs))]
pub struct MoveNpcsTimer {
    #[primary_key]
    #[auto_inc]
    pub scheduled_id: u64,
    pub scheduled_at: ScheduleAt,
}
