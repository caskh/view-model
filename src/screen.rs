use mod_api::{ClientHost, DrawContext, Flow, Hand, ScreenRenderPostEvent, ScreenView};

use crate::ID;
use crate::settings::{FIELDS, HIDE_ARMS_KEY, Settings, TARGET_KEY, TARGET_LABELS, Target};

type Rect = [f32; 4];

pub(crate) struct Editor {
    hand: Hand,
    focus: usize,
    drag_start: Option<f32>,
}

impl Default for Editor {
    fn default() -> Self {
        Self {
            hand: Hand::Main,
            focus: 0,
            drag_start: None,
        }
    }
}

struct Layout {
    panel: Rect,
    main: Rect,
    off: Rect,
    target: Rect,
    hide: Rect,
    reset: Rect,
}

impl Layout {
    fn new([x, y, width, height]: Rect) -> Self {
        let panel_width = (width - 12.0).clamp(200.0, 286.0);
        let panel_height = 207.0;
        let left = x + ((width - panel_width) / 2.0).max(0.0);
        let top = y + (height - panel_height).clamp(0.0, 6.0);
        let half = (panel_width - 26.0) / 2.0;
        let first = left + 10.0;
        let second = first + half + 6.0;
        Self {
            panel: [left, top, panel_width, panel_height],
            main: [first, top + 22.0, half, 18.0],
            off: [second, top + 22.0, half, 18.0],
            target: [first, top + 46.0, half, 18.0],
            hide: [second, top + 46.0, half, 18.0],
            reset: [left + panel_width - 122.0, top + 174.0, 112.0, 17.0],
        }
    }

    fn row(&self, index: usize) -> Rect {
        let [x, y, width, _] = self.panel;
        [x + 10.0, y + 69.0 + index as f32 * 14.0, width - 20.0, 13.0]
    }

    fn track(&self, index: usize) -> Rect {
        let [x, y, width, _] = self.row(index);
        [x + 69.0, y + 5.0, width - 115.0, 3.0]
    }
}

fn contains([x, y, width, height]: Rect, [mx, my]: [f32; 2]) -> bool {
    (x..x + width).contains(&mx) && (y..y + height).contains(&my)
}

fn label(host: &dyn ClientHost, key: &str) -> String {
    host.translate(key).unwrap_or_else(|| key.to_owned())
}

fn button(draw: &mut dyn DrawContext, rect: Rect, mouse: [f32; 2], text: &str, selected: bool) {
    let color = match (selected, contains(rect, mouse)) {
        (true, true) => 0xff4a6676,
        (true, false) => 0xff344f61,
        (false, true) => 0xff383c42,
        (false, false) => 0xff292d32,
    };
    draw.fill(rect, color);
    let [x, y, width, height] = rect;
    draw.text(
        text,
        [
            x + (width - draw.text_width(text)) / 2.0,
            y + (height - 8.0) / 2.0,
        ],
        0xfff2f3f5,
        false,
    );
}

impl Editor {
    pub(crate) fn draw(
        &self,
        host: &dyn ClientHost,
        event: &mut ScreenRenderPostEvent<'_>,
        settings: &Settings,
    ) {
        let layout = Layout::new(event.screen.rect);
        let mouse = event.screen.mouse;
        let [x, y, _, _] = layout.panel;
        let draw = &mut *event.draw;
        draw.fill(layout.panel, 0xfa13161b);
        draw.text(
            &label(host, "viewmodel.title"),
            [x + 10.0, y + 8.0],
            0xffffffff,
            true,
        );
        for (rect, key, selected) in [
            (layout.main, "viewmodel.main", self.hand == Hand::Main),
            (layout.off, "viewmodel.off", self.hand == Hand::Off),
            (layout.target, TARGET_LABELS[settings.target.index()], false),
            (
                layout.hide,
                if settings.hide_arms {
                    "viewmodel.hidden"
                } else {
                    "viewmodel.shown"
                },
                settings.hide_arms,
            ),
            (layout.reset, "viewmodel.reset", false),
        ] {
            button(draw, rect, mouse, &label(host, key), selected);
        }

        for (index, field) in FIELDS.iter().enumerate() {
            let row = layout.row(index);
            let [x, y, width, _] = row;
            let [track_x, track_y, track_width, track_height] = layout.track(index);
            if index == self.focus {
                draw.fill(row, 0xff2a3c49);
            }
            draw.text(
                &label(host, field.label),
                [x + 3.0, y + 3.0],
                0xffe1e5eb,
                false,
            );
            let value = settings.value(self.hand, index);
            let fraction = ((f64::from(value) - field.min) / (field.max - field.min)) as f32;
            draw.fill([track_x, track_y, track_width, track_height], 0xff56606b);
            draw.fill(
                [
                    track_x + track_width * fraction - 2.0,
                    track_y - 2.0,
                    4.0,
                    track_height + 4.0,
                ],
                0xffa9d9ea,
            );
            let value = format!("{value:.3}");
            draw.text(
                &value,
                [x + width - draw.text_width(&value) - 3.0, y + 3.0],
                0xffe1e5eb,
                false,
            );
        }
        draw.text(
            &label(host, "viewmodel.help"),
            [x + 10.0, y + 195.0],
            0xffb8c1ca,
            false,
        );
    }

