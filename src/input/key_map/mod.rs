use commands::{self, Command};
use errors::*;
use input::Key;
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::convert::Into;
use yaml::{Yaml, YamlLoader};

/// Nested HashMap newtype that provides a more ergonomic interface.
pub struct KeyMap(HashMap<String, HashMap<Key, Command>>);

impl KeyMap {
    /// Parses a Yaml tree of modes and their keybindings into a complete keymap.
    ///
    /// e.g.
    ///
    ///  normal:
    ///     ctrl-r: cursor::move_up
    ///
    /// becomes this HashMap entry:
    ///
    ///   "normal" => { Key::Ctrl('r') => commands::cursor::move_up }
    ///
    pub fn from(keymap_data: &Yaml) -> Result<KeyMap> {
        let modes = keymap_data.as_hash().ok_or(
            "Keymap config didn't return a hash of modes",
        )?;
        let mut keymap = HashMap::new();
        let commands = commands::hash_map();

        for (yaml_mode, yaml_key_bindings) in modes {
            let mode = yaml_mode.as_str().ok_or(format!(
                "A mode key couldn't be parsed as a string"
            ))?;
            let key_bindings = parse_mode_key_bindings(yaml_key_bindings, &commands).
                chain_err(|| "Failed to parse keymaps for \"{}\" mode")?;

            keymap.insert(mode.to_string(), key_bindings);
        }

        Ok(KeyMap(keymap))
    }

    /// Searches the keymap for the specified key.
    /// Character keys will fall back to wildcard character bindings
    /// if the specific character binding cannot be found.
    ///
    pub fn command_for(&self, mode: &str, key: &Key) -> Option<Command> {
        self.0.get(mode).and_then(|mode_keymap| {
            if let &Key::Char(_) = key {
                // Look for a command for this specific character, falling
                // back to another search for a wildcard character binding.
                mode_keymap.get(key).or_else(|| mode_keymap.get(&Key::AnyChar))
            } else {
                mode_keymap.get(key)
            }
        }).map(|command| *command)
    }

    pub fn default() -> Result<KeyMap> {
        let default_keymap_data = YamlLoader::load_from_str(include_str!("default.yml"))
            .chain_err(|| "Couldn't parse default keymap")?
            .into_iter()
            .nth(0)
            .ok_or("Couldn't locate a document in the default keymap")?;

        KeyMap::from(&default_keymap_data)
    }

    /// Merges each of the passed key map's modes, consuming them in the process.
    /// Note: the mode must exist to be merged; unmatched modes are discarded.
    ///
    /// e.g.
    ///
    /// normal:
    ///     ctrl-r: cursor::move_up
    ///
    /// merged with:
    ///
    /// normal:
    ///     ctrl-s: cursor::move_down
    /// unknown:
    ///     ctrl-l: cursor::move_right
    ///
    /// becomes this:
    ///
    ///   "normal" => {
    ///       Key::Ctrl('r') => commands::cursor::move_up
    ///       Key::Ctrl('s') => commands::cursor::move_down
    ///   }
    ///
    pub fn merge(&mut self, mut key_map: KeyMap) {
        // Step through the specified key map's modes.
        for (mode, other_key_bindings) in key_map.iter_mut() {
            // Fetch the current key bindings for the specified mode.
            if let Some(mut key_bindings) = self.get_mut(mode) {
                for (key, command) in other_key_bindings.drain() {
                    key_bindings.insert(key, command);
                }
            }
        }
    }
}

/// Parses the key bindings for a particular mode.
///
/// e.g.
///
///   ctrl-r: cursor::move_up
///
/// becomes this HashMap entry:
///
///   Key::Ctrl('r') => commands::cursor::move_up
///
fn parse_mode_key_bindings(mode: &Yaml, commands: &HashMap<&str, Command>) -> Result<HashMap<Key, Command>> {
    let mode_key_bindings = mode.as_hash().ok_or(
        "Keymap mode config didn't return a hash of key bindings",
    )?;

    let mut key_bindings = HashMap::new();
    for (yaml_key, yaml_command) in mode_key_bindings {
        // Parse modifier/character from key component.
        let key = parse_key(yaml_key.as_str().ok_or(format!(
            "A keymap key couldn't be parsed as a string"
        ))?)?;

        // Parse and find command reference from command component.
        let command_string = yaml_command.as_str().ok_or(format!(
            "A keymap command couldn't be parsed as a string"
        ))?;
        let command = commands.get(command_string).ok_or(format!(
            "Keymap command \"{}\" doesn't exist",
            command_string
        ))?;

        // Add a key/command entry to the mapping.
        key_bindings.insert(key, *command);
    }

    Ok(key_bindings)
}

