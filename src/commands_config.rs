use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandsFile {
    /// Wake word to trigger command mode
    #[serde(default)]
    pub wake_word: String,
    /// List of voice commands
    #[serde(default)]
    pub actions: Vec<VoiceCommand>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceCommand {
    /// Phrase(s) to match (case-insensitive)
    /// Can be a single string or array of strings
    #[serde(deserialize_with = "deserialize_string_or_vec")]
    pub phrase: Vec<String>,
    /// Action type: "keybind" or "shell"
    #[serde(rename = "type")]
    pub action_type: String,
    /// Value: key combination for keybind, or command for shell
    pub value: String,
}

/// Custom deserializer that accepts either a single string or an array of strings
fn deserialize_string_or_vec<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{self, Visitor};
    use std::fmt;

    struct StringOrVec;

    impl<'de> Visitor<'de> for StringOrVec {
        type Value = Vec<String>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("string or array of strings")
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(vec![value.to_string()])
        }

        fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: de::SeqAccess<'de>,
        {
            let mut vec = Vec::new();
            while let Some(value) = seq.next_element()? {
                vec.push(value);
            }
            Ok(vec)
        }
    }

    deserializer.deserialize_any(StringOrVec)
}

impl CommandsFile {
    /// Load commands from file
    pub fn load() -> Result<Self> {
        let commands_path = Self::commands_path()?;

        if !commands_path.exists() {
            // Create default commands file
            let default = Self::default();
            default.save()?;
            return Ok(default);
        }

        let content = fs::read_to_string(&commands_path)
            .context(format!("Failed to read commands file at {:?}", commands_path))?;

        let commands: CommandsFile = toml::from_str(&content)
            .context("Failed to parse commands file. Please check TOML syntax.")?;

        Ok(commands)
    }

    /// Save commands to file
    pub fn save(&self) -> Result<()> {
        let commands_path = Self::commands_path()?;

        // Create parent directory if it doesn't exist
        if let Some(parent) = commands_path.parent() {
            fs::create_dir_all(parent)
                .context(format!("Failed to create config directory at {:?}", parent))?;
        }

        let content = toml::to_string_pretty(self)
            .context("Failed to serialize commands to TOML")?;

        fs::write(&commands_path, content)
            .context(format!("Failed to write commands file to {:?}", commands_path))?;

        Ok(())
    }

    /// Get the commands file path
    pub fn commands_path() -> Result<PathBuf> {
        let home = std::env::var("HOME")
            .context("HOME environment variable not set")?;
        Ok(PathBuf::from(home).join(".config/thehand/commands.toml"))
    }
}

impl Default for CommandsFile {
    fn default() -> Self {
        Self {
            wake_word: String::new(),
            actions: Vec::new(),
        }
    }
}
