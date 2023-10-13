use std::collections::HashMap;

use emerald::{toml::Value, Emerald, EmeraldError, Entity, World};
use hitboxes::{hitbox_system, Hitbox, HitboxSet};
use hurtboxes::{Hurtbox, HurtboxSet};
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

pub type OnTagTriggerFn = fn(emd: &mut Emerald, world: &mut World, ctx: OnTagTriggerContext);
pub type GetDeltaFn = fn(emd: &mut Emerald, world: &World) -> f32;
pub type GetDeltaForEntityFn = fn(emd: &mut Emerald, world: &World, id: Entity) -> f32;

pub struct HitmeConfig {
    /// An alternate method for getting delta aside from `emd.delta()`
    /// Used for calculations and hitbox sequence progression.
    pub alt_get_delta_fn: Option<GetDeltaFn>,

    /// An alternate method for getting delta aside from `emd.delta()` for any given entity.
    /// Used for calculations and hitbox sequence progression.
    /// Ex. An entity is affected by a time slow effect, and progresses slower than usual.
    pub alt_get_delta_for_entity_fn: Option<GetDeltaForEntityFn>,

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
    tracker_system(emd, world, &config);
    emd.resources().insert(config);
}

fn merge_handler(
    new_world: &mut World,
    _old_world: &mut World,
    entity_map: &mut HashMap<Entity, Entity>,
) -> Result<(), EmeraldError> {
    println!("merging");
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
            entity_map
                .get(&hurtbox_set.owner)
                .map(|e| hurtbox_set.owner = e.clone());
        });
    new_world
        .query::<&mut HitboxSet>()
        .iter()
        .for_each(|(_, hitbox_set)| {
            entity_map.get(&hitbox_set.owner).map(|e| {
                hitbox_set.owner = e.clone();
                println!("updated");
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
