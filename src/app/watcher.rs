use std::path::PathBuf;

use iced::futures::{channel::mpsc, stream::BoxStream, StreamExt};
use iced::Subscription;
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};

use super::Message;

fn send_watcher_unavailable(sender: &mut mpsc::Sender<Message>, message: String) {
    eprintln!("watcher: {message}");
    if let Err(err) = sender.try_send(Message::WatcherUnavailable(message)) {
        eprintln!("watcher: failed to send watcher status: {err}");
    }
}

pub fn subscription(root: PathBuf) -> Subscription<Message> {
    Subscription::run_with(WatchRoot(root), watch_stream)
}

#[derive(Clone, Hash)]
struct WatchRoot(PathBuf);

fn watch_stream(root: &WatchRoot) -> BoxStream<'static, Message> {
    let root = root.0.clone();
    let (sender, receiver) = mpsc::channel(64);

    std::thread::spawn(move || {
        let mut status_sender = sender.clone();
        let mut event_sender = sender.clone();
        let handler = move |result: notify::Result<Event>| match result {
            Ok(_) => {
                let _ = event_sender.try_send(Message::FilesChanged);
            }
            Err(err) => {
                eprintln!("watcher: event error: {err}");
            }
        };

        let mut watcher = match RecommendedWatcher::new(handler, Config::default()) {
            Ok(watcher) => watcher,
            Err(err) => {
                send_watcher_unavailable(
                    &mut status_sender,
                    format!("Watcher unavailable: failed to create watcher: {err}"),
                );
                return;
            }
        };

        if let Err(err) = watcher.configure(Config::default()) {
            send_watcher_unavailable(
                &mut status_sender,
                format!("Watcher unavailable: failed to configure watcher: {err}"),
            );
            return;
        }

        if let Err(err) = watcher.watch(&root, RecursiveMode::Recursive) {
            send_watcher_unavailable(
                &mut status_sender,
                format!(
                    "Watcher unavailable: failed to start watching {}: {err}",
                    root.display()
                ),
            );
            return;
        }

        loop {
            std::thread::park();
        }
    });

    receiver.boxed()
}
