use crate::components::{
    active_trade, inventory, player, trade, ActiveTradeComponent, InventoryComponent, PlayerComponent,
    TradeSessionComponent,
};
use crate::tuples::Pocket;
use spacetimedb::{log, reducer, ReducerContext, Table};

#[reducer]
pub fn initiate_trade_session(
    ctx: &ReducerContext,
    initiator_entity_id: u64,
    acceptor_entity_id: u64,
) -> Result<(), String> {
    log::info!(
        "Attempting trade session between {} and {}",
        initiator_entity_id,
        acceptor_entity_id,
    );

    let initiator = ctx
        .db
        .player()
        .entity_id()
        .find(&initiator_entity_id)
        .expect("The trade initiator doesn't exist!");
    let _acceptor = ctx
        .db
        .player()
        .entity_id()
        .find(&acceptor_entity_id)
        .expect("The trade acceptor doesn't exist!");

    if ctx.db.active_trade().entity_id().find(&initiator_entity_id).is_some() {
        log::info!("Trade initiator is already in a trading session.");
        return Ok(());
    }

    if ctx.db.active_trade().entity_id().find(&acceptor_entity_id).is_some() {
        log::info!("Trade acceptor is already in a trading session.");
        return Ok(());
    }

    // Make sure this identity owns this player
    if initiator.owner_id != ctx.sender {
        return Err(format!("This identity doesn't own this player! (allowed for now)"));
    }

    // Create trade session
    let trade_session = ctx.db.trade().insert(TradeSessionComponent {
        entity_id: 0,
        initiator_entity_id,
        acceptor_entity_id,
        initiator_offer_inventory_entity_id: 0,
        acceptor_offer_inventory_entity_id: 0,
        approved_by_acceptor: false,
        approved_by_initiator: false,
    });

    // Create trade components on each participant
    let acceptor_trade = ActiveTradeComponent {
        entity_id: acceptor_entity_id,
        trade_session_entity_id: trade_session.entity_id,
    };
    ctx.db.active_trade().insert(acceptor_trade);

    let initiator_trade = ActiveTradeComponent {
        entity_id: initiator_entity_id,
        trade_session_entity_id: trade_session.entity_id,
    };
    ctx.db.active_trade().insert(initiator_trade);

    // Create trade session inventories
    let initiator_offer = InventoryComponent {
        entity_id: 0,
        pockets: Vec::<Pocket>::new(),
    };
    ctx.db.inventory().insert(initiator_offer);

    let acceptor_offer = InventoryComponent {
        entity_id: 0,
        pockets: Vec::<Pocket>::new(),
    };
    ctx.db.inventory().insert(acceptor_offer);

    Ok(())
}

#[reducer]
pub fn add_to_trade(
    ctx: &ReducerContext,
    participant_entity_id: u64,
    source_pocket_id: u32,
    dest_pocket_id: u32,
) -> Result<(), String> {
    let participant = ctx
        .db
        .player()
        .entity_id()
        .find(&participant_entity_id)
        .expect("This player doesn't exist!");

    // Make sure this identity owns this player
    if participant.owner_id != ctx.sender {
        log::info!("This identity doesn't own this player! (allowed for now)");
    }

    // Retrieve active trade session entity_id
    let active_session = ctx
        .db
        .active_trade()
        .entity_id()
        .find(&participant_entity_id)
        .expect("There is no ongoing trade.");

    // Retrieve and update trade session
    let mut session = ctx
        .db
        .trade()
        .entity_id()
        .find(&active_session.trade_session_entity_id)
        .expect("This trade session no longer exists.");

    let offer_inventory_entity_id = if session.initiator_entity_id == participant_entity_id {
        session.initiator_offer_inventory_entity_id
    } else {
        session.acceptor_offer_inventory_entity_id
    };

    // Contents changed, trade is no longer approved by anyone
    session.approved_by_acceptor = false;
    session.approved_by_initiator = false;
    ctx.db.trade().entity_id().update(session);

    // Remove from player inventory
    let mut player_inventory = ctx
        .db
        .inventory()
        .entity_id()
        .find(&participant_entity_id)
        .expect("Player has no inventory.");
    let pocket = player_inventory
        .get_pocket(source_pocket_id)
        .expect("Traded items do not exist");
    if !player_inventory.add(ctx, pocket.item_id, -pocket.item_count, Some(source_pocket_id)) {
        return Err(format!("Failed to remove item from player inventory"));
    }
    ctx.db.inventory().entity_id().update(player_inventory);

    // Add to trade offer
    let mut offer_inventory = ctx
        .db
        .inventory()
        .entity_id()
        .find(&offer_inventory_entity_id)
        .expect("Trade session has no such offer");
    if !offer_inventory.add(ctx, pocket.item_id, pocket.item_count, Some(dest_pocket_id)) {
        return Err(format!("Failed to add item to trade window"));
    }
    ctx.db.inventory().entity_id().update(offer_inventory);

    Ok(())
}

