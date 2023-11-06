use std::collections::{HashMap, HashSet};

use crate::hurtboxes::RectCollider;
use crate::tracker::SimpleTranslationTracker;
use crate::{HitmeConfig, OnTagTriggerContext};
use emerald::serde::Deserialize;
use emerald::toml::Value;
use emerald::{
    toml::value::Map, ColliderHandle, EmeraldError, Entity, RigidBodyBuilder, Transform, Vector2,
    World,
};
use emerald::{Emerald, Group, InteractionGroups, Translation};

/// A series of hitboxes that act as one.
/// If the set is disabled, it's children will not be considered for combat.
#[derive(Debug)]
pub struct HitboxSet {
    // Hitboxes by name
    pub hitboxes: HashMap<String, Entity>,
    pub owner: Entity,
    pub sequences: HashMap<String, Vec<HitboxSequenceFrame>>,
    pub active_sequence: Option<ActiveSequenceData>,
}
impl HitboxSet {
    pub fn from_toml(
        world: &mut World,
        value: &emerald::toml::Value,
        owner: Entity,
        hurtbox_group: Group,
        hitbox_group: Group,
    ) -> Result<Self, EmeraldError> {
        let default = emerald::toml::Value::Table(Map::new());
        let default_map = Map::new();
        let hitboxes_table = value
            .get("hitboxes")
            .unwrap_or(&default)
            .as_table()
            .unwrap_or(&default_map);
        let owner_transform = world.get::<&mut Transform>(owner)?.clone();
        let hitboxes = hitboxes_table
            .into_iter()
            .map(|(key, value)| {
                let hitbox = Hitbox::from_toml(world, value, owner)?;
                let colliders = hitbox.raw_collider_data.clone();
                let (id, rbh) = world.spawn_with_body(
                    (
                        hitbox,
                        owner_transform.clone(),
                        SimpleTranslationTracker {
                            target: owner,
                            offset: Translation::new(0.0, 0.0),
                        },
                    ),
                    RigidBodyBuilder::dynamic(),
                )?;
                for collider in colliders {
                    let name = collider.name.clone();
                    let builder = collider
                        .to_collider_builder()
                        .collision_groups(InteractionGroups::new(hitbox_group, hurtbox_group));
                    let handle = world.physics().build_collider(rbh, builder);

                    if let Some(collider_name) = name {
                        world
                            .get::<&mut Hitbox>(id)?
                            .colliders
                            .insert(collider_name, handle);
                    }
                }

                Ok((key.clone(), id))
            })
            .collect::<Result<HashMap<String, Entity>, EmeraldError>>()?;

        let mut sequences = HashMap::new();
        if let Some(s) = value.get("sequences") {
            if let Some(table) = s.as_table() {
                for (key, value) in table {
                    let mut frames = Vec::new();

                    if let Some(arr) = value.as_array() {
                        for v in arr {
                            if let Ok(sequence) =
                                emerald::toml::from_str::<HitboxSequenceFrame>(&v.to_string())
                            {
                                frames.push(sequence);
                            }
                        }

                        sequences.insert(key.clone(), frames);
                    }
                }
            }
        }

        Ok(Self {
            hitboxes,
            owner,
            sequences,
            active_sequence: None,
        })
    }

    pub fn start_sequence<T: Into<String>>(
        &mut self,
        sequence_name: T,
    ) -> Result<(), EmeraldError> {
        let name: String = sequence_name.into();
        if !self.has_sequence(&name) {
            return Err(EmeraldError::new(format!(
                "Hitbox set does not have sequence {}",
                &name
            )));
        }

        let sequence = ActiveSequenceData::new(name);
        self.active_sequence = Some(sequence);
        self.reset_sequences();

        Ok(())
    }

    pub fn has_sequence<'a, T: Into<&'a String>>(&self, name: T) -> bool {
        self.sequences.contains_key(name.into())
    }

    pub fn progress_active_sequence(&mut self, delta: f32) -> Vec<HitboxSequenceEvent> {
        self.active_sequence
            .as_mut()
            .map(|sequence| sequence.progress(&mut self.sequences, &self.hitboxes, delta))
            .unwrap_or_default()
    }