/// Parses a str-based key into its Key equivalent.
///
/// e.g.
///
///   ctrl-r becomes Key::Ctrl('r')
///
fn parse_key(data: &str) -> Result<Key> {
    let mut key_components = data.split("-");
    let component = key_components.next().ok_or(
        "A keymap key is an empty string",
    )?;

    if let Some(key) = key_components.next() {
        // We have a modifier-qualified key; get the key.
        let key_char = key.chars().nth(0).ok_or(format!(
            "Keymap key \"{}\" is invalid",
            key
        ))?;

        // Find the variant for the specified modifier.
        match component {
            "ctrl" => Ok(Key::Ctrl(key_char)),
            _ => bail!(format!("Keymap modifier \"{}\" is invalid", component)),
        }
    } else {
        // No modifier; just get the key.
        Ok(match component {
            "space"     => Key::Char(' '),
            "backspace" => Key::Backspace,
            "left"      => Key::Left,
            "right"     => Key::Right,
            "up"        => Key::Up,
            "down"      => Key::Down,
            "home"      => Key::Home,
            "end"       => Key::End,
            "page_up"   => Key::PageUp,
            "page_down" => Key::PageDown,
            "delete"    => Key::Delete,
            "insert"    => Key::Insert,
            "escape"    => Key::Esc,
            "tab"       => Key::Tab,
            "enter"     => Key::Enter,
            "_"         => Key::AnyChar,
            _           => Key::Char(
                // It's not a keyword; take its first character, if available.
                component.chars().nth(0).ok_or(
                    format!("Keymap key \"{}\" is invalid", component)
                )?
            ),
        })
    }
}

impl Deref for KeyMap {
    type Target = HashMap<String, HashMap<Key, Command>>;

    fn deref(&self) -> &HashMap<String, HashMap<Key, Command>> {
        &self.0
    }
}

impl DerefMut for KeyMap {
    fn deref_mut(&mut self) -> &mut HashMap<String, HashMap<Key, Command>> {
        &mut self.0
    }
}

