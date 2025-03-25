use spacetimedb::table;

#[table(name = npc)]
pub struct NpcComponent {
    #[unique]
    #[auto_inc]
    pub entity_id: u64,
    pub model: String,
    pub next_action: u64,
}