    pub(crate) fn press(
        &mut self,
        host: &dyn ClientHost,
        screen: &ScreenView<'_>,
        settings: &mut Settings,
    ) {
        self.finish_drag(host, settings);
        let layout = Layout::new(screen.rect);
        let mouse = screen.mouse;
        if contains(layout.main, mouse) {
            self.hand = Hand::Main;
        } else if contains(layout.off, mouse) {
            self.hand = Hand::Off;
        } else if contains(layout.target, mouse) {
            let next = (settings.target.index() + 1) % TARGET_LABELS.len();
            settings.target = Target::from_index(next as u32);
            host.set_setting(ID, TARGET_KEY, &next.to_string());
        } else if contains(layout.hide, mouse) {
            settings.hide_arms = !settings.hide_arms;
            host.set_setting(
                ID,
                HIDE_ARMS_KEY,
                if settings.hide_arms { "true" } else { "false" },
            );
        } else if contains(layout.reset, mouse) {
            for (index, field) in FIELDS.iter().enumerate() {
                self.edit(host, settings, index, field.default);
            }
        } else if let Some(index) = self.row_at(&layout, mouse) {
            self.focus = index;
            let [x, y, width, _] = layout.track(index);
            if contains([x - 3.0, y - 4.0, width + 6.0, 11.0], mouse) {
                self.drag_start = Some(settings.value(self.hand, index));
                self.drag(screen, settings);
            }
        }
    }

    pub(crate) fn drag(&self, screen: &ScreenView<'_>, settings: &mut Settings) {
        if self.drag_start.is_some() {
            let [x, _, width, _] = Layout::new(screen.rect).track(self.focus);
            let field = &FIELDS[self.focus];
            let fraction = f64::from((screen.mouse[0] - x) / width).clamp(0.0, 1.0);
            settings.set(
                self.hand,
                self.focus,
                field.min + fraction * (field.max - field.min),
            );
        }
    }

    pub(crate) fn finish_drag(&mut self, host: &dyn ClientHost, settings: &Settings) {
        if let Some(original) = self.drag_start.take()
            && original != settings.value(self.hand, self.focus)
        {
            settings.store(host, self.hand, self.focus);
        }
    }

    pub(crate) fn key(
        &mut self,
        host: &dyn ClientHost,
        key: &str,
        settings: &mut Settings,
    ) -> Flow {
        if self.drag_start.is_some() {
            return Flow::Continue;
        }
        match key {
            "key.keyboard.tab" | "key.keyboard.down" => {
                self.focus = (self.focus + 1) % FIELDS.len()
            }
            "key.keyboard.up" => self.focus = (self.focus + FIELDS.len() - 1) % FIELDS.len(),
            "key.keyboard.left" => self.adjust(host, settings, -1.0),
            "key.keyboard.right" => self.adjust(host, settings, 1.0),
            "key.keyboard.home" => {
                self.edit(host, settings, self.focus, FIELDS[self.focus].default)
            }
            _ => return Flow::Continue,
        }
        Flow::Cancel
    }

    pub(crate) fn scroll(
        &mut self,
        host: &dyn ClientHost,
        screen: &ScreenView<'_>,
        delta: f64,
        settings: &mut Settings,
    ) {
        if self.drag_start.is_none()
            && delta.is_finite()
            && delta != 0.0
            && let Some(index) = self.row_at(&Layout::new(screen.rect), screen.mouse)
        {
            self.focus = index;
            self.adjust(host, settings, delta.signum());
        }
    }

    fn row_at(&self, layout: &Layout, mouse: [f32; 2]) -> Option<usize> {
        (0..FIELDS.len()).find(|&index| contains(layout.row(index), mouse))
    }

    fn adjust(&self, host: &dyn ClientHost, settings: &mut Settings, direction: f64) {
        self.edit(
            host,
            settings,
            self.focus,
            f64::from(settings.value(self.hand, self.focus)) + direction * FIELDS[self.focus].step,
        );
    }

    fn edit(&self, host: &dyn ClientHost, settings: &mut Settings, index: usize, value: f64) {
        if settings.set(self.hand, index, value) {
            settings.store(host, self.hand, index);
        }
    }
}
