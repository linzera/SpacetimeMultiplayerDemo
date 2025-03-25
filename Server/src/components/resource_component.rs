use spacetimedb::table;

#[table(name = resource, public)]
pub struct ResourceComponent {
    #[primary_key]
    #[auto_inc]
    pub entity_id: u64,
    pub health: u8,
    pub resource_id: u8,
    pub max_health: u8, // todo: ideally we would find that static data from a table using resource_id
    pub item_yield_id: u8,
    pub item_yield_quantity: u8,
}
