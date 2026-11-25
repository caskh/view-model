use std::collections::VecDeque;

use mod_api::{ClientHost, Hand, Registrar, SettingSpec};

use crate::pose::Pose;

pub(crate) const TARGET_KEY: &str = "target";
pub(crate) const HIDE_ARMS_KEY: &str = "hide_arms";
pub(crate) const TARGET_LABELS: [&str; 3] = [
    "viewmodel.target.both",
    "viewmodel.target.items",
    "viewmodel.target.arms",
];

#[derive(Clone, Copy)]
pub(crate) enum Target {
    Both,
    Items,
    Arms,
}

impl Target {
    pub(crate) fn from_index(index: u32) -> Self {
        match index {
            1 => Self::Items,
            2 => Self::Arms,
            _ => Self::Both,
        }
    }

    pub(crate) fn index(self) -> usize {
        self as usize
    }
}

pub(crate) struct Field {
    pub label: &'static str,
    keys: [&'static str; 2],
    pub min: f64,
    pub max: f64,
    pub step: f64,
    pub default: f64,
}

impl Field {
    pub(crate) fn key(&self, hand: Hand) -> &'static str {
        self.keys[hand_index(hand)]
    }

    pub(crate) fn snap(&self, value: f64) -> f32 {
        if !value.is_finite() {
            return self.default as f32;
        }
        let steps = ((value.clamp(self.min, self.max) - self.min) / self.step).round();
        let rounded = self.min + steps * self.step;
        ((rounded * 1000.0).round() / 1000.0).clamp(self.min, self.max) as f32
    }

    fn spec(&self, hand: Hand) -> SettingSpec {
        let label = format!("viewmodel.setting.{}", self.key(hand));
        SettingSpec::range(
            self.key(hand),
            label,
            self.min,
            self.max,
            self.step,
            self.default,
        )
        .tooltip("viewmodel.setting.editor_hint")
    }
}

pub(crate) const FIELDS: [Field; 7] = [
    Field {
        label: "viewmodel.field.x",
        keys: ["main_x", "off_x"],
        min: -16.0,
        max: 16.0,
        step: 0.005,
        default: 0.0,
    },
    Field {
        label: "viewmodel.field.y",
        keys: ["main_y", "off_y"],
        min: -16.0,
        max: 16.0,
        step: 0.005,
        default: 0.0,
    },
    Field {
        label: "viewmodel.field.z",
        keys: ["main_z", "off_z"],
        min: -16.0,
        max: 16.0,
        step: 0.005,
        default: 0.0,
    },
    Field {
        label: "viewmodel.field.pitch",
        keys: ["main_pitch", "off_pitch"],
        min: -70.0,
        max: 70.0,
        step: 0.005,
        default: 0.0,
    },
    Field {
        label: "viewmodel.field.yaw",
        keys: ["main_yaw", "off_yaw"],
        min: -60.0,
        max: 60.0,
        step: 0.005,
        default: 0.0,
    },
    Field {
        label: "viewmodel.field.roll",
        keys: ["main_roll", "off_roll"],
        min: -60.0,
        max: 60.0,
        step: 0.005,
        default: 0.0,
    },
    Field {
        label: "viewmodel.field.scale",
        keys: ["main_scale", "off_scale"],
        min: 0.1,
        max: 5.0,
        step: 0.01,
        default: 1.0,
    },
];

pub(crate) fn register(registrar: &mut dyn Registrar) {
    let _ = registrar.register_setting(SettingSpec::choice(
        TARGET_KEY,
        "viewmodel.setting.target",
        TARGET_LABELS,
        0,
    ));
    let _ = registrar.register_setting(SettingSpec::toggle(
        HIDE_ARMS_KEY,
        "viewmodel.setting.hide_arms",
        false,
    ));
    for hand in [Hand::Main, Hand::Off] {
        for field in &FIELDS {
            let _ = registrar.register_setting(field.spec(hand));
        }
    }
}

fn hand_index(hand: Hand) -> usize {
    usize::from(hand == Hand::Off)
}

#[derive(Clone, Copy)]
pub(crate) struct Settings {
    values: [[f32; FIELDS.len()]; 2],
    pub target: Target,
    pub hide_arms: bool,
}

