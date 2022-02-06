use eyre::Context;
use inotify::{Inotify, WatchMask};
use log::{debug, info};
use serde::Deserialize;
use std::{path::Path, str::FromStr};

const DEV_INPUT: &'static str = "/dev/input";
const CONFIG_ENV_KEY: &'static str = "INPUTACTION_CONFIG";

#[derive(Debug, Deserialize, PartialEq, Eq, Copy, Clone)]
#[serde(try_from = "&str")]
struct KeyCode(evdev::Key);

impl<'a> TryFrom<&'a str> for KeyCode {
    type Error = eyre::Report;

    fn try_from(value: &'a str) -> Result<Self, Self::Error> {
        evdev::Key::from_str(value)
            .map_err(|_| eyre::eyre!("invalid key name '{}'", value))
            .map(KeyCode)
    }
}

#[derive(Debug, Deserialize, PartialEq, Eq, Copy, Clone)]
#[serde(rename_all = "lowercase")]
enum KeyState {
    Press,
    Release,
    Repeat,
}

impl TryFrom<i32> for KeyState {
    type Error = eyre::Report;

    fn try_from(v: i32) -> Result<Self, Self::Error> {
        match v {
            0 => Ok(KeyState::Release),
            1 => Ok(KeyState::Press),
            2 => Ok(KeyState::Repeat),
            v => Err(eyre::eyre!("unexpected value from key event: {}", v)),
        }
    }
}

impl Default for KeyState {
    fn default() -> Self {
        KeyState::Press
    }
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
struct ActionConfig {
    key: KeyCode,
    #[serde(default)]
    on: KeyState,
    action: String,
}

#[derive(Debug, Deserialize)]
struct Config {
    name: String,
    #[serde(default)]
    actions: Vec<ActionConfig>,
}

impl Config {
    fn action_for(&self, ev: &evdev::InputEvent) -> eyre::Result<Option<&ActionConfig>> {
        Ok(match ev.kind() {
            evdev::InputEventKind::Key(key) => {
                let state = KeyState::try_from(ev.value())?;
                self.actions
                    .iter()
                    .find(|&c| c.key.0 == key && c.on == state)
            }
            _ => None,
        })
    }
}

fn device_matches(config: &Config, device: &evdev::Device) -> bool {
    let name = device.name();
    match name {
        Some(name) if name == &config.name => {
            debug!("Device name '{}' == '{}'", name, config.name);
            true
        }
        Some(name) => {
            debug!("Device name '{}' != '{}'", name, config.name);
            false
        }
        None => {
            debug!("Device has no name");
            false
        }
    }
}

fn listen_device(config: &Config, device: &mut evdev::Device) -> eyre::Result<()> {
    info!("Listening for input events...");
    loop {
        for event in device.fetch_events()? {
            debug!("Received input event {:?}", event);
            if let Some(action) = config.action_for(&event)? {
                info!("Running command '{}'", action.action);
            }
        }
    }
}

fn main() -> eyre::Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .try_init()?;

    let config_string =
        std::env::var(CONFIG_ENV_KEY).context(format!("{} not set", CONFIG_ENV_KEY))?;
    let config: Config = toml::from_str(&config_string)?;

    info!("Enumerating initial devices...");
    for mut device in evdev::enumerate() {
        if device_matches(&config, &device) {
            listen_device(&config, &mut device)?;
            break;
        }
    }

    info!("Listening for device events...");
    let mut inotify = Inotify::init()?;
    inotify.add_watch(
        DEV_INPUT,
        WatchMask::ATTRIB | WatchMask::CREATE | WatchMask::MOVED_TO,
    )?;

    let mut buffer = [0; 1024];
    loop {
        let events = inotify.read_events_blocking(&mut buffer)?;
        for event in events {
            debug!("Received event {:?}", event);
            if let Some(name) = event.name {
                let path = Path::new(DEV_INPUT).join(name);
                debug!("{}: trying to open device", path.display());
                let device = evdev::Device::open(&path);
                match device {
                    Ok(mut device) if device_matches(&config, &device) => {
                        listen_device(&config, &mut device)?;
                    }
                    Err(error) => debug!("{}: failed to open device: {:?}", path.display(), error),
                    _ => (),
                }
            }
        }
    }
}