    pub fn get_current_sequence_frame(&mut self) -> Option<&HitboxSequenceFrame> {
        if let Some(active_sequence) = &self.active_sequence {
            if let Some(frames) = &self.sequences.get(&active_sequence.name) {
                return frames.get(active_sequence.frame);
            }
        }

        None
    }

    pub fn reset_sequences(&mut self) {
        self.sequences.iter_mut().for_each(|(_, frames)| {
            frames.iter_mut().for_each(|f| f.reset());
        });
    }

    /// If there is an active sequence, returns if its finjished
    pub fn is_current_sequence_finished(&self) -> Option<bool> {
        self.active_sequence
            .as_ref()
            .map(|active_sequence| active_sequence.is_finished(&self.sequences))
    }
}

fn default_tag_data() -> Value {
    Value::Table(emerald::toml::map::Map::new())
}

#[derive(Debug, Deserialize)]
#[serde(crate = "emerald::serde")]
pub struct HitboxSequenceFrameTag {
    #[serde(default)]
    pub triggered: bool,

    #[serde(default)]
    pub name: String,

    /// How long after the frame started, to emit the tag
    #[serde(default)]
    pub delay: f32,

    #[serde(default = "default_tag_data")]
    pub data: emerald::toml::Value,
}

#[derive(Debug, Deserialize)]
#[serde(crate = "emerald::serde")]
pub struct HitboxSequenceFrame {
    /// Time limit for the frame, before it moves onto the next frame
    #[serde(default)]
    pub duration: f32,

    /// Name of the collider to activate
    pub name: Option<String>,

    /// Name of the colliders to activate
    pub names: Option<Vec<String>>,

    /// How long this current frame should wait before activating
    #[serde(default)]
    pub delay: f32,

    /// Tags bound this frame, often used as "triggers" for other effects
    #[serde(default)]
    tags: Vec<HitboxSequenceFrameTag>,

    #[serde(default)]
    active: bool,
}
impl HitboxSequenceFrame {
    pub fn reset(&mut self) {
        self.tags.iter_mut().for_each(|tag| tag.triggered = false);
        self.active = false;
    }

    pub fn get_hitboxes(&self, hitboxes: &HashMap<String, Entity>) -> Vec<Entity> {
        let mut entities = Vec::new();

        if let Some(name) = &self.name {
            if let Some(e) = hitboxes.get(name) {
                entities.push(e.clone());
            }
        }

        if let Some(names) = &self.names {
            for name in names {
                if let Some(e) = hitboxes.get(name) {
                    entities.push(e.clone());
                }
            }
        }

        entities
    }
}

#[derive(Debug)]
pub enum HitboxSequenceEvent {
    HitboxDeactivated { hitbox: Entity },
    HitboxActivated { hitbox: Entity },
    TagTriggered { name: String, data: Value },
    Finished,
}
impl HitboxSequenceEvent {
    pub fn get_activated_hitboxes(events: &Vec<HitboxSequenceEvent>) -> Vec<Entity> {
        events
            .iter()
            .filter_map(|e| match e {
                HitboxSequenceEvent::HitboxActivated { hitbox } => Some(hitbox.clone()),
                _ => None,
            })
            .collect()
    }

    pub fn get_deactivated_hitboxes(events: &Vec<HitboxSequenceEvent>) -> Vec<Entity> {
        events
            .iter()
            .filter_map(|e| match e {
                HitboxSequenceEvent::HitboxDeactivated { hitbox } => Some(hitbox.clone()),
                _ => None,
            })
            .collect()
    }
}

pub fn activate_hitbox_sequence(world: &mut World, id: Entity, sequence: &str) {
    world
        .get::<&mut HitboxSet>(id)
        .ok()
        .map(|mut h| h.start_sequence(sequence).ok());
}
#[derive(Debug)]
pub struct ActiveSequenceData {
    /// Name of the active sequence
    pub name: String,
    pub frame: usize,
    pub elapsed_time: f32,
}
impl ActiveSequenceData {
    pub fn new(name: String) -> Self {
        Self {
            name,
            frame: 0,
            elapsed_time: 0.0,
        }
    }

