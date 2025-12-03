use anyhow::Result;
use enigo::{Enigo, Key, Keyboard, Settings};
use std::thread;
use std::time::Duration;

/// Type text into the focused window
pub fn type_text(text: &str, keystroke_delay_ms: u64) -> Result<()> {
    let mut enigo = Enigo::new(&Settings::default())?;
    let delay = Duration::from_millis(keystroke_delay_ms);

    // Strip newlines - we never want Enter pressed for normal transcriptions
    // Only the "text" command type should press Enter
    let text_cleaned = text.replace('\n', " ");

    for c in text_cleaned.chars() {
        enigo.text(&c.to_string())?;

        // Small delay between keystrokes for reliability
        if keystroke_delay_ms > 0 {
            thread::sleep(delay);
        }
    }

    // Append a trailing space for convenience
    enigo.text(" ")?;

    Ok(())
}
