use spacetimedb::table;

#[table(name = player_login, public)]
pub struct PlayerLoginComponent {
    #[primary_key]
    pub entity_id: u64,
    pub logged_in: bool,
}
