use glam::{Mat3, Vec3};
use mod_api::{ClientHost, Hand, HandRender};

pub(crate) struct Pose {
    pub translation: [f32; 3],
    pub rotation: [f32; 3],
    pub scale: f32,
}

impl Pose {
    pub(crate) fn is_identity(&self) -> bool {
        self.translation == [0.0; 3] && self.rotation == [0.0; 3] && self.scale == 1.0
    }

    pub(crate) fn needs_pivot(&self) -> bool {
        self.scale != 1.0 || self.translation[2] != 0.0
    }

    pub(crate) fn with_pivot(&self, pivot: Vec3) -> [f32; 3] {
        let [x, y, z] = self.translation;
        let shift = Vec3::new(x, y, 0.0);
        let anchor = if self.rotation == [0.0; 3] {
            pivot
        } else {
            let [pitch, yaw, roll] = self.rotation.map(f32::to_radians);
            Mat3::from_rotation_x(pitch)
                * Mat3::from_rotation_y(yaw)
                * Mat3::from_rotation_z(roll)
                * pivot
        };

        // The hook scales around the camera. Undo the scale of the hand's
        // anchor so geometry grows around the hand under perspective.
        let scale_offset = (1.0 - self.scale) * anchor;
        // Keep the anchor on its viewing ray and reserve most of its depth.
        let depth_fraction = 0.24 * z / (1.0 + z.abs());
        (shift + scale_offset - depth_fraction * (anchor + shift))
            .clamp(Vec3::splat(-16.0), Vec3::splat(16.0))
            .to_array()
    }
}

pub(crate) fn hand_pivot(host: &dyn ClientHost, hand: &HandRender<'_>, is_map: bool) -> Vec3 {
    let left = host.gui_options().main_hand_left != (hand.hand == Hand::Off);
    let side = if left { -1.0 } else { 1.0 };
    // The SDK exposes the hand pose before the renderer resolves its model.
    // Use the resting center of the vanilla geometry as the scale pivot.
    if is_map {
        let two_handed = hand.hand == Hand::Main
            && host.local_player().is_some_and(|player| {
                host.living(player.entity())
                    .is_some_and(|living| living.hand_item(host, Hand::Off).is_none())
            });
        if two_handed {
            let tilt = (1.0 - hand.interpolated_pitch / 45.0 + 0.1).clamp(0.0, 1.0);
            let tilt = (1.0 - (tilt * std::f32::consts::PI).cos()) / 2.0;
            return Vec3::new(0.0, 0.04 - hand.equip * 1.2 - tilt * 0.5, -0.72);
        }
        return Vec3::new(side * 0.635, -0.205 - hand.equip * 1.2, -0.75);
    }
    if hand.stack.is_none() {
        Vec3::new(side * 0.64, -0.60 - hand.equip * 0.6, -0.72)
    } else {
        Vec3::new(side * 0.63, -0.32 - hand.equip * 0.6, -0.65)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ANCHOR: Vec3 = Vec3::new(0.63, -0.32, -0.65);

    fn projected(point: Vec3) -> glam::Vec2 {
        point.truncate() / -point.z
    }

    #[test]
    fn scale_changes_projected_size_without_moving_the_anchor() {
        for scale in [0.1, 0.5, 1.0, 1.5, 5.0] {
            let pose = Pose {
                translation: [0.0; 3],
                rotation: [0.0; 3],
                scale,
            };
            let offset = Vec3::from_array(pose.with_pivot(ANCHOR));
            let center = offset + scale * ANCHOR;
            let edge = offset + scale * (ANCHOR + Vec3::X * 0.1);
            assert!(center.abs_diff_eq(ANCHOR, 1e-6));
            let delta = projected(edge).x - projected(center).x;
            assert!((delta - scale * 0.1 / -ANCHOR.z).abs() < 1e-6);
        }
    }

    #[test]
    fn extreme_depth_keeps_the_translated_anchor_on_its_viewing_ray() {
        let mut previous_z = f32::NEG_INFINITY;
        for z in [-16.0, -2.5, 0.0, 2.5, 16.0] {
            let pose = Pose {
                translation: [-0.1, 0.2, z],
                rotation: [0.0; 3],
                scale: 1.0,
            };
            let center = ANCHOR + Vec3::from_array(pose.with_pivot(ANCHOR));
            assert!(center.z > previous_z && center.z < -0.49 && center.z > -0.81);
            assert!(
                projected(center).abs_diff_eq(projected(ANCHOR + Vec3::new(-0.1, 0.2, 0.0)), 1e-6,)
            );
            previous_z = center.z;
        }
    }
}
