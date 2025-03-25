use crate::{
    components::{animation, npc, transform, AnimationComponent},
    math::{StdbQuaternion, StdbVector3},
};
use spacetimedb::{reducer, ReducerContext};

#[reducer]
pub fn move_npc(
    ctx: &ReducerContext,
    timestamp: u64,
    entity_id: u64,
    pos: StdbVector3,
    rot: StdbQuaternion,
    duration: u64,
) {
    /*
    TODO: Uncomment when supported.
    if identity != 0 {
        log::info!("Only the server should move NPCs (allowed for now)");
    }
    */

    // Next action timestamp
    let mut npc = ctx
        .db
        .npc()
        .entity_id()
        .find(&entity_id)
        .expect("This npc doesn't exist.");
    npc.next_action = timestamp + duration;
    ctx.db.npc().entity_id().update(npc);

    let mut transform = ctx
        .db
        .transform()
        .entity_id()
        .find(&entity_id)
        .expect("This transform doesn't exist.");
    transform.pos = pos;
    transform.rot = rot;
    ctx.db.transform().entity_id().update(transform);
}

#[reducer]
pub fn update_npc_animation(
    ctx: &ReducerContext,
    timestamp: u64,
    entity_id: u64,
    moving: bool,
    action_target_entity_id: u64,
) {
    let _npc = ctx
        .db
        .npc()
        .entity_id()
        .find(&entity_id)
        .expect("This npc doesn't exist.");

    /*
    TODO: Uncomment when supported.
    // Make sure this identity owns this player
    if identity != 0 {
        log::info!("Only the server should animate NPCs (allowed for now)");
    }
    */

    ctx.db.animation().entity_id().update(AnimationComponent {
        entity_id,
        moving,
        action_target_entity_id,
    });
}