#[reducer]
pub fn remove_from_trade(
    ctx: &ReducerContext,
    participant_entity_id: u64,
    source_pocket_id: u32,
    dest_pocket_id: u32,
) -> Result<(), String> {
    let participant = ctx
        .db
        .player()
        .entity_id()
        .find(&participant_entity_id)
        .expect("This player doesn't exist!");

    // Make sure this identity owns this player
    if participant.owner_id != ctx.sender {
        log::info!("This identity doesn't own this player! (allowed for now)");
    }

    // Retrieve active trade session entity_id
    let active_session = ctx
        .db
        .active_trade()
        .entity_id()
        .find(&participant_entity_id)
        .expect("There is no ongoing trade.");

    // Retrieve and update trade session
    let mut session = ctx
        .db
        .trade()
        .entity_id()
        .find(&active_session.trade_session_entity_id)
        .expect("This trade session no longer exists.");

    let offer_inventory_entity_id = if session.initiator_entity_id == participant_entity_id {
        session.initiator_offer_inventory_entity_id
    } else {
        session.acceptor_offer_inventory_entity_id
    };

    // Contents changed, trade is no longer approved by anyone
    session.approved_by_acceptor = false;
    session.approved_by_initiator = false;
    ctx.db.trade().entity_id().update(session);

    // Remove from trade offer
    let mut offer_inventory = ctx
        .db
        .inventory()
        .entity_id()
        .find(&offer_inventory_entity_id)
        .expect("Trade session has no such offer");
    let pocket = offer_inventory
        .get_pocket(source_pocket_id)
        .expect("Traded items do not exist");
    if !offer_inventory.add(ctx, pocket.item_id, -pocket.item_count, Some(source_pocket_id)) {
        return Err(format!("Failed to remove item from trade inventory"));
    }
    ctx.db.inventory().entity_id().update(offer_inventory);

    // Add to player inventory
    let mut player_inventory = ctx
        .db
        .inventory()
        .entity_id()
        .find(&participant_entity_id)
        .expect("Player has no inventory.");
    if !player_inventory.add(ctx, pocket.item_id, pocket.item_count, Some(dest_pocket_id)) {
        return Err(format!("Failed to add item to player inventory"));
    }
    ctx.db.inventory().entity_id().update(player_inventory);

    Ok(())
}

#[reducer]
pub fn toggle_accept_trade(ctx: &ReducerContext, participant_entity_id: u64) -> Result<(), String> {
    let participant = ctx
        .db
        .player()
        .entity_id()
        .find(&participant_entity_id)
        .expect("This player doesn't exist!");

    // Make sure this identity owns this player
    if participant.owner_id != ctx.sender {
        log::info!("This identity doesn't own this player! (allowed for now)");
    }

    // Retrieve active trade session entity_id
    let active_session = ctx
        .db
        .active_trade()
        .entity_id()
        .find(&participant_entity_id)
        .expect("There is no trade to approve.");

    // Retrieve and update trade session
    let mut session = ctx
        .db
        .trade()
        .entity_id()
        .find(&active_session.trade_session_entity_id)
        .expect("This trade session no longer exists.");

    if session.acceptor_entity_id == participant_entity_id {
        session.approved_by_acceptor = !session.approved_by_acceptor;
    } else if session.initiator_entity_id == participant_entity_id {
        session.approved_by_initiator = !session.approved_by_initiator;
    } else {
        return Err(format!(
            "This player is not part of the trade session. How is this possible?"
        ));
    }
    let close_session = session.approved_by_acceptor && session.approved_by_initiator;
    ctx.db.trade().entity_id().update(session);

    // If session is approved by both parties, conclude it succesfully
    if close_session {
        close_trade_session(ctx, active_session.trade_session_entity_id, true);
    }

    Ok(())
}

