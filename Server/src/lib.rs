mod components;
mod math;
mod npcs;
mod tables;
mod trade_session;
mod tuples;

use std::time::Duration;

use crate::components::chunk_component::generate_terrain_stub;
use crate::npcs::{despawn_npcs, move_npcs, spawn_npcs};
use crate::tables::{Config, PlayerChatMessage, ServerGlobals};
use crate::trade_session::cancel_trade_session_with_participant;
use crate::tuples::Pocket;
use components::{
    active_trade, animation, inventory, player, player_login, resource, trade, transform, AnimationComponent,
    InventoryComponent, PlayerComponent, PlayerLoginComponent, TransformComponent,
};
use math::{StdbQuaternion, StdbVector3};
use spacetimedb::{log, reducer, ReducerContext, ScheduleAt, Table, Timestamp};
use tables::{
    check_chunks_for_all_players_timer, config, despawn_npcs_timer, move_npcs_timer, player_chat_message,
    server_globals, spawn_npcs_timer, CheckChunkForAllPlayersTimer, DespawnNpcsTimer, MoveNpcsTimer, SpawnNpcsTimer,
};
mod random;
mod terrain_generation;

// This is in charge of initializing any static global data
#[reducer(init)]
pub fn init(ctx: &ReducerContext) -> Result<(), String> {
    // TODO(cloutiertyler): Validate that the identity is the authorized
    // identity. (i.e. the one who initialized this database)

    let config = ctx.db.config().version().find(&0);
    if config.is_some() {
        log::info!("Config already exists, skipping config.");
        return Ok(());
    }
    log::info!("Creating new config!");

    ctx.db.config().insert(Config {
        version: 0,
        max_player_inventory_slots: 30,
        trading_slots: 18,
        chunk_terrain_resolution: 16,
        chunk_splat_resolution: 128,
        chunk_size: 10.0,
        entity_density: 16,
        terrain_seed: 78648326,
        min_spawn_range: 32.0,
        max_spawn_range: 48.0,
        npc_detection_range: 20.0,
    });

    ctx.db.server_globals().insert(ServerGlobals {
        version: 0,
        entity_id_counter: 0,
    });

    // This one terrain chunk is inserted so the client can identify the world state message. The issue we have right now
    // is that other players or agents can update tables, therefore there is now way to be sure that the first subscription
    // received by the client is the world state.
    // TODO: the client should be able to subscribe on demand and the server should make sure no subscription is received until then.
    generate_terrain_stub(ctx);

    ctx.db
        .check_chunks_for_all_players_timer()
        .try_insert(CheckChunkForAllPlayersTimer {
            scheduled_id: 0,
            scheduled_at: ScheduleAt::Interval(Duration::from_millis(1000).into()),
        });

    ctx.db.spawn_npcs_timer().try_insert(SpawnNpcsTimer {
        scheduled_id: 0,
        scheduled_at: ScheduleAt::Interval(Duration::from_millis(5000).into()),
    });

    ctx.db.despawn_npcs_timer().try_insert(DespawnNpcsTimer {
        scheduled_id: 0,
        scheduled_at: ScheduleAt::Interval(Duration::from_millis(15000).into()),
    });

    ctx.db.move_npcs_timer().try_insert(MoveNpcsTimer {
        scheduled_id: 0,
        scheduled_at: ScheduleAt::Interval(Duration::from_millis(100).into()),
    });

    Ok(())
}

