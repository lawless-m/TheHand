use anyhow::Result;
use enigo::{Enigo, Key, Keyboard, Settings};
use std::thread;
use std::time::Duration;

/// Type text into the focused window
pub fn type_text(text: &str, keystroke_delay_ms: u64) -> Result<()> {
    let mut enigo = Enigo::new(&Settings::default())?;
    let delay = Duration::from_millis(keystroke_delay_ms);

    for c in text.chars() {
        // Type the character
        if c == '\n' {
            enigo.key(Key::Return, enigo::Direction::Click)?;
        } else {
            enigo.text(&c.to_string())?;
        }

        // Small delay between keystrokes for reliability
        if keystroke_delay_ms > 0 {
            thread::sleep(delay);
        }
    }

    Ok(())
}
