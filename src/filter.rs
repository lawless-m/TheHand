use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

/// Load filtered phrases from a plain text file
/// One phrase per line, no quotes or special formatting
pub fn load_filtered_phrases() -> Result<Vec<String>> {
    let filter_path = filter_path()?;

    if !filter_path.exists() {
        // Create default filter file
        save_filtered_phrases(&default_filtered_phrases())?;
        return Ok(default_filtered_phrases());
    }

    let content = fs::read_to_string(&filter_path)
        .context(format!("Failed to read filter file at {:?}", filter_path))?;

    // Parse lines, trim whitespace, skip empty lines
    let phrases: Vec<String> = content
        .lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| line.to_string())
        .collect();

    Ok(phrases)
}

/// Save filtered phrases to file
pub fn save_filtered_phrases(phrases: &[String]) -> Result<()> {
    let filter_path = filter_path()?;

    // Create parent directory if it doesn't exist
    if let Some(parent) = filter_path.parent() {
        fs::create_dir_all(parent)
            .context(format!("Failed to create config directory at {:?}", parent))?;
    }

    let content = phrases.join("\n");
    fs::write(&filter_path, content)
        .context(format!("Failed to write filter file to {:?}", filter_path))?;

    Ok(())
}

/// Get the filter file path
pub fn filter_path() -> Result<PathBuf> {
    let home = std::env::var("HOME")
        .context("HOME environment variable not set")?;
    Ok(PathBuf::from(home).join(".config/thehand/filtered.txt"))
}

/// Add a single phrase to the filter file
pub fn add_filtered_phrase(phrase: &str) -> Result<()> {
    let filter_path = filter_path()?;

    // Create parent directory if it doesn't exist
    if let Some(parent) = filter_path.parent() {
        fs::create_dir_all(parent)
            .context(format!("Failed to create config directory at {:?}", parent))?;
    }

    // Append the phrase to the file
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&filter_path)
        .context(format!("Failed to open filter file at {:?}", filter_path))?;

    writeln!(file, "{}", phrase)
        .context("Failed to write to filter file")?;

    Ok(())
}

/// Default filtered phrases (common Whisper hallucinations)
fn default_filtered_phrases() -> Vec<String> {
    vec![
        "Thank you.".to_string(),
        "Thank you".to_string(),
        "you".to_string(),
        "You".to_string(),
        "Okay.".to_string(),
        "Okay".to_string(),
        "Bye.".to_string(),
        "Woof.".to_string(),
        "Woof!".to_string(),
        "Peace out.".to_string(),
        "Yeah.".to_string(),
    ]
}
