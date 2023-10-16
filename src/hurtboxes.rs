use std::collections::HashSet;

use emerald::{
    ColliderBuilder, EmeraldError, Entity, Group, InteractionGroups, RigidBodyBuilder, Transform,
    Translation, Vector2, World,
};

use crate::tracker::SimpleTranslationTracker;

struct HurtboxParent(pub Entity);

pub struct HurtboxSet {
    pub hurtboxes: Vec<Entity>,
    /// The entity that owns this hurtbox, and will receive damage from it
    pub owner: Entity,
}
impl HurtboxSet {
    pub fn from_toml(
        world: &mut World,
        value: &emerald::toml::Value,
        owner: Entity,
        hurtbox_group: Group,
        hitbox_group: Group,
    ) -> Result<Self, EmeraldError> {
        let hurtboxes = value
            .get("hurtboxes")
            .unwrap_or(&emerald::toml::Value::Array(Vec::new()))
            .as_array()
            .unwrap_or(&Vec::new())
            .into_iter()
            .map(|value| Hurtbox::from_toml(value, owner))
            .collect::<Result<Vec<Hurtbox>, EmeraldError>>()?
            .into_iter()
            .map(|hurtbox| {
                let colliders = hurtbox.colliders.clone();
                let (id, rbh) = world.spawn_with_body(
                    (
                        hurtbox,
                        Transform::default(),
                        HurtboxParent(owner),
                        SimpleTranslationTracker {
                            target: owner,
                            offset: Translation::new(0.0, 0.0),
                        },
                    ),
                    RigidBodyBuilder::dynamic(),
                )?;

                for collider in colliders {
                    let builder = collider
                        .to_collider_builder()
                        .collision_groups(InteractionGroups::new(hurtbox_group, hitbox_group));
                    world.physics().build_collider(rbh, builder);
                }

                Ok(id)
            })
            .collect::<Result<Vec<Entity>, EmeraldError>>()?;

        Ok(Self { hurtboxes, owner })
    }

    fn get_active_hurtboxes(world: &World, hurtbox_entities: Vec<Entity>) -> Vec<Entity> {
        hurtbox_entities
            .into_iter()
            .filter(|id| {
                if let Ok(hurtbox) = world.get::<&Hurtbox>(id.clone()) {
                    return hurtbox.active;
                }

                false
            })
            .collect()
    }
}

pub fn get_hurtbox_owner(world: &World, hurtbox_id: Entity) -> Option<Entity> {
    world
        .get::<&Hurtbox>(hurtbox_id)
        .ok()
        .map(|h| h.parent_set.clone())
        .map(|p| {
            world
                .get::<&HurtboxSet>(p)
                .ok()
                .map(|set| set.owner.clone())
        })
        .flatten()
}

pub struct Hurtbox {
    pub active: bool,
    pub parent_set: Entity,
    pub colliders: Vec<RectCollider>,
}
impl Hurtbox {
    pub fn from_toml(
        value: &emerald::toml::Value,
        parent_set: Entity,
    ) -> Result<Self, EmeraldError> {
        let active = value
            .get("active")
            .unwrap_or(&emerald::toml::Value::Boolean(false))
            .as_bool()
            .unwrap_or(false);

        let colliders: Vec<RectCollider> = value
            .get("colliders")
            .unwrap_or(&emerald::toml::Value::Array(Vec::new()))
            .as_array()
            .unwrap_or(&Vec::new())
            .into_iter()
            .map(|value| RectCollider::from_toml(value))
            .collect::<Result<Vec<RectCollider>, EmeraldError>>()?;

        Ok(Self {
            active,
            parent_set,
            colliders,
        })
    }
}

#[derive(Clone, Debug)]
pub struct RectCollider {
    pub width: f32,
    pub height: f32,
    pub name: Option<String>,
    pub translation: Translation,
}
impl RectCollider {
    pub fn to_collider_builder(self) -> ColliderBuilder {
        ColliderBuilder::cuboid(self.width / 2.0, self.height / 2.0)
            .translation(Vector2::new(self.translation.x, self.translation.y))
            .sensor(true)
    }