    pub fn get_current_active_hitboxes(
        &self,
        sequences: &HashMap<String, Vec<HitboxSequenceFrame>>,
        hitboxes: &HashMap<String, Entity>,
    ) -> Vec<Entity> {
        let mut entities = Vec::new();

        if let Some(frames) = sequences.get(&self.name) {
            if let Some(frame) = frames.get(self.frame) {
                entities.extend(frame.get_hitboxes(hitboxes));
            }
        }

        entities
    }

    pub fn get_future_hitboxes_to_be_activated(
        &self,
        sequences: &HashMap<String, Vec<HitboxSequenceFrame>>,
        hitboxes: &HashMap<String, Entity>,
    ) -> Vec<Entity> {
        let mut entities = Vec::new();

        if let Some(frames) = sequences.get(&self.name) {
            let total_frame_count = frames.len();
            let diff = total_frame_count - self.frame;

            if diff == 0 {
                return entities;
            }

            for i in 1..diff {
                if let Some(frame) = frames.get(self.frame + i) {
                    entities.extend(frame.get_hitboxes(hitboxes));
                }
            }
        }

        entities
    }

    pub fn is_finished(&self, sequences: &HashMap<String, Vec<HitboxSequenceFrame>>) -> bool {
        let (last_frame, last_frame_limit) = sequences
            .get(&self.name)
            .map(|frames| {
                (
                    frames.len() - 1,
                    frames.last().map(|f| f.duration).unwrap_or(0.0),
                )
            })
            .unwrap_or((0, 0.0));

        self.frame == last_frame && self.elapsed_time >= last_frame_limit
    }

    pub fn is_current_frame_active(
        &self,
        sequences: &mut HashMap<String, Vec<HitboxSequenceFrame>>,
    ) -> bool {
        sequences
            .get(&self.name)
            .map(|frames| frames.get(self.frame).map(|f| f.active).unwrap_or(false))
            .unwrap_or(false)
    }

    pub fn progress(
        &mut self,
        sequences: &mut HashMap<String, Vec<HitboxSequenceFrame>>,
        hitboxes: &HashMap<String, Entity>,
        delta: f32,
    ) -> Vec<HitboxSequenceEvent> {
        let mut events = Vec::new();

        let delay = sequences
            .get(&self.name)
            .map(|frames| frames.get(self.frame).map(|f| f.delay))
            .flatten()
            .unwrap_or(0.0);
        self.elapsed_time += delta;

        // First frame, activate hitboxes
        if self.elapsed_time >= delay && !self.is_current_frame_active(sequences) {
            self.activate_current_frame(sequences, hitboxes, &mut events);
        }

        if let Some(frames) = sequences.get_mut(&self.name) {
            if let Some(frame) = frames.get_mut(self.frame) {
                frame.tags.iter_mut().for_each(|tag| {
                    if self.elapsed_time >= tag.delay + delay && !tag.triggered {
                        tag.triggered = true;
                        events.push(HitboxSequenceEvent::TagTriggered {
                            name: tag.name.clone(),
                            data: tag.data.clone(),
                        });
                    }
                });

                if self.elapsed_time >= frame.duration + delay {
                    self.deactivate_current_frame(sequences, hitboxes, &mut events);

                    self.elapsed_time = 0.0;
                    self.reset_current_frame(sequences);
                    self.frame += 1;

                    get_sequence_frame_count(sequences, &self.name).map(|count| {
                        if self.frame >= count {
                            events.push(HitboxSequenceEvent::Finished);
                        }
                    });
                }
            }
        }

        events
    }

    fn reset_current_frame(&mut self, sequences: &mut HashMap<String, Vec<HitboxSequenceFrame>>) {
        sequences
            .get_mut(&self.name)
            .map(|frames| frames.get_mut(self.frame).map(|f| f.reset()));
    }

