use inotify::{Inotify, WatchMask};

fn main() -> eyre::Result<()> {
    color_eyre::install()?;

    let mut inotify = Inotify::init()?;
    inotify.add_watch(
        "/dev/input",
        WatchMask::ATTRIB | WatchMask::CREATE | WatchMask::MOVED_TO,
    )?;

    let mut buffer = [0; 1024];
    loop {
        let events = inotify.read_events_blocking(&mut buffer)?;
        for event in events {
            println!("event: {:?}", event);
        }
    }
}
