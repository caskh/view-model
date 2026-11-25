mod pose;
mod screen;
mod settings;

use std::sync::Mutex;

use mod_api::{
    ClientHost, ClientLevelUnloadEvent, ClientTickPostEvent, ClientTickPreEvent, Flow, Hand, Hooks,
    KeyConflictContext, KeyRegistration, Registrar, RegistrationContext, RenderArmEvent,
    RenderHandEvent, SettingChangedEvent, Text,
};
use settings::{PendingSettings, Settings, Target};

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
    pending: PendingSettings,
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
            let State {
                settings, pending, ..
            } = &mut *state;
            if settings.is_none() && host.local_player().is_some() {
                *settings = Some(Settings::read(host, pending));
            }
        }
        if host.consume_click(ID, OPEN_CONFIG) {
            if host.mod_screen_open(ID) {
                self.state.lock().unwrap().editor = screen::Editor::default();
            } else {
                let _ = host.chat_line(Text::translate("viewmodel.error.open", Vec::new()));
            }
        }
    }

    fn client_tick_pre(&self, host: &mut dyn ClientHost, _: &mut ClientTickPreEvent) {
        if self.state.lock().unwrap().pending.is_empty()
            || host.mod_screen() == Some(ID)
            || host.local_player().is_none()
        {
            return;
        }
        // Horizon's setting API writes synchronously. Give each I/O call its
        // own tick hook and leave disk activity out of the editor.
        let write = self.state.lock().unwrap().pending.take_next();
        if let Some(write) = write {
            if write.verify {
                if host.setting(ID, write.key).as_deref() != Some(write.value.as_str()) {
                    let _ = host.chat_line(Text::translate("viewmodel.error.save", Vec::new()));
                }
            } else {
                host.set_setting(ID, write.key, &write.value);
                self.state.lock().unwrap().pending.await_verification(write);
            }
        }
    }

    fn client_level_unload(&self, _: &mut dyn ClientHost, _: &mut ClientLevelUnloadEvent<'_>) {
        let mut state = self.state.lock().unwrap();
        state.settings = None;
        state.editor = screen::Editor::default();
    }

    fn setting_changed(&self, _: &mut dyn ClientHost, event: &mut SettingChangedEvent<'_>) {
        let mut state = self.state.lock().unwrap();
        state.pending.forget(event.key);
        state.settings = None;
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
        let State {
            settings,
            editor,
            pending,
        } = &mut *state;
        let settings = settings.get_or_insert_with(|| Settings::read(host, pending));
        editor.press(event.screen, settings, pending);
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
        let State {
            settings, editor, ..
        } = &mut *state;
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
        self.finish_drag();
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
        let State {
            settings,
            editor,
            pending,
        } = &mut *state;
        let Some(settings) = settings else {
            return Flow::Continue;
        };
        editor.key(event.key, settings, pending)
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
        let State {
            settings,
            editor,
            pending,
        } = &mut *state;
        if let Some(settings) = settings {
            editor.scroll(event.screen, event.scroll[1], settings, pending);
        }
        Flow::Cancel
    }

    fn screen_closing(&self, _: &dyn ClientHost, event: &mut mod_api::ScreenClosingEvent<'_>) {
        if event.screen.id == "mod" {
            self.finish_drag();
        }
    }
}

impl ViewModel {
    fn finish_drag(&self) {
        let mut state = self.state.lock().unwrap();
        let State {
            settings,
            editor,
            pending,
        } = &mut *state;
        if let Some(settings) = settings.as_ref() {
            editor.finish_drag(settings, pending);
        }
    }
}
