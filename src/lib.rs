use std::collections::HashMap;

use emerald::{toml::Value, Emerald, Entity, World};
use hitboxes::{hitbox_system, HitboxSequenceEvent};

pub mod component_loader;
pub mod hitboxes;
pub mod hurtboxes;

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
    emd.resources().insert(config);
}
