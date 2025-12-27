use std::path::PathBuf;

use iced::futures::{channel::mpsc, stream::BoxStream, StreamExt};
use iced::Subscription;
use notify::{Config, Event, RecursiveMode, Watcher};

use super::Message;

pub fn subscription(root: PathBuf) -> Subscription<Message> {
    Subscription::run_with(WatchRoot(root), watch_stream)
}

#[derive(Clone, Hash)]
struct WatchRoot(PathBuf);

fn watch_stream(root: &WatchRoot) -> BoxStream<'static, Message> {
    let root = root.0.clone();
    let (mut sender, receiver) = mpsc::channel(64);

    std::thread::spawn(move || {
        let handler = move |result: notify::Result<Event>| {
            if result.is_ok() {
                let _ = sender.try_send(Message::FilesChanged);
            }
        };

        let Ok(mut watcher) = notify::recommended_watcher(handler) else {
            return;
        };

        if watcher.configure(Config::default()).is_err() {
            return;
        }

        if watcher.watch(&root, RecursiveMode::Recursive).is_err() {
            return;
        }

        loop {
            std::thread::park();
        }
    });

    receiver.boxed()
}
