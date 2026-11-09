// Copyright 2026 Mark Alan Boykin
// SPDX-License-Identifier: MPL-2.0

use std::collections::HashMap;

use servo::{Key, NamedKey};

use super::types::ServoKeyEvent;

/// Project a host key event onto Servo's logical key. A release reuses the
/// logical key recorded at press, so text composed on press survives.
pub(super) fn logical_key(event: &ServoKeyEvent, pressed_keys: &mut HashMap<u32, Key>) -> Key {
    if !event.pressed {
        if let Some(key) = pressed_keys.remove(&event.key_code) {
            return key;
        }
    }
    // Winit can supply an empty string for a non-printable key, or control
    // text such as carriage return for Enter. Neither is a character key.
    let key = if let Some(text) = event
        .text
        .as_ref()
        .filter(|text| !text.is_empty() && !text.chars().any(char::is_control))
    {
        Key::Character(text.clone())
    } else if event.key_code == 32
        || (48..=57).contains(&event.key_code)
        || (65..=90).contains(&event.key_code)
    {
        Key::Character(
            char::from_u32(event.key_code)
                .unwrap()
                .to_ascii_lowercase()
                .to_string(),
        )
    } else {
        Key::Named(match event.key_code {
            8 => NamedKey::Backspace,
            9 => NamedKey::Tab,
            13 => NamedKey::Enter,
            27 => NamedKey::Escape,
            33 => NamedKey::PageUp,
            34 => NamedKey::PageDown,
            35 => NamedKey::End,
            36 => NamedKey::Home,
            37 => NamedKey::ArrowLeft,
            38 => NamedKey::ArrowUp,
            39 => NamedKey::ArrowRight,
            40 => NamedKey::ArrowDown,
            45 => NamedKey::Insert,
            46 => NamedKey::Delete,
            _ => NamedKey::Unidentified,
        })
    };
    if event.pressed {
        pressed_keys.insert(event.key_code, key.clone());
    }
    key
}