impl Settings {
    pub(crate) fn read(host: &dyn ClientHost, pending: &PendingSettings) -> Self {
        let values = [Hand::Main, Hand::Off].map(|hand| {
            std::array::from_fn(|i| {
                let field = &FIELDS[i];
                let key = field.key(hand);
                let value = pending
                    .get(key)
                    .or_else(|| host.setting_f64(key))
                    .unwrap_or(field.default);
                field.snap(value)
            })
        });
        Self {
            values,
            target: Target::from_index(
                pending
                    .get(TARGET_KEY)
                    .or_else(|| host.setting_choice(TARGET_KEY))
                    .unwrap_or_default(),
            ),
            hide_arms: pending
                .get(HIDE_ARMS_KEY)
                .or_else(|| host.setting_bool(HIDE_ARMS_KEY))
                .unwrap_or(false),
        }
    }

    pub(crate) fn pose(self, hand: Hand) -> Pose {
        let [x, y, z, pitch, yaw, roll, scale] = self.values[hand_index(hand)];
        Pose {
            translation: [x, y, z],
            rotation: [pitch, yaw, roll],
            scale,
        }
    }

    pub(crate) fn value(&self, hand: Hand, index: usize) -> f32 {
        self.values[hand_index(hand)][index]
    }

    pub(crate) fn set(&mut self, hand: Hand, index: usize, value: f64) -> bool {
        let previous = &mut self.values[hand_index(hand)][index];
        let new = FIELDS[index].snap(value);
        if *previous == new {
            return false;
        }
        *previous = new;
        true
    }

    pub(crate) fn queue(&self, pending: &mut PendingSettings, hand: Hand, index: usize) {
        pending.set(
            FIELDS[index].key(hand),
            format!("{:.3}", self.value(hand, index)),
        );
    }
}

#[derive(Default)]
pub(crate) struct PendingSettings {
    entries: VecDeque<SettingWrite>,
}

pub(crate) struct SettingWrite {
    pub key: &'static str,
    pub value: String,
    pub verify: bool,
}

impl PendingSettings {
    pub(crate) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn get<T: std::str::FromStr>(&self, key: &str) -> Option<T> {
        self.entries
            .iter()
            .find(|entry| entry.key == key)
            .and_then(|entry| entry.value.parse().ok())
    }

    pub(crate) fn set(&mut self, key: &'static str, value: String) {
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.key == key) {
            if entry.value != value {
                entry.value = value;
                entry.verify = false;
            }
        } else {
            self.entries.push_back(SettingWrite {
                key,
                value,
                verify: false,
            });
        }
    }

    pub(crate) fn forget(&mut self, key: &str) {
        self.entries.retain(|entry| entry.key != key);
    }

    pub(crate) fn take_next(&mut self) -> Option<SettingWrite> {
        self.entries.pop_front()
    }

    pub(crate) fn await_verification(&mut self, mut entry: SettingWrite) {
        entry.verify = true;
        self.entries.push_front(entry);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_settings_keep_only_the_latest_write_per_key() {
        let mut pending = PendingSettings::default();
        pending.set("main_x", "0.005".to_owned());
        pending.set("main_y", "0.010".to_owned());
        let first = pending.take_next().unwrap();
        pending.await_verification(first);
        pending.set("main_x", "0.015".to_owned());
        assert_eq!(pending.get::<f32>("main_x"), Some(0.015));
        let latest = pending.take_next().unwrap();
        assert_eq!(latest.key, "main_x");
        assert!(!latest.verify);
        pending.forget("main_y");
        assert!(pending.is_empty());
    }

    #[test]
    fn transform_specs_keep_identity_and_distinct_hand_keys() {
        let defaults = FIELDS.each_ref().map(|field| field.snap(field.default));
        let settings = Settings {
            values: [defaults; 2],
            target: Target::Both,
            hide_arms: false,
        };
        assert!(settings.pose(Hand::Main).is_identity());
        assert!(settings.pose(Hand::Off).is_identity());
        for field in &FIELDS {
            assert_ne!(field.key(Hand::Main), field.key(Hand::Off));
            assert!(field.spec(Hand::Main).check().is_ok());
            assert!(field.spec(Hand::Off).check().is_ok());
        }
    }

    #[test]
    fn fine_adjustment_is_signed_clamped_and_independent() {
        let defaults = FIELDS.each_ref().map(|field| field.default as f32);
        let mut settings = Settings {
            values: [defaults; 2],
            target: Target::Both,
            hide_arms: false,
        };
        assert!(settings.set(Hand::Off, 0, -0.006));
        assert_eq!(settings.pose(Hand::Off).translation[0], -0.005);
        assert!(settings.pose(Hand::Main).is_identity());
        assert!(!settings.set(Hand::Off, 0, -0.005));
        assert!(settings.set(Hand::Off, 0, -45.0));
        assert_eq!(settings.pose(Hand::Off).translation[0], -16.0);
        assert!(settings.set(Hand::Off, 0, f64::NAN));
        assert!(settings.pose(Hand::Off).is_identity());
    }
}