impl Into<HashMap<String, HashMap<Key, Command>>> for KeyMap {
    fn into(self) -> HashMap<String, HashMap<Key, Command>> {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use yaml::YamlLoader;
    use super::KeyMap;
    use commands;
    use input::Key;

    #[test]
    fn keymap_correctly_parses_yaml_character_keybindings() {
        // Build the keymap
        let yaml_data = "normal:\n  k: cursor::move_up";
        let yaml = YamlLoader::load_from_str(yaml_data).unwrap();
        let keymap = KeyMap::from(&yaml[0]).unwrap();

        let command = keymap.command_for("normal", &Key::Char('k')).expect(
            "Keymap doesn't contain command",
        );
        assert_eq!(
            (command as *const usize),
            (commands::cursor::move_up as *const usize)
        );
    }

    #[test]
    fn keymap_correctly_parses_yaml_wildcard_character_keybindings() {
        // Build the keymap
        let yaml_data = "normal:\n  _: cursor::move_up";
        let yaml = YamlLoader::load_from_str(yaml_data).unwrap();
        let keymap = KeyMap::from(&yaml[0]).unwrap();

        let characters = vec!['a', 'b', 'c'];
        for c in characters.into_iter() {
            let command = keymap.command_for("normal", &Key::Char(c)).expect(
                "Keymap doesn't contain command",
            );
            assert_eq!(
                (command as *const usize),
                (commands::cursor::move_up as *const usize)
            );
        }
    }

    #[test]
    fn keymap_correctly_prioritizes_character_over_wildcard_character_keybindings() {
        // Build the keymap
        let yaml_data = "normal:\n  j: cursor::move_down\n  _: cursor::move_up";
        let yaml = YamlLoader::load_from_str(yaml_data).unwrap();
        let keymap = KeyMap::from(&yaml[0]).unwrap();

        let char_command = keymap.command_for("normal", &Key::Char('j')).expect(
            "Keymap doesn't contain command",
        );
        assert_eq!(
            (char_command as *const usize),
            (commands::cursor::move_down as *const usize)
        );
        let wildcard_command = keymap.command_for("normal", &Key::Char('a')).expect(
            "Keymap doesn't contain command",
        );
        assert_eq!(
            (wildcard_command as *const usize),
            (commands::cursor::move_up as *const usize)
        );
    }

    #[test]
    fn keymap_correctly_parses_yaml_control_keybindings() {
        // Build the keymap
        let yaml_data = "normal:\n  ctrl-r: cursor::move_up";
        let yaml = YamlLoader::load_from_str(yaml_data).unwrap();
        let keymap = KeyMap::from(&yaml[0]).unwrap();

        let command = keymap.command_for("normal", &Key::Ctrl('r')).expect(
            "Keymap doesn't contain command",
        );
        assert_eq!(
            (command as *const usize),
            (commands::cursor::move_up as *const usize)
        );
    }

    #[test]
    fn keymap_correctly_parses_yaml_keyword_keybindings() {
        let mappings = vec![
            ("normal:\n  space: cursor::move_up",     Key::Char(' '), commands::cursor::move_up),
            ("normal:\n  backspace: cursor::move_up", Key::Backspace, commands::cursor::move_up),
            ("normal:\n  left: cursor::move_up",      Key::Left,      commands::cursor::move_up),
            ("normal:\n  right: cursor::move_up",     Key::Right,     commands::cursor::move_up),
            ("normal:\n  up: cursor::move_up",        Key::Up,        commands::cursor::move_up),
            ("normal:\n  down: cursor::move_up",      Key::Down,      commands::cursor::move_up),
            ("normal:\n  home: cursor::move_up",      Key::Home,      commands::cursor::move_up),
            ("normal:\n  end: cursor::move_up",       Key::End,       commands::cursor::move_up),
            ("normal:\n  page_up: cursor::move_up",   Key::PageUp,    commands::cursor::move_up),
            ("normal:\n  page_down: cursor::move_up", Key::PageDown,  commands::cursor::move_up),
            ("normal:\n  delete: cursor::move_up",    Key::Delete,    commands::cursor::move_up),
            ("normal:\n  insert: cursor::move_up",    Key::Insert,    commands::cursor::move_up),
            ("normal:\n  escape: cursor::move_up",    Key::Esc,       commands::cursor::move_up),
            ("normal:\n  tab: cursor::move_up",       Key::Tab,       commands::cursor::move_up),
            ("normal:\n  enter: cursor::move_up",     Key::Enter,     commands::cursor::move_up)
        ];

        for (binding, key, command) in mappings {
            // Build the keymap
            let yaml = YamlLoader::load_from_str(binding).unwrap();
            let keymap = KeyMap::from(&yaml[0]).unwrap();

            let parsed_command = keymap.command_for("normal", &key).expect("Keymap doesn't contain command");
            assert_eq!((parsed_command as *const usize), (command as *const usize));
        }
    }

    #[test]
    fn keymap_correctly_loads_default_keybindings() {
        // Build the keymap
        let keymap = KeyMap::default().unwrap();

        let command = keymap.command_for("normal", &Key::Char('k')).expect(
            "Keymap doesn't contain command",
        );
        assert_eq!(
            (command as *const usize),
            (commands::cursor::move_up as *const usize)
        );
    }

    #[test]
    fn keymap_correctly_merges_keybindings() {
        let yaml_data = "normal:\n  k: cursor::move_up\n  j: cursor::move_down";
        let yaml = YamlLoader::load_from_str(yaml_data).unwrap();
        let mut keymap = KeyMap::from(&yaml[0]).unwrap();

        let other_yaml_data = "normal:\n  k: cursor::move_left\n  l: cursor::move_right";
        let other_yaml = YamlLoader::load_from_str(other_yaml_data).unwrap();
        let other_keymap = KeyMap::from(&other_yaml[0]).unwrap();

        keymap.merge(other_keymap);

        let mut command = keymap.command_for("normal", &Key::Char('j')).expect(
            "Keymap doesn't contain original command",
        );
        assert_eq!(
            (command as *const usize),
            (commands::cursor::move_down as *const usize)
        );

        command = keymap.command_for("normal", &Key::Char('k')).expect(
            "Keymap doesn't contain overlapping command",
        );
        assert_eq!(
            (command as *const usize),
            (commands::cursor::move_left as *const usize)
        );

        command = keymap.command_for("normal", &Key::Char('l')).expect(
            "Keymap doesn't contain other command",
        );
        assert_eq!(
            (command as *const usize),
            (commands::cursor::move_right as *const usize)
        );
    }
}