#[reducer]
pub fn move_or_swap_inventory_slot(
    ctx: &ReducerContext,
    player_entity_id: u64,
    inventory_entity_id: u64,
    source_pocket_idx: u32,
    dest_pocket_idx: u32,
) -> Result<(), String> {
    let config = ctx.db.config().version().find(&0).expect("Config exists.");

    // Check to see if the source pocket index is bad
    if source_pocket_idx >= config.max_player_inventory_slots {
        return Err(format!("The source pocket index is invalid: {}", source_pocket_idx));
    }

    // Check to see if the dest pocket index is bad
    if dest_pocket_idx >= config.max_player_inventory_slots {
        return Err(format!("The dest pocket index is invalid: {}", dest_pocket_idx));
    }

    if source_pocket_idx == dest_pocket_idx {
        // Cannot drag and drop on itself
        return Ok(());
    }

    // Make sure this identity owns this player
    let player = ctx
        .db
        .player()
        .entity_id()
        .find(&player_entity_id)
        .expect("This player doesn't exist!");
    if player.owner_id != ctx.sender {
        // TODO: We are doing this for now so that its easier to test reducers from the command line
        return Err(format!("This identity doesn't own this player! (allowed for now)"));
    }

    if inventory_entity_id != player_entity_id {
        // Make sure the player is allowed to modify this inventory
        let mut valid = false;

        // Is it part of a trade involving the player?
        if let Some(active_trade) = ctx.db.active_trade().entity_id().find(&player_entity_id) {
            if let Some(session) = ctx.db.trade().entity_id().find(&active_trade.trade_session_entity_id) {
                valid |= session.initiator_entity_id == player_entity_id;
                valid |= session.acceptor_entity_id == player_entity_id;
            }
        }

        // ToDo: external storages, etc.

        // We did all the checks for external inventory update.
        if !valid {
            return Err(format!("This player is not allowed to modify that inventory"));
        }
    }

    let mut inventory = ctx
        .db
        .inventory()
        .entity_id()
        .find(&inventory_entity_id)
        .expect("This inventory doesn't exist!");

    let mut source_pocket = inventory
        .get_pocket(source_pocket_idx)
        .expect("Nothing in source pocket, nothing to do.");

    let dest_pocket = inventory.get_pocket(dest_pocket_idx);

    // If we don't have a dest pocket, then just do a direct move
    if dest_pocket.is_none() {
        inventory.delete_pocket(source_pocket_idx);
        source_pocket.pocket_idx = dest_pocket_idx;
        inventory.set_pocket(source_pocket);
        ctx.db.inventory().entity_id().update(inventory);
        log::info!("Source pocket moved to dest pocket.");

        return Ok(());
    }

    // If we have a dest and source pocket then we have to see if we can stack onto the dest
    let mut dest_pocket = dest_pocket.unwrap();
    if source_pocket.item_id == dest_pocket.item_id {
        // Move source items to dest
        dest_pocket.item_count += source_pocket.item_count;
        inventory.delete_pocket(source_pocket_idx);
        inventory.set_pocket(dest_pocket);
        ctx.db.inventory().entity_id().update(inventory);
        log::info!("Source pocket moved into dest pocket (same item)");

        return Ok(());
    }

    inventory.delete_pocket(source_pocket_idx);
    inventory.delete_pocket(dest_pocket_idx);
    dest_pocket.pocket_idx = source_pocket_idx;
    source_pocket.pocket_idx = dest_pocket_idx;
    inventory.set_pocket(source_pocket);
    inventory.set_pocket(dest_pocket);
    ctx.db.inventory().entity_id().update(inventory);
    log::info!("Pockets swapped (different items)");

    Ok(())
}

/// This adds or removes items from an inventory slot. you can pass a negative item count in order
/// to remove items.
#[reducer]
pub fn add_item_to_inventory(
    ctx: &ReducerContext,
    entity_id: u64,
    item_id: u32,
    pocket_idx: i32, // < 0 to auto assign the first valid index
    item_count: i32,
) -> Result<(), String> {
    // Make sure this identity owns this player
    let player = ctx
        .db
        .player()
        .entity_id()
        .find(&entity_id)
        .expect("add_item_to_inventory: This player doesn't exist!");

    if player.owner_id != ctx.sender {
        // TODO: We are doing this for now so that its easier to test reducers from the command line
        log::info!("This identity doesn't own this player! (allowed for now)");
        // return;
    }

    let mut inventory = ctx
        .db
        .inventory()
        .entity_id()
        .find(&entity_id)
        .expect("This player doesn't have an inventory!");

    if !inventory.add(
        ctx,
        item_id,
        item_count,
        if pocket_idx < 0 { None } else { Some(pocket_idx as u32) },
    ) {
        return Err("Failed to add items to inventory".to_string());
    }

    ctx.db.inventory().entity_id().update(inventory);
    log::info!("Item {} inserted into inventory {}", item_id, entity_id);

    Ok(())
}

#[reducer]
pub fn dump_inventory(ctx: &ReducerContext, entity_id: u64) -> Result<(), String> {
    let inventory = ctx
        .db
        .inventory()
        .entity_id()
        .find(&entity_id)
        .unwrap_or_else(|| panic!("Inventory NOT found for entity {}", entity_id));

    for pocket in inventory.pockets {
        log::info!(
            "PocketIdx: {} Item: {} Count: {}",
            pocket.pocket_idx,
            pocket.item_id,
            pocket.item_count,
        );
    }

    Ok(())
}

#[reducer]
pub fn move_player(ctx: &ReducerContext, entity_id: u64, pos: StdbVector3, rot: StdbQuaternion) -> Result<(), String> {
    let player = ctx
        .db
        .player()
        .entity_id()
        .find(&entity_id)
        .expect("This player doesn't exist.");

    // Make sure this identity owns this player
    if player.owner_id != ctx.sender {
        log::info!("This identity doesn't own this player! (allowed for now)");
    }

    ctx.db
        .transform()
        .entity_id()
        .update(TransformComponent { entity_id, pos, rot });

    Ok(())
}

#[reducer]
pub fn update_animation(
    ctx: &ReducerContext,
    entity_id: u64,
    moving: bool,
    action_target_entity_id: u64,
) -> Result<(), String> {
    let player = ctx
        .db
        .player()
        .entity_id()
        .find(&entity_id)
        .expect("This player doesn't exist!");

    // Make sure this identity owns this player
    if player.owner_id != ctx.sender {
        log::info!("This identity doesn't own this player! (allowed for now)");
    }

    ctx.db.animation().entity_id().update(AnimationComponent {
        entity_id,
        moving,
        action_target_entity_id,
    });

    Ok(())
}

