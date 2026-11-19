//! A dialog (prototype `.fcard`): a floating card over a dimmed app, with a
//! title, an optional line under it, and a close button.

use egui::{Align2, Area, Context, Id, Order, Rect, Sense, Shape, Ui, pos2, vec2};

use crate::anim;
use crate::emboss;
use crate::spring;
use crate::theme::{self, color, motion, radius, size, text};

pub struct SoftDialog<'a> {
    title: &'a str,
    subtitle: Option<String>,
    width: f32,
    dim: bool,
}

/// What happened to a dialog this frame.
pub struct DialogResponse<R> {
    pub inner: Option<R>,
    /// The close button or the dimmed area outside was clicked.
    pub close: bool,
}

impl<'a> SoftDialog<'a> {
    pub fn new(title: &'a str) -> Self {
        Self {
            title,
            subtitle: None,
            width: size::DIALOG_WIDTH,
            dim: true,
        }
    }

    pub fn subtitle(mut self, s: impl Into<String>) -> Self {
        self.subtitle = Some(s.into());
        self
    }

    pub fn width(mut self, w: f32) -> Self {
        self.width = w;
        self
    }

    /// Leave the app behind it as it is, and usable (it closes by its X).
    pub fn undimmed(mut self) -> Self {
        self.dim = false;
        self
    }

    /// Show it while `open`; it pops in and fades out.
    pub fn show<R>(
        self,
        ctx: &Context,
        open: bool,
        add: impl FnOnce(&mut Ui) -> R,
    ) -> DialogResponse<R> {
        let id = Id::unique(("soft-dialog", self.title));
        let t = anim::spring(
            ctx,
            id.with("shown"),
            if open { 1.0 } else { 0.0 },
            spring::SPRING_SNAPPY,
        );
        let mut close = false;
        if t < 0.01 && !open {
            return DialogResponse { inner: None, close };
        }
        let shown = t.clamp(0.0, 1.0);
        let screen = ctx.viewport_rect();

        // The dimmed app behind and the card are one layer: two layers would
        // keep their stacking between openings, and a backdrop that was once
        // clicked would then stay on top of the card.
        let [px, py] = size::DIALOG_PAD;
        let max_h = screen.height() * size::DIALOG_MAX_HEIGHT;
        let left = screen.center().x - self.width * 0.5;
        let top = screen.top() + (screen.height() - max_h) * 0.5;
        let rise = (1.0 - shown) * motion::POP_RISE;
        let card = Rect::from_min_size(pos2(left, top + rise), vec2(self.width, max_h));
        let area = Area::new(id)
            .order(Order::Foreground)
            .fixed_pos(if self.dim { screen.min } else { card.min })
            .interactable(open)
            .show(ctx, |ui| {
                // A click on the dimmed app (not on the card) closes the dialog.
                let backdrop = self.dim.then(|| {
                    ui.painter()
                        .rect_filled(screen, 0.0, theme::fade(color::SCRIM, shown));
                    ui.allocate_rect(screen, Sense::click())
                });
                let inner = ui.scope_builder(egui::UiBuilder::new().max_rect(card), |ui| {
                    ui.multiply_opacity(shown);
                    let bg = ui.painter().add(Shape::Noop);
                    let inner = egui::Frame::new()
                        .inner_margin(egui::Margin::symmetric(px as i8, py as i8))
                        .show(ui, |ui| {
                            ui.set_width(self.width - 2.0 * px);
                            ui.set_max_height(max_h - 2.0 * py);
                            let head = ui.horizontal(|ui| {
                                ui.vertical(|ui| {
                                    ui.label(
                                        egui::RichText::new(self.title)
                                            .font(egui::FontId::new(
                                                text::DIALOG_TITLE,
                                                theme::title().family,
                                            ))
                                            .color(color::INK),
                                    );
                                    if let Some(s) = &self.subtitle {
                                        ui.label(
                                            egui::RichText::new(s)
                                                .font(theme::mono())
                                                .color(color::MUTED),
                                        );
                                    }
                                });
                            });
                            let close_rect = Rect::from_min_size(
                                pos2(
                                    head.response.rect.left() + self.width
                                        - 2.0 * px
                                        - size::CLOSE_BUTTON,
                                    head.response.rect.top(),
                                ),
                                vec2(size::CLOSE_BUTTON, size::CLOSE_BUTTON),
                            );
                            let close_resp =
                                ui.interact(close_rect, id.with("close"), Sense::click());
                            let hot = close_resp.hovered();
                            if hot {
                                let mut v = Vec::new();
                                emboss::inset(
                                    &mut v,
                                    close_rect,
                                    radius::MENU_BUTTON,
                                    theme::depth::MENU_INSET,
                                );
                                ui.painter().extend(v);
                            }
                            ui.painter().text(
                                close_rect.center(),
                                Align2::CENTER_CENTER,
                                "×",
                                theme::title(),
                                color::INK,
                            );
                            if close_resp.clicked() {
                                close = true;
                            }
                            add(ui)
                        });
                    let mut shapes = Vec::new();
                    emboss::floating(
                        &mut shapes,
                        inner.response.rect,
                        radius::CARD,
                        color::SURFACE,
                        1.0,
                    );
                    ui.painter().set(bg, Shape::Vec(shapes));
                    (inner.inner, inner.response.rect)
                });
                let (value, card_rect) = inner.inner;
                if let Some(b) = backdrop
                    && b.clicked()
                    && b.interact_pointer_pos()
                        .is_none_or(|p| !card_rect.contains(p))
                {
                    close = true;
                }
                value
            });
        DialogResponse {
            inner: Some(area.inner),
            close,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use egui::{Event, PointerButton, RawInput};

    use super::*;
    use crate::widgets::SoftButton;

    struct Run {
        ctx: Context,
        time: f64,
        focused: bool,
        button: Cell<Rect>,
        clicked: Cell<bool>,
        closed: Cell<bool>,
    }

    impl Run {
        fn new() -> Self {
            Self {
                ctx: Context::default(),
                time: 0.0,
                focused: true,
                button: Cell::new(Rect::NOTHING),
                clicked: Cell::new(false),
                closed: Cell::new(false),
            }
        }

        fn frame(&mut self, open: bool, events: Vec<Event>) {
            self.time += 0.016;
            let input = RawInput {
                time: Some(self.time),
                screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1280.0, 800.0))),
                events,
                focused: self.focused,
                ..Default::default()
            };
            self.ctx
                .run_pass(input, |ctx| {
                    let r = SoftDialog::new("Import").show(ctx, open, |ui| {
                        let b = ui.add(SoftButton::new("Import"));
                        self.button.set(b.rect);
                        if b.clicked() {
                            self.clicked.set(true);
                        }
                    });
                    if r.close {
                        self.closed.set(true);
                    }
                })
                .textures_delta
                .clear();
        }

