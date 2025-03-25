use super::move_npc;
use super::update_npc_animation;
use crate::components::animation;
use crate::components::npc;
use crate::components::player_login;
use crate::components::transform;
use crate::components::{AnimationComponent, NpcComponent, TransformComponent};
use crate::math::StdbQuaternion;
use crate::math::StdbVector3;
use crate::random;
use crate::tables::config;
use crate::tables::DespawnNpcsTimer;
use crate::tables::MoveNpcsTimer;
use crate::tables::SpawnNpcsTimer;
use rand::Rng;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

use spacetimedb::Table;
use spacetimedb::{log, reducer, ReducerContext, Timestamp};

use std::f32::consts::PI;
use std::ops::Add;

#[reducer]
pub(crate) fn spawn_npcs(ctx: &ReducerContext, _timer: SpawnNpcsTimer) -> Result<(), String> {
    log::info!("spawn_npcs");

    let config = ctx.db.config().version().find(&0);
    if config.is_none() {
        return Err("Config not found".to_string());
    }
    let config = config.unwrap();
    let timestamp = ctx.timestamp.duration_since(Timestamp::UNIX_EPOCH).unwrap().as_millis() as u64;
    random::register();
    let mut rng = ChaCha8Rng::seed_from_u64(timestamp);

    let min_sq_radius = config.min_spawn_range.powi(2);

    // pick a random logged-in player around which the npc will be spawned
    let logged_in_players: Vec<u64> = ctx
        .db
        .player_login()
        .iter()
        .filter(|p| p.logged_in)
        .map(|p| p.entity_id)
        .collect();
    let count = logged_in_players.len();
    if count == 0 {
        // nobody is logged in, no need for npcs
        return Ok(());
    }
    let index = rng.gen_range(0..logged_in_players.len()) as usize;
    let player_entity_id = logged_in_players[index];

    let mut spawn_pos = ctx.db.transform().entity_id().find(&player_entity_id).unwrap().pos;

    let range = rng.gen_range(config.min_spawn_range..config.max_spawn_range);
    let rad = rng.gen_range(-PI..PI);

    spawn_pos.x += range * rad.cos();
    spawn_pos.z += range * rad.sin();

    // Make sure the position is not within (min_radius) distance of another player
    for player in ctx.db.player_login().iter() {
        if player.logged_in {
            let transform = ctx.db.transform().entity_id().find(&player.entity_id).unwrap();
            let dist = transform.pos.sq_distance(&spawn_pos);
            if dist < min_sq_radius {
                return Ok(());
            }
        }
    }

    // Make sure the position is not within (min_radius) distance of another npc
    for npc in ctx.db.npc().iter() {
        let transform = ctx.db.transform().entity_id().find(&npc.entity_id).unwrap();
        let dist = transform.pos.sq_distance(&spawn_pos);
        if dist < min_sq_radius {
            return Ok(());
        }
    }

    // Spawn the npc.
    let rot = StdbQuaternion::new(0.0, rng.gen_range(-PI..PI), 0.0);

    let npc_entity = ctx.db.npc().insert(NpcComponent {
        entity_id: 0,
        model: "Rabbit".to_string(),
        next_action: timestamp,
    });
    ctx.db.transform().insert(TransformComponent {
        entity_id: npc_entity.entity_id,
        pos: spawn_pos,
        rot,
    });
    ctx.db.animation().insert(AnimationComponent {
        entity_id: npc_entity.entity_id,
        moving: false,
        action_target_entity_id: 0,
    });

    Ok(())
}