    pub fn activate_current_frame(
        &self,
        sequences: &mut HashMap<String, Vec<HitboxSequenceFrame>>,
        hitboxes: &HashMap<String, Entity>,
        events: &mut Vec<HitboxSequenceEvent>,
    ) {
        events.extend(
            self.get_current_active_hitboxes(sequences, hitboxes)
                .into_iter()
                .map(|e| HitboxSequenceEvent::HitboxActivated { hitbox: e })
                .collect::<Vec<HitboxSequenceEvent>>(),
        );
        sequences
            .get_mut(&self.name)
            .map(|frames| frames.get_mut(self.frame).map(|f| f.active = true));
    }
    pub fn deactivate_current_frame(
        &self,
        sequences: &mut HashMap<String, Vec<HitboxSequenceFrame>>,
        hitboxes: &HashMap<String, Entity>,
        events: &mut Vec<HitboxSequenceEvent>,
    ) {
        events.extend(
            self.get_current_active_hitboxes(sequences, hitboxes)
                .into_iter()
                .map(|e| HitboxSequenceEvent::HitboxDeactivated { hitbox: e })
                .collect::<Vec<HitboxSequenceEvent>>(),
        );
        sequences
            .get_mut(&self.name)
            .map(|frames| frames.get_mut(self.frame).map(|f| f.active = false));
    }
}

pub fn get_sequence_frame_count<T: Into<String>>(
    sequences: &HashMap<String, Vec<HitboxSequenceFrame>>,
    name: T,
) -> Option<usize> {
    let name: String = name.into();
    sequences.get(&name).map(|frames| frames.len())
}

pub fn get_hitbox_owner(world: &World, hitbox: Entity) -> Option<Entity> {
    world
        .get::<&Hitbox>(hitbox)
        .ok()
        .map(|h| h.parent_set.clone())
        .map(|p| {
            world
                .get::<&HitboxSet>(p)
                .ok()
                .map(|hitbox_set| hitbox_set.owner.clone())
        })
        .flatten()
}

pub fn is_hitbox_owner(world: &World, id: Entity, hitbox_id: Entity) -> bool {
    get_hitbox_owner(world, hitbox_id)
        .map(|owner| owner == id)
        .unwrap_or(false)
}

pub struct Hitbox {
    active: bool,

    /// One time hitbox activation trigger, useful for spawned bullets/hitbox ents
    activate_after: Option<f32>,

    /// One time hitbox deactivation trigger, useful for spawned bullets/hitbox ents
    deactivate_after: Option<f32>,

    elapsed_time: f32,

    pub parent_set: Entity,
    pub raw_collider_data: Vec<RectCollider>,
    pub colliders: HashMap<String, ColliderHandle>,

    /// How much time must progress before the hitbox is allowed to damage the same entity twice
    cooldown_per_entity: Option<f32>,

    /// Entities that have been damaged by this hitbox, and how much time has elapsed since they've been hit
    pub damaged_entities: HashMap<Entity, f32>,
}
impl Hitbox {
    pub fn from_toml(
        world: &World,
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

        let activate_after = value
            .get("activate_after")
            .map(|v| v.as_float())
            .flatten()
            .map(|f| f as f32);

        let deactivate_after = value
            .get("deactivate_after")
            .map(|v| v.as_float())
            .flatten()
            .map(|f| f as f32);

        // default to 1 second
        let mut cooldown_per_entity = Some(1.0);

        if let Some(cd) = value.get("cooldown_per_entity") {
            if let Some(n) = cd.as_float() {
                cooldown_per_entity = Some(n as f32);
            }
        }

        Ok(Self {
            parent_set,
            colliders: HashMap::new(),
            raw_collider_data: colliders,
            active,
            damaged_entities: HashMap::new(),
            activate_after,
            deactivate_after,
            cooldown_per_entity,
            elapsed_time: 0.0,
        })
    }