#[reducer]
pub fn refuse_trade(ctx: &ReducerContext, participant_entity_id: u64) -> Result<(), String> {
    let participant = ctx
        .db
        .player()
        .entity_id()
        .find(&participant_entity_id)
        .expect("This player doesn't exist!");

    // Make sure this identity owns this player
    if participant.owner_id != ctx.sender {
        log::info!("This identity doesn't own this player! (allowed for now)");
    }

    cancel_trade_session_with_participant(ctx, participant_entity_id);

    Ok(())
}

pub fn cancel_trade_session_with_participant(ctx: &ReducerContext, participant_entity_id: u64) {
    // Retrieve active trade session entity_id
    if let Some(active_session) = ctx.db.active_trade().entity_id().find(&participant_entity_id) {
        close_trade_session(ctx, active_session.trade_session_entity_id, false);
    }
}

pub fn close_trade_session(ctx: &ReducerContext, session_entity_id: u64, success: bool) {
    let session = ctx.db.trade().entity_id().find(&session_entity_id).unwrap();

    let can_trade = if success {
        // make sure both participants can receive every item of the trade
        let inventory = ctx
            .db
            .inventory()
            .entity_id()
            .find(&session.acceptor_offer_inventory_entity_id)
            .expect("There is no acceptor offer in this trade session.");
        let items: Vec<(u32, i32)> = inventory.pockets.iter().map(|p| (p.item_id, p.item_count)).collect();
        if inventory.can_hold(ctx, &items) {
            let inventory = ctx
                .db
                .inventory()
                .entity_id()
                .find(&session.initiator_offer_inventory_entity_id)
                .expect("There is no initiator offer in this trade session.");
            let items: Vec<(u32, i32)> = inventory.pockets.iter().map(|p| (p.item_id, p.item_count)).collect();
            inventory.can_hold(ctx, &items)
        } else {
            false
        }
    } else {
        false
    };

    if can_trade {
        // move offer contents into other participant's inventories
        let offer_inventory = ctx
            .db
            .inventory()
            .entity_id()
            .find(&session.initiator_offer_inventory_entity_id)
            .expect("There is no initiator offer in this trade session.");
        let mut player_inventory = ctx
            .db
            .inventory()
            .entity_id()
            .find(&session.initiator_entity_id)
            .expect("There is no acceptor in this trade session.");
        player_inventory.combine(ctx, &offer_inventory);
        ctx.db.inventory().entity_id().update(player_inventory);

        let offer_inventory = ctx
            .db
            .inventory()
            .entity_id()
            .find(&session.acceptor_offer_inventory_entity_id)
            .expect("There is no acceptor offer in this trade session.");
        let mut player_inventory = ctx
            .db
            .inventory()
            .entity_id()
            .find(&session.acceptor_entity_id)
            .expect("There is no acceptor in this trade session.");
        player_inventory.combine(ctx, &offer_inventory);
        ctx.db.inventory().entity_id().update(player_inventory);
    } else {
        // move offer contents back into each participant's inventories
        let offer_inventory = ctx
            .db
            .inventory()
            .entity_id()
            .find(&session.initiator_offer_inventory_entity_id)
            .expect("There is no initiator offer in this trade session.");
        let mut player_inventory = ctx
            .db
            .inventory()
            .entity_id()
            .find(&session.initiator_entity_id)
            .expect("There is no initiator in this trade session.");
        player_inventory.combine(ctx, &offer_inventory);
        ctx.db.inventory().entity_id().update(player_inventory);

        let offer_inventory = ctx
            .db
            .inventory()
            .entity_id()
            .find(&session.acceptor_offer_inventory_entity_id)
            .expect("There is no acceptor offer in this trade session.");
        let mut player_inventory = ctx
            .db
            .inventory()
            .entity_id()
            .find(&session.acceptor_entity_id)
            .expect("There is no acceptor in this trade session.");
        player_inventory.combine(ctx, &offer_inventory);
        ctx.db.inventory().entity_id().update(player_inventory);
    }

    // delete everything session-related
    ctx.db.trade().entity_id().delete(&session.entity_id);
    ctx.db.active_trade().entity_id().delete(&session.initiator_entity_id);
    ctx.db.active_trade().entity_id().delete(&session.acceptor_entity_id);
    ctx.db
        .inventory()
        .entity_id()
        .delete(&session.initiator_offer_inventory_entity_id);
    ctx.db
        .inventory()
        .entity_id()
        .delete(&session.acceptor_offer_inventory_entity_id);
}
