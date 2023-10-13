use std::ops::Deref;

use emerald::{Emerald, Entity, Transform, Translation, World};

use crate::HitmeConfig;

#[derive(Clone, Debug)]
pub(crate) struct SimpleTranslationTracker {
    pub target: Entity,
    pub offset: Translation,
}
pub(crate) fn tracker_system(emd: &mut Emerald, world: &mut World, config: &HitmeConfig) {
    let mut to_destroy = Vec::new();
    world
        .query::<(&SimpleTranslationTracker, &mut Transform)>()
        .iter()
        .filter_map(|(id, (tracker, transform))| {
            if world.contains(tracker.target) && world.has::<Transform>(tracker.target) {
                Some((tracker, transform))
            } else {
                to_destroy.push(id);
                None
            }
        })
        .for_each(|(tracker, transform)| {
            let target_transform = world
                .get::<&Transform>(tracker.target)
                .unwrap()
                .deref()
                .clone();

            *transform = target_transform + Transform::from_translation(tracker.offset);
        });

    to_destroy.into_iter().for_each(|id| {
        world.despawn(id).ok();
    });
}
