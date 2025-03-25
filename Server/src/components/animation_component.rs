use spacetimedb::table;

#[table(name = animation, public)]
#[derive(Clone)]
pub struct AnimationComponent {
    #[primary_key]
    pub entity_id: u64,
    pub moving: bool,
    pub action_target_entity_id: u64,
}