    pub fn is_one_time(&self) -> bool {
        self.activate_after.is_some() || self.deactivate_after.is_some()
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn deactivate(&mut self) {
        self.active = false;
    }

    pub fn activate(&mut self) {
        if self.active {
            return;
        }
        self.active = true;
    }

    /// Refreshes the hitbox, clearing damaged entities
    pub fn refresh(&mut self) {
        self.damaged_entities = HashMap::new();
    }

    pub fn can_damage_entity(&self, other_entity: &Entity) -> bool {
        if let Some(delta) = self.damaged_entities.get(other_entity) {
            if let Some(cd) = &self.cooldown_per_entity {
                return delta >= cd;
            }

            true
        } else {
            true
        }
    }

    pub fn add_damaged_entity(&mut self, entity: Entity) {
        self.add_damaged_entities([entity].to_vec());
    }

    pub fn add_damaged_entities(&mut self, entities: Vec<Entity>) {
        for id in entities {
            self.damaged_entities.insert(id, 0.0);
        }
    }
}

pub fn refresh_hitboxes(world: &mut World, id: Entity) {
    let mut hitboxes = Vec::new();
    if let Ok(set) = world.get::<&HitboxSet>(id) {
        for (_, id) in &set.hitboxes {
            hitboxes.push(id.clone());
        }
    }

    if world.has::<Hitbox>(id) {
        hitboxes.push(id);
    }

    for id in hitboxes {
        world.get::<&mut Hitbox>(id).map(|mut h| h.refresh()).ok();
    }
}

#[derive(Clone)]
pub enum StatusEffect {
    Stun,
    Poison,
}

pub fn get_all_active_hitboxes(world: &World) -> Vec<Entity> {
    world
        .query::<&Hitbox>()
        .iter()
        .filter_map(|(id, hitbox)| hitbox.active.then(|| id))
        .collect()
}

/// Updates hitboxes
pub fn hitbox_system(
    emd: &mut Emerald,
    world: &mut World,
    config: &HitmeConfig,
) -> Result<(), EmeraldError> {
    hitbox_one_time_system(emd, world, config)?;
    hitbox_damaged_entity_delta_system(emd, world, config);
    hitbox_sequence_system(emd, world, config)?;

    Ok(())
}

fn hitbox_damaged_entity_delta_system(emd: &mut Emerald, world: &mut World, config: &HitmeConfig) {
    let delta = config.get_delta(emd, world);

    for (_, hitbox) in world.query::<&mut Hitbox>().iter() {
        for (_, e_d) in &mut hitbox.damaged_entities {
            *e_d = *e_d + delta;
        }
    }
}

fn hitbox_one_time_system(
    emd: &mut Emerald,
    world: &mut World,
    config: &HitmeConfig,
) -> Result<(), EmeraldError> {
    for (id, hitbox) in world
        .query::<&mut Hitbox>()
        .iter()
        .filter(|(_, h)| h.is_one_time())
    {
        hitbox.elapsed_time += config.get_delta_for_entity(emd, world, id);

        if let Some(trigger) = &hitbox.activate_after {
            if &hitbox.elapsed_time >= trigger {
                hitbox.activate();
                hitbox.activate_after.take();
            }
        } else {
            if let Some(trigger) = &hitbox.deactivate_after {
                if &hitbox.elapsed_time >= trigger {
                    hitbox.deactivate();
                    hitbox.deactivate_after.take();
                }
            }
        }
    }
    Ok(())
}

/// Updates hitbox sequences.
/// Deactivates hitboxes associated with a finished frame.
/// Activates hitboxes associated with a starting frame.
fn hitbox_sequence_system(
    emd: &mut Emerald,
    world: &mut World,
    config: &HitmeConfig,
) -> Result<(), EmeraldError> {
    let mut to_deactivate = Vec::new();
    let mut to_activate = Vec::new();
    let mut tag_triggers = Vec::new();

    for (id, hitbox_set) in world.query::<&mut HitboxSet>().iter() {
        if hitbox_set.active_sequence.is_none() {
            continue;
        }

        let delta = config.get_delta_for_entity(emd, world, id);

        let sequence_events = hitbox_set.progress_active_sequence(delta);
        for event in sequence_events {
            match event {
                HitboxSequenceEvent::HitboxDeactivated { hitbox } => {
                    to_deactivate.push(hitbox);
                }
                HitboxSequenceEvent::HitboxActivated { hitbox } => {
                    to_activate.push(hitbox);
                }
                HitboxSequenceEvent::Finished => {
                    hitbox_set.active_sequence = None;
                }
                HitboxSequenceEvent::TagTriggered { name, data } => {
                    tag_triggers.push((name, id, data));
                }
            }
        }
    }

    for (tag, hitbox_set_owner, data) in tag_triggers {
        let mut handlers = config.tag_handlers.clone();

        config.tag_handlers_by_name.get(&tag).map(|f| {
            handlers.push(*f);
        });

        for f in handlers {
            f(
                emd,
                world,
                OnTagTriggerContext {
                    tag: tag.clone(),
                    hitbox_set_owner,
                    data: data.clone(),
                },
            )
        }
    }

    for id in to_activate {
        world.get::<&mut Hitbox>(id).ok().map(|mut hitbox| {
            hitbox.activate();
        });
    }

    for id in to_deactivate {
        world.get::<&mut Hitbox>(id).ok().map(|mut hitbox| {
            hitbox.deactivate();
            hitbox.refresh();
        });
    }

    Ok(())
}

#[cfg(test)]
mod sequence_tests {

