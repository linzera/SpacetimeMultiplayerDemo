use spacetimedb::table;

#[table(name = trade, public)]
#[derive(Clone)]
pub struct TradeSessionComponent {
    #[primary_key]
    pub entity_id: u64,
    pub initiator_entity_id: u64,
    pub acceptor_entity_id: u64,
    pub acceptor_offer_inventory_entity_id: u64,
    pub initiator_offer_inventory_entity_id: u64,
    pub approved_by_initiator: bool,
    pub approved_by_acceptor: bool,
}

#[table(name = active_trade, public)]
pub struct ActiveTradeComponent {
    #[primary_key]
    pub entity_id: u64,
    pub trade_session_entity_id: u64,
}