    pub fn from_toml(value: &emerald::toml::Value) -> Result<Self, EmeraldError> {
        let width = value
            .get("width")
            .unwrap_or(&emerald::toml::Value::Float(0.0))
            .as_float()
            .unwrap_or(0.0) as f32;
        let height = value
            .get("height")
            .unwrap_or(&emerald::toml::Value::Float(0.0))
            .as_float()
            .unwrap_or(0.0) as f32;

        let mut name = None;

        if let Some(name_val) = value.get("name") {
            if let Some(n) = name_val.as_str() {
                name = Some(n.to_string());
            }
        }

        let mut translation = Translation::default();

        if let Some(value) = value.get("translation") {
            translation = toml_value_to_translation(value);
        }

        Ok(Self {
            width,
            height,
            translation,
            name,
        })
    }
}

pub fn toml_value_to_translation(value: &emerald::toml::Value) -> Translation {
    let x = value
        .get("x")
        .unwrap_or(&emerald::toml::Value::Float(0.0))
        .as_float()
        .unwrap_or(0.0) as f32;
    let y = value
        .get("y")
        .unwrap_or(&emerald::toml::Value::Float(0.0))
        .as_float()
        .unwrap_or(0.0) as f32;

    Translation::new(x, y)
}

pub fn get_all_active_hurtboxes(world: &World) -> Vec<Entity> {
    world
        .query::<&Hurtbox>()
        .iter()
        .filter_map(|(id, hurtbox)| if hurtbox.active { Some(id) } else { None })
        .collect()
}

/// returns all entities that have hurtboxes from the given set
fn get_active_hurtboxes(world: &World, entities: Vec<Entity>) -> Vec<Entity> {
    entities
        .into_iter()
        .filter_map(|id| {
            if let Ok(hurtbox) = world.get::<&Hurtbox>(id) {
                if hurtbox.active {
                    return Some(id);
                }
            }

            None
        })
        .collect()
}

fn get_colliding_active_hurtbox_sets(world: &mut World, id: Entity) -> Vec<Entity> {
    let hurtboxes = get_colliding_active_hurtboxes(world, id);

    let mut hurtbox_sets = HashSet::new();

    for id in hurtboxes {
        if let Ok(hurtbox) = world.get::<&Hurtbox>(id) {
            hurtbox_sets.insert(hurtbox.parent_set);
        }
    }

    hurtbox_sets.into_iter().collect()
}

pub fn get_colliding_active_hurtboxes(world: &mut World, id: Entity) -> Vec<Entity> {
    let colliding_entities = world.physics().get_colliding_entities(id);
    get_active_hurtboxes(world, colliding_entities)
}

pub fn get_hurtbox_parent_set(world: &World, id: Entity) -> Option<Entity> {
    if let Ok(hurtbox) = world.get::<&Hurtbox>(id) {
        return Some(hurtbox.parent_set);
    }

    None
}

pub fn get_creatures_from_hurtboxes(world: &World, hurtboxes: Vec<Entity>) -> Vec<Entity> {
    get_hurtbox_sets_from_hurtboxes(world, hurtboxes)
        .into_iter()
        .filter_map(|id| {
            if let Ok(hurtbox_set) = world.get::<&HurtboxSet>(id) {
                return Some(hurtbox_set.owner);
            }
            None
        })
        .collect::<HashSet<Entity>>()
        .into_iter()
        .collect()
}

pub fn get_hurtbox_sets_from_hurtboxes(world: &World, hurtboxes: Vec<Entity>) -> Vec<Entity> {
    hurtboxes
        .into_iter()
        .filter_map(|id| {
            if let Ok(hurtbox) = world.get::<&Hurtbox>(id) {
                return Some(hurtbox.parent_set);
            }

            None
        })
        .collect::<HashSet<Entity>>()
        .into_iter()
        .collect()
}
