use crate::math::StdbQuaternion;
use crate::math::StdbVector3;
use spacetimedb::table;

#[table(name = transform, public)]
pub struct TransformComponent {
    #[primary_key]
    pub entity_id: u64,
    pub pos: StdbVector3,
    pub rot: StdbQuaternion,
}