#[reducer]
pub fn create_new_player(
    ctx: &ReducerContext,
    start_pos: StdbVector3,
    start_rot: StdbQuaternion,
    username: String,
) -> Result<(), String> {
    let creation_time = ctx
        .timestamp
        .duration_since(Timestamp::UNIX_EPOCH)
        .ok_or("Failed to get creation time")?
        .as_millis() as u64;

    let new_player = ctx.db.player().insert(PlayerComponent {
        entity_id: 0,
        owner_id: ctx.sender,
        username,
        creation_time,
    });
    log::info!("Created player with this ID: {}", new_player.entity_id);

    ctx.db.inventory().insert(InventoryComponent {
        entity_id: new_player.entity_id,
        pockets: Vec::<Pocket>::new(),
    });
    ctx.db.transform().insert(TransformComponent {
        entity_id: new_player.entity_id,
        pos: start_pos,
        rot: start_rot,
    });
    log::info!("Player created: {}", new_player.entity_id);

    Ok(())
}

#[reducer]
pub fn player_chat(ctx: &ReducerContext, player_id: u64, message: String) -> Result<(), String> {
    let msg_time = ctx
        .timestamp
        .duration_since(Timestamp::UNIX_EPOCH)
        .ok_or("Failed to get message time")?
        .as_millis() as u64;

    let chat = PlayerChatMessage {
        player_id,
        msg_time,
        message,
    };

    ctx.db.player_chat_message().insert(chat);

    Ok(())
}

#[reducer]
pub fn player_update_login_state(ctx: &ReducerContext, logged_in: bool) -> Result<(), String> {
    let player = ctx
        .db
        .player()
        .owner_id()
        .find(&ctx.sender)
        .expect("You cannot sign in without a player!");

    if let Some(login_state) = ctx.db.player_login().entity_id().find(&player.entity_id) {
        assert!(
            login_state.logged_in != logged_in,
            "Player is already set to this login state: {}",
            logged_in
        );
        let player_entity_id = player.entity_id;

        if !logged_in {
            cancel_trade_session_with_participant(ctx, player_entity_id);
        }

        ctx.db.player_login().entity_id().update(PlayerLoginComponent {
            entity_id: player_entity_id,
            logged_in,
        });

        return Ok(());
    }

    log::info!("Player set login state to: {}", logged_in);
    ctx.db.player_login().insert(PlayerLoginComponent {
        entity_id: player.entity_id,
        logged_in,
    });

    Ok(())
}

#[reducer(client_connected)]
pub fn identity_connected(ctx: &ReducerContext) -> Result<(), String> {
    let player = ctx.db.player().owner_id().find(&ctx.sender);
    if let Some(player) = player {
        log::info!("Player {} has returned.", player.entity_id);
    } else {
        log::info!("A new identity has connected.");
    }

    Ok(())
}

#[reducer(client_disconnected)]
pub fn identity_disconnected(ctx: &ReducerContext) -> Result<(), String> {
    if let Some(player) = ctx.db.player().owner_id().find(&ctx.sender) {
        if let Some(login_state) = ctx.db.player_login().entity_id().find(&player.entity_id) {
            if login_state.logged_in {
                log::info!("User has disconnected without signing out.");
                let player_entity_id = player.entity_id;

                cancel_trade_session_with_participant(ctx, player_entity_id);

                ctx.db.player_login().entity_id().update(PlayerLoginComponent {
                    entity_id: player_entity_id,
                    logged_in: false,
                });
            }
        }
    }

    Ok(())
}

#[reducer]
pub fn extract(ctx: &ReducerContext, entity_id: u64, resource_entity_id: u64) -> Result<(), String> {
    let player = ctx
        .db
        .player()
        .entity_id()
        .find(&entity_id)
        .expect("This player doesn't exist.");

    // Make sure this identity owns this player
    if player.owner_id != ctx.sender {
        log::info!("This identity doesn't own this player! (allowed for now)");
    }

    // ToDo: validate resource distance from player. For now resource position is determined by the chunk so we can't.

    let mut resource = ctx
        .db
        .resource()
        .entity_id()
        .find(&resource_entity_id)
        .expect("This resource doesn't exist");

    // Attempt to add resources to the player's inventory
    add_item_to_inventory(
        ctx,
        entity_id,
        resource.item_yield_id.into(),
        -1,
        resource.item_yield_quantity.into(),
    )?;

    resource.health -= 1;

    if resource.health <= 0 {
        ctx.db.resource().entity_id().delete(&resource_entity_id);
    } else {
        ctx.db.resource().entity_id().update(resource);
    }

    Ok(())
}
