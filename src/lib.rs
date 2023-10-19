use std::collections::{HashMap, HashSet};

use emerald::{toml::Value, Emerald, EmeraldError, Entity, World};
use hitboxes::{get_all_active_hitboxes, get_hitbox_owner, hitbox_system, Hitbox, HitboxSet};
use hurtboxes::{get_colliding_active_hurtboxes, get_hurtbox_owner, Hurtbox, HurtboxSet};
use tracker::{tracker_system, SimpleTranslationTracker};

pub mod component_loader;
pub mod hitboxes;
pub mod hurtboxes;
pub(crate) mod tracker;

pub struct OnTagTriggerContext {
    pub tag: String,
    pub hitbox_set_owner: Entity,
    pub data: Value,
}
pub struct OnHitFilterContext {
    /// The entity that is hitting something.
    pub hit_entity: Entity,

    /// The entity that is hurting.
    pub hurt_entity: Entity,

    /// The hurtbox touched by the hitbox
    pub hurtbox: Entity,

    /// The hitbox touching the hurtbox.
    pub hitbox: Entity,
}

pub struct OnHitContext {
    /// The entity that is hitting something.
    pub hit_entity: Entity,

    /// The entity that is hurting.
    pub hurt_entity: Entity,

    /// The hurtbox touched by the hitbox
    pub hurtbox: Entity,

    /// The hitbox touching the hurtbox.
    pub hitbox: Entity,
}

pub type OnTagTriggerFn = fn(emd: &mut Emerald, world: &mut World, ctx: OnTagTriggerContext);
pub type GetDeltaFn = fn(emd: &mut Emerald, world: &World) -> f32;
pub type GetDeltaForEntityFn = fn(emd: &mut Emerald, world: &World, id: Entity) -> f32;
pub type OnHitFilterFn = fn(emd: &mut Emerald, world: &mut World, ctx: OnHitFilterContext) -> bool;
pub type OnHitFn = fn(emd: &mut Emerald, world: &mut World, ctx: OnHitContext);

pub struct HitmeConfig {
    /// An alternate method for getting delta aside from `emd.delta()`
    /// Used for calculations and hitbox sequence progression.
    pub alt_get_delta_fn: Option<GetDeltaFn>,

    /// An alternate method for getting delta aside from `emd.delta()` for any given entity.
    /// Used for calculations and hitbox sequence progression.
    /// Ex. An entity is affected by a time slow effect, and progresses slower than usual.
    pub alt_get_delta_for_entity_fn: Option<GetDeltaForEntityFn>,

    /// A list of functions that filter out hits, a hit must pass all filters to succeed.
    pub hit_filter_fns: Vec<OnHitFilterFn>,

    /// A list of callbacks to call when a hitbox successfully hits a hurtbox.
    pub on_hit_fns: Vec<OnHitFn>,

    tag_handlers_by_name: HashMap<String, OnTagTriggerFn>,
    tag_handlers: Vec<OnTagTriggerFn>,
}
impl HitmeConfig {
    pub fn get_delta(&self, emd: &mut Emerald, world: &World) -> f32 {
        self.alt_get_delta_fn
            .map(|f| f(emd, world))
            .unwrap_or(emd.delta())
    }

    pub fn get_delta_for_entity(&self, emd: &mut Emerald, world: &World, id: Entity) -> f32 {
        self.alt_get_delta_for_entity_fn
            .map(|f| f(emd, world, id))
            .unwrap_or(emd.delta())
    }
}
impl Default for HitmeConfig {
    fn default() -> Self {
        Self {
            alt_get_delta_fn: Default::default(),
            alt_get_delta_for_entity_fn: Default::default(),
            tag_handlers: Vec::new(),
            tag_handlers_by_name: HashMap::new(),
            hit_filter_fns: Vec::new(),
            on_hit_fns: Vec::new(),
        }
    }
}

pub fn init(emd: &mut Emerald, config: HitmeConfig) {
    emd.resources().insert(config);
    emd.loader().add_world_merge_handler(merge_handler);
}

pub fn add_on_tag_trigger_by_name<T: Into<String>>(
    emd: &mut Emerald,
    tag: T,
    handler: OnTagTriggerFn,
) {
    emd.resources()
        .get_mut::<HitmeConfig>()
        .map(|config| config.tag_handlers_by_name.insert(tag.into(), handler));
}
pub fn add_on_tag_trigger(emd: &mut Emerald, handler: OnTagTriggerFn) {
    emd.resources()
        .get_mut::<HitmeConfig>()
        .map(|config| config.tag_handlers.push(handler));
}
pub fn emd_hitme_system(emd: &mut Emerald, world: &mut World) {
    let config = emd.resources().remove::<HitmeConfig>().unwrap();
    hitbox_system(emd, world, &config).unwrap();
    let collisions = get_active_hitbox_to_active_hurtbox_collisions(world);
    collisions.into_iter().for_each(|(hitbox_id, hurtboxes)| {
        hurtboxes.into_iter().for_each(|hurtbox| {
            config.on_hit_fns.iter().for_each(|f| {
                get_hurtbox_owner(world, hurtbox).map(|hurtbox_owner| {
                    get_hitbox_owner(world, hitbox_id).map(|hitbox_owner| {
                        let can_damage_hurtbox_owner = world
                            .get::<&Hitbox>(hitbox_id)
                            .ok()
                            .map(|h| h.can_damage_entity(&hurtbox_owner))
                            .unwrap_or(false);

                        let hit = !config.hit_filter_fns.iter().any(|filter_fn| {
                            !filter_fn(
                                emd,
                                world,
                                OnHitFilterContext {
                                    hit_entity: hitbox_owner,
                                    hurt_entity: hurtbox_owner,
                                    hurtbox: hurtbox,
                                    hitbox: hitbox_id,
                                },
                            )
                        });

                        if hit && can_damage_hurtbox_owner {
                            f(
                                emd,
                                world,
                                OnHitContext {
                                    hit_entity: hitbox_owner,
                                    hurt_entity: hurtbox_owner,
                                    hurtbox,
                                    hitbox: hitbox_id,
                                },
                            );
                            add_to_damaged_list(world, hitbox_id, hurtbox_owner);
                        }
                    });
                });
            });
        });
    });

    tracker_system(emd, world, &config);

    emd.resources().insert(config);
}

