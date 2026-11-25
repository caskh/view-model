mod pose;
mod screen;
mod settings;

use std::sync::Mutex;

use mod_api::{
    ClientHost, ClientLevelUnloadEvent, ClientTickPostEvent, Flow, Hand, Hooks, KeyConflictContext,
    KeyRegistration, Registrar, RegistrationContext, RenderArmEvent, RenderHandEvent,
    SettingChangedEvent,
};
use settings::{Settings, Target};

const ID: &str = "viewmodel";
const OPEN_CONFIG: &str = "open_config";

#[derive(Default)]
struct ViewModel {
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    settings: Option<Settings>,
    editor: screen::Editor,
}

#[mod_api::horizon_mod]
impl Hooks for ViewModel {
    fn register(&self, registrar: &mut dyn Registrar, _: &RegistrationContext) {
        settings::register(registrar);
        registrar.register_key_mapping(
            KeyRegistration::new(OPEN_CONFIG, "key.keyboard.v")
                .label("key.viewmodel.open_config")
                .context(KeyConflictContext::InGame),
        );
    }

    fn client_tick_post(&self, host: &mut dyn ClientHost, _: &mut ClientTickPostEvent) {
        {
            let mut state = self.state.lock().unwrap();
            state.settings.get_or_insert_with(|| Settings::read(host));
        }
        if host.consume_click(ID, OPEN_CONFIG) && host.mod_screen_open(ID) {
            self.state.lock().unwrap().editor = screen::Editor::default();
        }
    }

    fn client_level_unload(&self, _: &mut dyn ClientHost, _: &mut ClientLevelUnloadEvent<'_>) {
        *self.state.lock().unwrap() = State::default();
    }

    fn setting_changed(&self, _: &mut dyn ClientHost, _: &mut SettingChangedEvent<'_>) {
        self.state.lock().unwrap().settings = None;
    }

    fn render_hand(&self, host: &dyn ClientHost, event: &mut RenderHandEvent<'_, '_>) -> Flow {
        let Some(settings) = self.state.lock().unwrap().settings else {
            return Flow::Continue;
        };
        let hand = event.render.hand;
        let pose = settings.pose(hand);
        if pose.is_identity() || (hand == Hand::Off && event.render.stack.is_none()) {
            return Flow::Continue;
        }
        let needs_pivot = pose.needs_pivot();
        let is_map = (needs_pivot || matches!(settings.target, Target::Items)) && {
            event.render.stack.is_some_and(|stack| {
                stack.components().is_some_and(|components| {
                    components
                        .as_nbt()
                        .keys()
                        .any(|key| key.0.iter().copied().eq("minecraft:map_id".encode_utf16()))
                })
            })
        };
        let apply = match settings.target {
            Target::Both => true,
            Target::Items => event.render.stack.is_some() && !is_map,
            Target::Arms => event.render.stack.is_none() && !settings.hide_arms,
        };
        if apply {
            let mut translation = pose.translation;
            if needs_pivot {
                translation = pose.with_pivot(pose::hand_pivot(host, event.render, is_map));
            }
            for axis in 0..3 {
                event.render.translation[axis] += translation[axis];
                event.render.rotation[axis] += pose.rotation[axis];
            }
            event.render.scale *= pose.scale;
        }
        Flow::Continue
    }

    fn render_arm(&self, _: &dyn ClientHost, _: &mut RenderArmEvent<'_>) -> Flow {
        if self
            .state
            .lock()
            .unwrap()
            .settings
            .is_some_and(|settings| settings.hide_arms)
        {
            Flow::Cancel
        } else {
            Flow::Continue
        }
    }

    fn screen_render_post(
        &self,
        host: &dyn ClientHost,
        event: &mut mod_api::ScreenRenderPostEvent<'_>,
    ) {
        if host.mod_screen() == Some(ID) {
            let state = self.state.lock().unwrap();
            if let Some(settings) = &state.settings {
                state.editor.draw(host, event, settings);
            }
        }
    }

    fn screen_mouse_button_pressed_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut mod_api::ScreenMouseButtonPressedPreEvent<'_>,
    ) -> Flow {
        if host.mod_screen() != Some(ID) || event.button != 0 {
            return Flow::Continue;
        }
        let mut state = self.state.lock().unwrap();
        let State { settings, editor } = &mut *state;
        let settings = settings.get_or_insert_with(|| Settings::read(host));
        editor.press(host, event.screen, settings);
        Flow::Cancel
    }

    fn screen_mouse_dragged_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut mod_api::ScreenMouseDraggedPreEvent<'_>,
    ) -> Flow {
        if host.mod_screen() != Some(ID) || event.button != 0 {
            return Flow::Continue;
        }
        let mut state = self.state.lock().unwrap();
        let State { settings, editor } = &mut *state;
        if let Some(settings) = settings {
            editor.drag(event.screen, settings);
        }
        Flow::Cancel
    }

    fn screen_mouse_button_released_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut mod_api::ScreenMouseButtonReleasedPreEvent<'_>,
    ) -> Flow {
        if host.mod_screen() != Some(ID) || event.button != 0 {
            return Flow::Continue;
        }
        self.finish_drag(host);
        Flow::Cancel
    }

    fn screen_key_pressed_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut mod_api::ScreenKeyPressedPreEvent<'_>,
    ) -> Flow {
        if host.mod_screen() != Some(ID) {
            return Flow::Continue;
        }
        let mut state = self.state.lock().unwrap();
        let State { settings, editor } = &mut *state;
        let Some(settings) = settings else {
            return Flow::Continue;
        };
        editor.key(host, event.key, settings)
    }

    fn screen_mouse_scrolled_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut mod_api::ScreenMouseScrolledPreEvent<'_>,
    ) -> Flow {
        if host.mod_screen() != Some(ID) {
            return Flow::Continue;
        }
        let mut state = self.state.lock().unwrap();
        let State { settings, editor } = &mut *state;
        if let Some(settings) = settings {
            editor.scroll(host, event.screen, event.scroll[1], settings);
        }
        Flow::Cancel
    }

    fn screen_closing(&self, host: &dyn ClientHost, event: &mut mod_api::ScreenClosingEvent<'_>) {
        if event.screen.id == "mod" {
            self.finish_drag(host);
        }
    }
}

impl ViewModel {
    fn finish_drag(&self, host: &dyn ClientHost) {
        let mut state = self.state.lock().unwrap();
        let State { settings, editor } = &mut *state;
        if let Some(settings) = settings.as_ref() {
            editor.finish_drag(host, settings);
        }
    }
}
