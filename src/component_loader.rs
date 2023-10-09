use emerald::{toml, AssetLoader, EmeraldError, Entity, Group, World};

use crate::{hitboxes::HitboxSet, hurtboxes::HurtboxSet};

pub fn component_loader(
    _loader: &mut AssetLoader<'_>,
    entity: Entity,
    world: &mut World,
    value: &toml::Value,
    key: &str,
    hurtbox_group: Group,
    hitbox_group: Group,
) -> Result<(), EmeraldError> {
    match key {
        "hitbox_set" => {
            let hitbox_set =
                HitboxSet::from_toml(world, value, entity, hurtbox_group, hitbox_group)?;
            world.insert_one(entity, hitbox_set)?;
        }
        "hurtbox_set" => {
            let hurtbox_set =
                HurtboxSet::from_toml(world, value, entity, hurtbox_group, hitbox_group)?;
            world.insert_one(entity, hurtbox_set)?;
        }
        _ => {}
    }

    Ok(())
}
