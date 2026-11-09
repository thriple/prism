//! The MCP Server window (brief 21): the switch, the address and token,
//! the command that adds Prism to Claude Code, and the last few calls.

use egui::{Context, RichText};
use prism_ui::theme::{self, color, size};
use prism_ui::widgets::{SoftButton, SoftDialog, SoftToggle, field};

use super::{Chrome, storage};
use crate::mcp::{Listen, new_token};

/// Ports below this need admin rights on some systems.
const FIRST_PORT: u16 = 1024;
const PORT_FIELD_WIDTH: f32 = 90.0;

pub(super) fn show(chrome: &mut Chrome, ctx: &Context) {
    let mut settings = chrome.mcp.settings.clone();
    let listen = chrome.mcp.listen.clone();
    let log: Vec<String> = chrome.mcp.log.iter().cloned().collect();
    let id = egui::Id::unique("mcp-port-text");
    let mut port_text: String = ctx
        .data(|d| d.get_temp(id))
        .unwrap_or_else(|| settings.port.to_string());
    let mut copied = false;
    let r = SoftDialog::new("MCP Server")
        .subtitle("Let an AI assistant on this computer use Prism")
        .show(ctx, chrome.mcp_open, |ui| {
            ui.add_space(size::MENU_PAD);
            let text = |ui: &mut egui::Ui, t: &str, c| {
                ui.add(egui::Label::new(RichText::new(t).color(c)).wrap());
            };
            if listen == Listen::Unavailable {
                text(
                    ui,
                    "Not available in the web version: a browser page cannot accept connections. \
                     Use the Windows or macOS build.",
                    color::INK,
                );
                return;
            }
            ui.add(SoftToggle::new(
                &mut settings.enabled,
                "Let AI assistants connect",
            ));
            ui.add_space(size::MENU_PAD * 0.5);
            match &listen {
                Listen::On(port) => text(
                    ui,
                    &format!("Listening on http://127.0.0.1:{port}/mcp (this computer only)."),
                    color::INK,
                ),
                Listen::Failed(e) => text(ui, e, color::INK),
                Listen::Off | Listen::Unavailable => text(ui, "Off.", color::MUTED),
            }
            ui.add_space(size::MENU_PAD);
            ui.horizontal(|ui| {
                ui.label(RichText::new("Port").color(color::INK));
                let r = field(ui, &mut port_text, "47615", PORT_FIELD_WIDTH);
                if r.lost_focus()
                    && let Ok(p) = port_text.trim().parse::<u16>()
                    && p >= FIRST_PORT
                {
                    settings.port = p;
                }
            });
            ui.add_space(size::MENU_PAD * 0.5);
            ui.horizontal(|ui| {
                ui.label(RichText::new("Token").color(color::INK));
                ui.label(
                    RichText::new(&settings.token)
                        .font(theme::mono())
                        .color(color::MUTED),
                );
                if ui.add(SoftButton::new("New Token").small()).clicked() {
                    settings.token = new_token();
                }
            });
            ui.add_space(size::MENU_PAD);
            text(ui, "To add Prism to Claude Code, run:", color::INK);
            ui.add(
                egui::Label::new(
                    RichText::new(settings.claude_command())
                        .font(theme::mono())
                        .color(color::INK),
                )
                .wrap(),
            );
            if ui.add(SoftButton::new("Copy Command").small()).clicked() {
                copied = true;
            }
            if !log.is_empty() {
                ui.add_space(size::MENU_PAD);
                text(ui, "Last calls:", color::MUTED);
                for l in &log {
                    ui.label(RichText::new(l).font(theme::mono()).color(color::MUTED));
                }
            }
            ui.add_space(size::MENU_PAD);
            ui.label(
                RichText::new(
                    "Only programs on this computer that have the token can connect. \
                     What they change can be undone like any edit.",
                )
                .font(theme::small())
                .color(color::MUTED),
            );
        });
    ctx.data_mut(|d| d.insert_temp(id, port_text));
    if copied {
        ctx.copy_text(settings.claude_command());
        chrome.status = "Copied the command for Claude Code".to_owned();
    }
    if r.close {
        chrome.mcp_open = false;
        ctx.data_mut(|d| d.remove::<String>(id));
    }
    if settings != chrome.mcp.settings {
        chrome.mcp.settings = settings;
        chrome.mcp.apply();
        if chrome.persist {
            storage::save_mcp(
                &serde_json::to_string_pretty(&chrome.mcp.settings).unwrap_or_default(),
            );
        }
    }
}
