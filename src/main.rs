use inotify::{Inotify, WatchMask};
use log::{debug, error, info};
use serde::Deserialize;
use std::{
    path::Path,
    process::{Child, Command},
    str::FromStr,
    sync::mpsc::{Receiver, SyncSender},
};

const DEV_INPUT: &'static str = "/dev/input";

#[derive(Debug, Deserialize, PartialEq, Eq, Copy, Clone)]
#[serde(try_from = "String")]
struct KeyCode(evdev::Key);

impl TryFrom<String> for KeyCode {
    type Error = eyre::Report;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        evdev::Key::from_str(&value)
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
#[serde(try_from = "String")]
struct Action(Vec<String>);

impl TryFrom<String> for Action {
    type Error = eyre::Report;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let words = shell_words::split(&value)?;
        if words.is_empty() {
            Err(eyre::eyre!("command cannot be empty"))
        } else {
            Ok(Action(words))
        }
    }
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
struct ActionConfig {
    key: KeyCode,
    #[serde(default)]
    on: KeyState,
    action: Action,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
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

fn run_action(action: &ActionConfig, send: &SyncSender<(String, Child)>) -> eyre::Result<()> {
    info!("Running command '{:?}'", action.action.0);
    let (program, args) = action
        .action
        .0
        .split_first()
        .ok_or_else(|| eyre::eyre!("command is empty"))?;
    let child = Command::new(program).args(args).spawn()?;
    send.send((program.clone(), child))?;
    Ok(())
}

fn listen_device_loop(
    config: &Config,
    device: &mut evdev::Device,
    send: &SyncSender<(String, Child)>,
) -> eyre::Result<()> {
    loop {
        for event in device.fetch_events()? {
            debug!("Received input event {:?}", event);
            if let Some(action) = config.action_for(&event)? {
                if let Err(error) = run_action(action, send) {
                    error!("Error while running action: {:?}", error)
                }
            }
        }
    }
}

fn listen_device(config: &Config, device: &mut evdev::Device, send: &SyncSender<(String, Child)>) {
    info!("Open matching input device '{}'", config.name);
    match listen_device_loop(config, device, send) {
        Ok(_) => info!("Device closed"),
        Err(error) => info!("Device closed ({:?})", error),
    }
}

fn log_command_results(recv: Receiver<(String, Child)>) -> eyre::Result<()> {
    debug!("Start logging command results");
    loop {
        let (program, mut child) = recv.recv()?;
        match child.wait() {
            Ok(status) if status.success() => debug!("Command '{}' exited successfully", program),
            Ok(status) => error!("Command '{}' exited unsuccessfully ({})", program, status),
            Err(error) => error!(
                "Error while waiting for command '{}' to finish: {:?}",
                program, error
            ),
        }
    }
}

fn main() -> eyre::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::new().default_filter_or("info")).try_init()?;

    let config: Config = {
        use eyre::Context;
        const CONFIG_ENV_KEY: &'static str = "FERNBEDIENUNG_CONFIG";
        let config_string =
            std::env::var(CONFIG_ENV_KEY).context(format!("{} not set", CONFIG_ENV_KEY))?;
        toml::from_str(&config_string)?
    };

    let (send, recv) = std::sync::mpsc::sync_channel(100);
    std::thread::spawn(|| log_command_results(recv));

    info!("Enumerating initial devices...");
    for (_, mut device) in evdev::enumerate() {
        if device_matches(&config, &device) {
            listen_device(&config, &mut device, &send);
            break;
        }
    }

    info!("Listening for inotify events...");
    let mut inotify = Inotify::init()?;
    inotify.add_watch(
        DEV_INPUT,
        WatchMask::ATTRIB | WatchMask::CREATE | WatchMask::MOVED_TO,
    )?;

    let mut buffer = [0; 1024];
    loop {
        let events = inotify.read_events_blocking(&mut buffer)?;
        for event in events {
            debug!("Received inotify event {:?}", event);
            if let Some(name) = event.name {
                let path = Path::new(DEV_INPUT).join(name);
                debug!("{}: trying to open device", path.display());
                let device = evdev::Device::open(&path);
                match device {
                    Ok(mut device) if device_matches(&config, &device) => {
                        listen_device(&config, &mut device, &send);
                    }
                    Err(error) => debug!("{}: failed to open device: {:?}", path.display(), error),
                    _ => (),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use indoc::indoc;

    #[test]
    fn should_parse_config_file() {
        let s = indoc!(
            r#"
            name = "testdevice"

            [[actions]]
            key = "KEY_KPENTER"
            on = "release"
            action = "/bin/sh -c 'echo 1 2 3'"

            [[actions]]
            key = "KEY_UP"
            action = "abc"

            [[actions]]
            key = "KEY_A"
            on = "repeat"
            action = "echo abc"
        "#
        );

        let config: Config = toml::from_str(s).unwrap();

        assert_eq!(
            config,
            Config {
                name: "testdevice".to_string(),
                actions: vec![
                    ActionConfig {
                        key: KeyCode(evdev::Key::KEY_KPENTER),
                        on: KeyState::Release,
                        action: Action(vec![
                            "/bin/sh".to_string(),
                            "-c".to_string(),
                            "echo 1 2 3".to_string()
                        ]),
                    },
                    ActionConfig {
                        key: KeyCode(evdev::Key::KEY_UP),
                        on: KeyState::Press,
                        action: Action(vec!["abc".to_string()]),
                    },
                    ActionConfig {
                        key: KeyCode(evdev::Key::KEY_A),
                        on: KeyState::Repeat,
                        action: Action(vec!["echo".to_string(), "abc".to_string()]),
                    },
                ],
            }
        );
    }
}
