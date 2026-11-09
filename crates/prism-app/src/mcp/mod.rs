//! The local MCP server (brief 21): off by default; when on, an AI
//! assistant on this machine can read the model and run actions. The
//! protocol is in [`protocol`], the socket in `server` (native only), and
//! the tools run on the main thread through [`host`].

pub mod host;
pub mod protocol;
#[cfg(not(target_arch = "wasm32"))]
mod server;

use std::collections::VecDeque;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The default port: unassigned, and unlikely to be in use.
pub const DEFAULT_PORT: u16 = 47615;
/// How many recent calls the window lists.
const LOG_LINES: usize = 6;

/// What the user chose. Kept in the user's settings (`mcp.json`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub enabled: bool,
    pub port: u16,
    pub token: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: false,
            port: DEFAULT_PORT,
            token: new_token(),
        }
    }
}

impl Settings {
    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}/mcp", self.port)
    }

    /// The command that adds Prism to Claude Code.
    pub fn claude_command(&self) -> String {
        format!(
            "claude mcp add --transport http prism {} --header \"Authorization: Bearer {}\"",
            self.url(),
            self.token
        )
    }
}

/// A fresh token: 32 hex characters from the system's random seed (each
/// `RandomState` is keyed from it).
pub fn new_token() -> String {
    use std::hash::{BuildHasher, Hasher};
    let nanos = web_time::SystemTime::now()
        .duration_since(web_time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    (0..2u64)
        .map(|i| {
            let mut h = std::collections::hash_map::RandomState::new().build_hasher();
            h.write_u64(i);
            h.write_u128(nanos);
            format!("{:016x}", h.finish())
        })
        .collect()
}

/// Whether the server is listening.
#[derive(Clone, Debug, PartialEq)]
pub enum Listen {
    Off,
    On(u16),
    Failed(String),
    /// The web build: browsers cannot listen.
    Unavailable,
}

/// A tool call waiting for the main thread.
pub struct Call {
    pub name: String,
    pub args: Value,
    pub reply: std::sync::mpsc::Sender<Result<String, String>>,
}

type Wake = Arc<dyn Fn() + Send + Sync>;

/// The server's state, owned by the chrome.
pub struct Mcp {
    pub settings: Settings,
    pub listen: Listen,
    /// The last few calls, newest last.
    pub log: VecDeque<String>,
    wake: Option<Wake>,
    #[cfg(not(target_arch = "wasm32"))]
    server: Option<server::Server>,
}

impl Default for Mcp {
    fn default() -> Self {
        Self::new(Settings::default())
    }
}

impl Mcp {
    pub fn new(settings: Settings) -> Self {
        Self {
            settings,
            listen: if cfg!(target_arch = "wasm32") {
                Listen::Unavailable
            } else {
                Listen::Off
            },
            log: VecDeque::new(),
            wake: None,
            #[cfg(not(target_arch = "wasm32"))]
            server: None,
        }
    }

    /// How the server wakes the app when a call arrives.
    pub fn set_wake(&mut self, wake: impl Fn() + Send + Sync + 'static) {
        self.wake = Some(Arc::new(wake));
    }

    /// Start or stop the server to match the settings (and restart it on
    /// a new port or token).
    pub fn apply(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            if let Some(s) = self.server.take() {
                s.stop();
            }
            self.listen = Listen::Off;
            if !self.settings.enabled {
                return;
            }
            let wake = self.wake.clone().unwrap_or_else(|| Arc::new(|| {}));
            match server::Server::start(self.settings.port, self.settings.token.clone(), wake) {
                Ok(s) => {
                    self.listen = Listen::On(s.port());
                    self.server = Some(s);
                }
                Err(e) => self.listen = Listen::Failed(e),
            }
        }
    }

    /// Calls waiting for the main thread.
    pub fn take_calls(&mut self) -> Vec<Call> {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(s) = &self.server {
            return s.calls.try_iter().collect();
        }
        Vec::new()
    }

    /// Note a call for the window.
    pub fn note(&mut self, line: String) {
        self.log.push_back(line);
        while self.log.len() > LOG_LINES {
            self.log.pop_front();
        }
    }
}

impl Drop for Mcp {
    fn drop(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(s) = self.server.take() {
            s.stop();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_32_hex_characters_and_differ() {
        let (a, b) = (new_token(), new_token());
        assert_eq!(a.len(), 32);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b);
    }

    #[test]
    fn settings_read_back_and_default_off() {
        let s = Settings::default();
        assert!(!s.enabled);
        let text = serde_json::to_string(&s).unwrap();
        assert_eq!(serde_json::from_str::<Settings>(&text).unwrap(), s);
        let partial: Settings = serde_json::from_str(r#"{"enabled": true}"#).unwrap();
        assert!(partial.enabled && partial.port == DEFAULT_PORT && partial.token.len() == 32);
        assert!(s.claude_command().contains(&format!("Bearer {}", s.token)));
        assert!(s.claude_command().contains("http://127.0.0.1:47615/mcp"));
    }
}