        fn click(&mut self, p: egui::Pos2) {
            let at = |pressed| Event::PointerButton {
                pos: p,
                button: PointerButton::Primary,
                pressed,
                modifiers: Default::default(),
            };
            self.frame(true, vec![Event::PointerMoved(p)]);
            self.frame(true, vec![at(true)]);
            self.frame(true, vec![at(false)]);
            self.frame(true, vec![]);
        }
    }

    /// A dialog that has just opened is on top of its dimmed backdrop: a
    /// click on its button is the button's, not the backdrop's.
    #[test]
    fn a_click_on_the_dialog_reaches_its_button() {
        let mut run = Run::new();
        run.frame(false, vec![]);
        for _ in 0..40 {
            run.frame(true, vec![]);
        }
        run.click(run.button.get().center());
        assert!(run.clicked.get(), "the button got the click");
        assert!(!run.closed.get(), "the backdrop did not");
    }

    /// The same, when the pointer was already over the window as the dialog
    /// opened (a file dropped on the window), and the click comes at once.
    #[test]
    fn a_click_straight_after_it_opens_reaches_its_button() {
        let mut run = Run::new();
        run.frame(false, vec![Event::PointerMoved(pos2(640.0, 300.0))]);
        run.frame(true, vec![]);
        run.frame(true, vec![]);
        run.click(run.button.get().center());
        assert!(run.clicked.get(), "the button got the click");
        assert!(!run.closed.get(), "the backdrop did not");
    }

    /// A click on the backdrop outside the card closes it.
    #[test]
    fn a_click_outside_closes() {
        let mut run = Run::new();
        run.frame(false, vec![]);
        for _ in 0..40 {
            run.frame(true, vec![]);
        }
        run.click(pos2(10.0, 10.0));
        assert!(run.closed.get());
        assert!(!run.clicked.get());
    }

    /// A file dropped on a window that is not in front: the window comes to
    /// the front with the user's first click, and the dialog still works.
    #[test]
    fn a_click_on_a_window_that_was_not_focused_reaches_the_button() {
        let mut run = Run::new();
        run.focused = false;
        run.frame(false, vec![Event::PointerMoved(pos2(640.0, 300.0))]);
        run.frame(true, vec![]);
        for _ in 0..40 {
            run.frame(true, vec![]);
        }
        run.focused = true;
        run.frame(true, vec![Event::WindowFocused(true)]);
        run.click(run.button.get().center());
        assert!(run.clicked.get(), "the button got the click");
        assert!(!run.closed.get(), "the backdrop did not");
    }

    /// Closed by a click on the backdrop, then opened again (the second file
    /// dropped on the window): the card is still on top of the backdrop.
    #[test]
    fn a_dialog_opened_again_after_a_backdrop_click_still_works() {
        let mut run = Run::new();
        run.frame(false, vec![]);
        for _ in 0..40 {
            run.frame(true, vec![]);
        }
        run.click(pos2(10.0, 10.0));
        assert!(run.closed.get());
        run.closed.set(false);
        for _ in 0..60 {
            run.frame(false, vec![]);
        }
        for _ in 0..40 {
            run.frame(true, vec![]);
        }
        run.click(run.button.get().center());
        assert!(run.clicked.get(), "the button got the click");
        assert!(!run.closed.get(), "the backdrop did not");
    }

    /// An undimmed dialog covers only its card: the app around it is usable.
    #[test]
    fn an_undimmed_dialog_leaves_the_app_around_it_alone() {
        let ctx = Context::default();
        let mut t = 0.0;
        let mut over = true;
        for k in 0..60 {
            t += 0.016;
            let input = RawInput {
                time: Some(t),
                screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1280.0, 800.0))),
                events: vec![Event::PointerMoved(pos2(10.0, 10.0))],
                ..Default::default()
            };
            ctx.run_pass(input, |ctx| {
                SoftDialog::new("Tool").undimmed().show(ctx, true, |ui| {
                    ui.label("hello");
                });
                if k == 59 {
                    over = ctx
                        .layer_id_at(pos2(10.0, 10.0))
                        .is_some_and(|l| l.order == Order::Foreground);
                }
            })
            .textures_delta
            .clear();
        }
        assert!(
            !over,
            "the pointer far from the card is not over the dialog"
        );
    }
}