#[reducer]
pub(crate) fn despawn_npcs(ctx: &ReducerContext, _timer: DespawnNpcsTimer) -> Result<(), String> {
    log::info!("despawn_npcs");

    let config = ctx.db.config().version().find(&0);
    if config.is_none() {
        return Err("Config not found".to_string());
    }
    let config = config.unwrap();

    let min_sq_radius = config.max_spawn_range.powi(2);

    let mut despawn_array = Vec::new();
    // Make sure the position is within (min_radius) distance of a player
    for npc in ctx.db.npc().iter() {
        let mut within_range = false;
        let npc_transform = ctx.db.transform().entity_id().find(&npc.entity_id).unwrap();
        for player in ctx.db.player_login().iter() {
            // Keep logged out players for this check so the NPC will still be there when you relog.
            let player_transform = ctx.db.transform().entity_id().find(&player.entity_id).unwrap();
            within_range |= npc_transform.pos.sq_distance(&player_transform.pos) <= min_sq_radius;
        }
        if !within_range {
            despawn_array.push(npc.entity_id);
        }
    }

    for entity_id in despawn_array {
        ctx.db.npc().entity_id().delete(&entity_id);
        ctx.db.transform().entity_id().delete(&entity_id);
        ctx.db.animation().entity_id().delete(&entity_id);
    }

    Ok(())
}

#[reducer]
pub(crate) fn move_npcs(ctx: &ReducerContext, _timer: MoveNpcsTimer) -> Result<(), String> {
    log::info!("move_npcs");

    let config = ctx.db.config().version().find(&0);
    if config.is_none() {
        return Err("Config not found".to_string());
    }
    let config = config.unwrap();
    let detection_range = config.npc_detection_range;

    let timestamp = ctx.timestamp.duration_since(Timestamp::UNIX_EPOCH).unwrap().as_millis() as u64;

    random::register();
    let mut rng = ChaCha8Rng::seed_from_u64(timestamp);

    let npc_entity_ids: Vec<u64> = ctx.db.npc().iter().map(|npc| npc.entity_id).collect();

    for npc_entity_id in npc_entity_ids {
        let npc = ctx.db.npc().entity_id().find(&npc_entity_id).unwrap();
        if npc.next_action > timestamp {
            continue;
        }

        let npc_transform = ctx.db.transform().entity_id().find(&npc_entity_id).unwrap();
        let mut vector = StdbVector3 { x: 0.0, y: 0.0, z: 0.0 };

        // Calculate threat level under the form of a vector
        for player in ctx.db.player_login().iter() {
            if player.logged_in {
                // Keep logged out players for this check so the NPC will still be there when you relog.
                let player_transform = ctx.db.transform().entity_id().find(&player.entity_id).unwrap();
                let delta = npc_transform.pos - player_transform.pos;
                let len = (detection_range - delta.length()).max(0.0);
                if len > 0.0 {
                    vector = vector.add(delta.normalized() * len);
                }
            }
        }

        if rng.gen_range(0.0..detection_range) <= vector.length() {
            // React on threat
            move_npc(
                ctx,
                timestamp,
                npc_entity_id,
                npc_transform.pos + vector.normalized() * 2.0,
                StdbQuaternion::look_rotation(vector, StdbVector3::up()),
                300,
            );
            update_npc_animation(ctx, timestamp, npc_entity_id, true, 0);
        } else {
            // React randomly
            let rnd = rng.gen_range(0..40);
            if rnd == 0 {
                let distance = rng.gen_range(1.0..2.0);
                let vector = StdbVector3 {
                    x: rng.gen_range(-1.0..1.0),
                    y: 0.0,
                    z: rng.gen_range(-1.0..1.0),
                };
                move_npc(
                    ctx,
                    timestamp,
                    npc_entity_id,
                    npc_transform.pos + vector.normalized() * distance,
                    StdbQuaternion::look_rotation(vector, StdbVector3::up()),
                    (150.0 * distance) as u64,
                );
                update_npc_animation(ctx, timestamp, npc_entity_id, true, 0);
            } else {
                let npc_animation = ctx.db.animation().entity_id().find(&npc_entity_id).unwrap();
                if npc_animation.moving {
                    update_npc_animation(ctx, timestamp, npc_entity_id, false, 0);
                }
                let mut npc = ctx.db.npc().entity_id().find(&npc_entity_id).unwrap();
                npc.next_action = timestamp + rng.gen_range(100..300);
                ctx.db.npc().entity_id().update(npc);
            }
        }
    }

    Ok(())
}
