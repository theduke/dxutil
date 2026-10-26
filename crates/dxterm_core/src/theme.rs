use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Palette keys match xterm: `black` … `white`, `brightBlack` … `brightWhite`.
/// Values may be any CSS color, including CSS variables for DOM consumers.
#[derive(Clone, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalTheme {
    #[serde(flatten)]
    pub colors: BTreeMap<String, String>,
    pub background: Option<String>,
    pub foreground: Option<String>,
    pub cursor: Option<String>,
    pub selection_background: Option<String>,
}

impl TerminalTheme {
    pub fn background(&self) -> String {
        self.background
            .clone()
            .unwrap_or_else(|| "var(--terminal-background, #111820)".into())
    }
    pub fn foreground(&self) -> String {
        self.foreground
            .clone()
            .unwrap_or_else(|| "var(--terminal-foreground, #dce5ed)".into())
    }
    pub fn cursor(&self) -> String {
        self.cursor
            .clone()
            .unwrap_or_else(|| "var(--terminal-cursor, #dce5ed)".into())
    }
    pub fn selection_background(&self) -> String {
        self.selection_background
            .clone()
            .unwrap_or_else(|| "var(--terminal-selection, #45556b)".into())
    }
    pub fn palette(&self, index: u8) -> String {
        const KEYS: [&str; 16] = [
            "black",
            "red",
            "green",
            "yellow",
            "blue",
            "magenta",
            "cyan",
            "white",
            "brightBlack",
            "brightRed",
            "brightGreen",
            "brightYellow",
            "brightBlue",
            "brightMagenta",
            "brightCyan",
            "brightWhite",
        ];
        const COLORS: [&str; 16] = [
            "#000000", "#cd0000", "#00cd00", "#cdcd00", "#0000ee", "#cd00cd", "#00cdcd", "#e5e5e5",
            "#7f7f7f", "#ff0000", "#00ff00", "#ffff00", "#5c5cff", "#ff00ff", "#00ffff", "#ffffff",
        ];
        match index {
            0..=15 => self
                .colors
                .get(KEYS[index as usize])
                .cloned()
                .unwrap_or_else(|| COLORS[index as usize].into()),
            16..=231 => {
                let n = index - 16;
                let level = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
                rgb(level(n / 36), level(n / 6 % 6), level(n % 6))
            }
            _ => {
                let value = 8 + (index - 232) * 10;
                rgb(value, value, value)
            }
        }
    }
}
pub(crate) fn rgb(r: u8, g: u8, b: u8) -> String {
    format!("#{r:02x}{g:02x}{b:02x}")
}
