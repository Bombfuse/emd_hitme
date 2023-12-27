use emerald::{Color, ColorRect, Emerald, Transform, Vector2, World};

use crate::{hitboxes::Hitbox, hurtboxes::Hurtbox};

pub fn draw_debug(emd: &mut Emerald, world: &World, color: &Color) {
    let mut color_rect = ColorRect::new(color.clone(), 0, 0);
    for (_, (transform, hurtbox)) in world.query::<(&Transform, &Hurtbox)>().iter() {
        if !hurtbox.visible {
            continue;
        }

        for collider in &hurtbox.colliders {
            color_rect.width = collider.width as u32;
            color_rect.height = collider.height as u32;
            color_rect.offset = Vector2::new(collider.translation.x, collider.translation.y);
            emd.graphics().draw_color_rect(&color_rect, &transform).ok();
        }
    }

    for (_, (transform, hitbox)) in world.query::<(&Transform, &Hitbox)>().iter() {
        if !hitbox.visible {
            continue;
        }

        for collider in &hitbox.raw_collider_data {
            color_rect.width = collider.width as u32;
            color_rect.height = collider.height as u32;
            color_rect.offset = Vector2::new(collider.translation.x, collider.translation.y);
            emd.graphics().draw_color_rect(&color_rect, &transform).ok();
        }
    }
}