    use std::collections::HashMap;

    use emerald::{Entity, Transform, World};

    use crate::{
        emd_hitme_system,
        hitboxes::{ActiveSequenceData, HitboxSequenceEvent, HitboxSequenceFrame},
    };

    const TEST_SEQUENCE_NAME: &str = "test";
    const HITBOX_ENTITY_NAME: &str = "hitbox";

    fn get_test_package() -> (
        ActiveSequenceData,
        HashMap<String, Vec<HitboxSequenceFrame>>,
        HashMap<String, Entity>,
    ) {
        let mut world = World::new();
        let test_sequence_name = String::from(TEST_SEQUENCE_NAME);
        let hitbox_name = String::from(HITBOX_ENTITY_NAME);
        let mut sequences = HashMap::new();
        let mut hitboxes = HashMap::new();
        let sequence_frames = vec![HitboxSequenceFrame {
            duration: 2.0,
            name: Some(hitbox_name.clone()),
            names: None,
            delay: 0.0,
            tags: Vec::new(),
            active: false,
        }];

        let hitbox_entity = world.spawn((Transform::default(),));
        hitboxes.insert(hitbox_name.clone(), hitbox_entity.clone());
        sequences.insert(test_sequence_name.clone(), sequence_frames);

        let active_sequence = ActiveSequenceData::new(test_sequence_name.clone());

        (active_sequence, sequences, hitboxes)
    }

    #[test]
    fn first_frame_activates_hitboxes() {
        let (mut active_sequence, mut sequences, hitboxes) = get_test_package();
        let events = active_sequence.progress(&mut sequences, &hitboxes, 0.016);

        assert_eq!(events.len(), 1);
        let hitbox_entity = hitboxes.get(HITBOX_ENTITY_NAME).unwrap().clone();
        assert_eq!(
            hitbox_entity,
            HitboxSequenceEvent::get_activated_hitboxes(&events).remove(0)
        );
    }

    #[test]
    fn first_frame_respects_delay() {
        let (mut active_sequence, mut sequences, hitboxes) = get_test_package();
        sequences.get_mut(TEST_SEQUENCE_NAME).unwrap()[0].delay = 0.2;

        let events = active_sequence.progress(&mut sequences, &hitboxes, 0.016);

        assert_eq!(events.len(), 0);
        assert_eq!(
            0,
            HitboxSequenceEvent::get_activated_hitboxes(&events).len()
        );
        let events = active_sequence.progress(&mut sequences, &hitboxes, 0.2);

        assert_eq!(events.len(), 1);
        let hitbox_entity = hitboxes.get(HITBOX_ENTITY_NAME).unwrap().clone();
        assert_eq!(
            hitbox_entity,
            HitboxSequenceEvent::get_activated_hitboxes(&events).remove(0)
        );
    }

    #[test]
    fn progressing_past_limit_deactivates_hitboxes() {
        let (mut active_sequence, mut sequences, hitboxes) = get_test_package();
        let events = active_sequence.progress(&mut sequences, &hitboxes, 40.0);

        let hitbox_entity = hitboxes.get(HITBOX_ENTITY_NAME).unwrap().clone();
        assert_eq!(
            hitbox_entity,
            HitboxSequenceEvent::get_deactivated_hitboxes(&events).remove(0)
        );
    }

    #[test]
    fn attack_sequence_can_only_deal_one_instance_of_damage_with_multiple_hitboxes() {}

    #[test]
    fn progressing_past_limit_of_all_frames_finishes_sequence() {
        let (mut active_sequence, mut sequences, hitboxes) = get_test_package();
        let events = active_sequence.progress(&mut sequences, &hitboxes, 40.0);
        assert!(
            events
                .into_iter()
                .filter(|e| match e {
                    HitboxSequenceEvent::Finished => true,
                    _ => false,
                })
                .collect::<Vec<HitboxSequenceEvent>>()
                .len()
                == 1
        );
    }
}
