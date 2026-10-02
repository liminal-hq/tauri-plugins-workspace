// Parses Tauri-style accelerators into Windows hotkey modifiers and virtual-key codes
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::error::ServiceError;

/// The event emitted when a registered Windows global shortcut is pressed.
pub const PRESSED_EVENT: &str = "desktop-integration://shortcut-pressed";

pub const MOD_ALT: u32 = 0x0001;
pub const MOD_CONTROL: u32 = 0x0002;
pub const MOD_SHIFT: u32 = 0x0004;
pub const MOD_WIN: u32 = 0x0008;

/// A parsed accelerator: `RegisterHotKey`'s modifiers and virtual-key code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Accelerator {
    pub modifiers: u32,
    pub key: u32,
}

/// Parses an accelerator such as `Ctrl+Alt+K`, `Super+Space` or `CmdOrCtrl+Shift+F5`.
///
/// At least one modifier is required: a bare key would swallow that key system-wide.
pub fn parse_accelerator(accelerator: &str) -> Result<Accelerator, ServiceError> {
    let mut modifiers = 0;
    let mut key = None;
    // A trailing `+` is the plus key itself, not a separator.
    let (head, plus_key) = match accelerator.strip_suffix("++") {
        Some(head) => (head, true),
        None => (accelerator, false),
    };
    let mut tokens: Vec<&str> = head.split('+').filter(|t| !t.is_empty()).collect();
    if plus_key {
        tokens.push("Plus");
    }
    for token in tokens {
        let token = token.trim();
        match token.to_ascii_lowercase().as_str() {
            "alt" | "option" => modifiers |= MOD_ALT,
            "ctrl" | "control" | "cmdorctrl" | "commandorcontrol" => modifiers |= MOD_CONTROL,
            "shift" => modifiers |= MOD_SHIFT,
            "super" | "meta" | "win" | "windows" | "cmd" | "command" => modifiers |= MOD_WIN,
            _ => {
                let code = key_code(token).ok_or_else(|| {
                    ServiceError::invalid(format!("unknown key {token:?} in {accelerator:?}"))
                })?;
                if key.replace(code).is_some() {
                    return Err(ServiceError::invalid(format!(
                        "{accelerator:?} has more than one key"
                    )));
                }
            }
        }
    }
    let key = key.ok_or_else(|| ServiceError::invalid(format!("{accelerator:?} has no key")))?;
    if modifiers == 0 {
        return Err(ServiceError::invalid(format!(
            "{accelerator:?} needs at least one modifier"
        )));
    }
    Ok(Accelerator { modifiers, key })
}

/// The virtual-key code of a key name.
fn key_code(name: &str) -> Option<u32> {
    let lower = name.to_ascii_lowercase();
    let mut chars = lower.chars();
    if let (Some(c), None) = (chars.next(), chars.next()) {
        match c {
            'a'..='z' => return Some(0x41 + (c as u32 - 'a' as u32)),
            '0'..='9' => return Some(0x30 + (c as u32 - '0' as u32)),
            _ => {}
        }
    }
    if let Some(number) = lower.strip_prefix('f').and_then(|n| n.parse::<u32>().ok()) {
        if (1..=24).contains(&number) {
            return Some(0x70 + number - 1);
        }
    }
    Some(match lower.as_str() {
        "space" => 0x20,
        "enter" | "return" => 0x0d,
        "tab" => 0x09,
        "escape" | "esc" => 0x1b,
        "backspace" => 0x08,
        "delete" | "del" => 0x2e,
        "insert" => 0x2d,
        "home" => 0x24,
        "end" => 0x23,
        "pageup" => 0x21,
        "pagedown" => 0x22,
        "left" | "arrowleft" => 0x25,
        "up" | "arrowup" => 0x26,
        "right" | "arrowright" => 0x27,
        "down" | "arrowdown" => 0x28,
        "plus" | "=" | "equal" => 0xbb,
        "minus" | "-" => 0xbd,
        "comma" | "," => 0xbc,
        "period" | "." => 0xbe,
        "slash" | "/" => 0xbf,
        "semicolon" | ";" => 0xba,
        "quote" | "'" => 0xde,
        "backslash" | "\\" => 0xdc,
        "bracketleft" | "[" => 0xdb,
        "bracketright" | "]" => 0xdd,
        "backquote" | "`" => 0xc0,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_letters_digits_and_function_keys() {
        assert_eq!(
            parse_accelerator("Ctrl+Alt+K").unwrap(),
            Accelerator {
                modifiers: MOD_CONTROL | MOD_ALT,
                key: 0x4b
            }
        );
        assert_eq!(parse_accelerator("Shift+7").unwrap().key, 0x37);
        assert_eq!(parse_accelerator("CmdOrCtrl+F5").unwrap().key, 0x74);
        assert_eq!(parse_accelerator("Ctrl+F24").unwrap().key, 0x87);
    }

    #[test]
    fn parses_named_keys_and_super() {
        assert_eq!(
            parse_accelerator("Super+Space").unwrap(),
            Accelerator {
                modifiers: MOD_WIN,
                key: 0x20
            }
        );
        assert_eq!(parse_accelerator("Alt+PageDown").unwrap().key, 0x22);
        assert_eq!(
            parse_accelerator("Ctrl+Shift+Left").unwrap().modifiers,
            MOD_CONTROL | MOD_SHIFT
        );
    }

    #[test]
    fn a_trailing_plus_is_the_plus_key() {
        assert_eq!(
            parse_accelerator("Ctrl++").unwrap(),
            Accelerator {
                modifiers: MOD_CONTROL,
                key: 0xbb
            }
        );
    }

    #[test]
    fn rejects_malformed_accelerators() {
        for bad in [
            "",
            "K",
            "F5",
            "Ctrl",
            "Ctrl+",
            "Ctrl+Alt+Nope",
            "Ctrl+A+B",
            "Ctrl+F25",
        ] {
            assert!(parse_accelerator(bad).is_err(), "{bad:?}");
        }
    }
}