pub fn add_to_damaged_list(world: &mut World, hitbox_id: Entity, damaged_entity: Entity) {
    world.get::<&mut Hitbox>(hitbox_id).ok().map(|mut h| {
        h.add_damaged_entity(damaged_entity);
    });
}

fn merge_handler(
    new_world: &mut World,
    _old_world: &mut World,
    entity_map: &mut HashMap<Entity, Entity>,
) -> Result<(), EmeraldError> {
    new_world
        .query::<&mut Hitbox>()
        .iter()
        .for_each(|(_, hitbox)| {
            entity_map
                .get(&hitbox.parent_set)
                .map(|e| hitbox.parent_set = e.clone());
        });
    new_world
        .query::<&mut Hurtbox>()
        .iter()
        .for_each(|(_, hurtbox)| {
            entity_map
                .get(&hurtbox.parent_set)
                .map(|e| hurtbox.parent_set = e.clone());
        });
    new_world
        .query::<&mut HurtboxSet>()
        .iter()
        .for_each(|(_, hurtbox_set)| {
            let old_hurtbox_ids = hurtbox_set.hurtboxes.clone();
            old_hurtbox_ids.into_iter().for_each(|h| {
                entity_map.get(&h).map(|e| {
                    hurtbox_set
                        .hurtboxes
                        .contains(&e)
                        .then(|| hurtbox_set.hurtboxes.push(e.clone()))
                });
            });
            entity_map
                .get(&hurtbox_set.owner)
                .map(|e| hurtbox_set.owner = e.clone());
        });
    new_world
        .query::<&mut HitboxSet>()
        .iter()
        .for_each(|(_, hitbox_set)| {
            let old_hitbox_ids = hitbox_set.hitboxes.clone();
            old_hitbox_ids.into_iter().for_each(|(name, h)| {
                entity_map.get(&h).map(|e| {
                    hitbox_set.hitboxes.insert(name, e.clone());
                });
            });
            entity_map.get(&hitbox_set.owner).map(|e| {
                hitbox_set.owner = e.clone();
            });
        });
    new_world
        .query::<&mut SimpleTranslationTracker>()
        .iter()
        .for_each(|(_, tracker)| {
            entity_map
                .get(&tracker.target)
                .map(|e| tracker.target = e.clone());
        });
    Ok(())
}

/// Returns a map of active hitboxes and active hurtboxes they are colliding with.
pub fn get_active_hitbox_to_active_hurtbox_collisions(
    world: &mut World,
) -> HashMap<Entity, Vec<Entity>> {
    let active_hitboxes = get_all_active_hitboxes(world);
    let mut hitbox_hurtbox_collisions: HashMap<Entity, HashSet<Entity>> = HashMap::new();
    for hitbox_id in active_hitboxes {
        let colliding_hurtboxes = get_colliding_active_hurtboxes(world, hitbox_id)
            .into_iter()
            .filter(|hurtbox_id| {
                let hurtbox_parent_set = world
                    .get::<&Hurtbox>(hurtbox_id.clone())
                    .unwrap()
                    .parent_set
                    .clone();
                let hurtbox_set_owner = world
                    .get::<&HurtboxSet>(hurtbox_parent_set)
                    .unwrap()
                    .owner
                    .clone();

                let hitbox_parent_set = world.get::<&Hitbox>(hitbox_id).unwrap().parent_set.clone();
                let hitbox_set_owner = world
                    .get::<&HitboxSet>(hitbox_parent_set)
                    .unwrap()
                    .owner
                    .clone();

                let can_damage_hurtbox_owner = world
                    .get::<&Hitbox>(hitbox_id)
                    .unwrap()
                    .can_damage_entity(&hurtbox_set_owner);
                let same_owner = hitbox_set_owner == hurtbox_set_owner;

                !same_owner && can_damage_hurtbox_owner
            })
            .collect::<HashSet<Entity>>();

        if !hitbox_hurtbox_collisions.contains_key(&hitbox_id) {
            hitbox_hurtbox_collisions.insert(hitbox_id, HashSet::new());
        }

        if let Some(collisions) = hitbox_hurtbox_collisions.get_mut(&hitbox_id) {
            for id in colliding_hurtboxes {
                collisions.insert(id);
            }
        }
    }

    hitbox_hurtbox_collisions
        .into_iter()
        .map(|(hitbox_id, hurtbox_set)| {
            let hurtboxes = hurtbox_set.into_iter().collect::<Vec<Entity>>();
            (hitbox_id, hurtboxes)
        })
        .collect()
}
