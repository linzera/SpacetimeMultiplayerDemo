use spacetimedb::{table, Identity};

#[table(name = player, public)]
pub struct PlayerComponent {
    #[primary_key]
    #[auto_inc]
    pub entity_id: u64,
    #[unique]
    pub owner_id: Identity,
    #[unique]
    pub username: String,
    pub creation_time: u64,
}
