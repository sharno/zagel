use std::path::PathBuf;
use std::pin::Pin;
use std::sync::mpsc as std_mpsc;
use std::task::{Context, Poll};

use iced::Subscription;
use iced::futures::{Stream, StreamExt, channel::mpsc, stream::BoxStream};
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};

use super::Message;

fn send_watcher_unavailable(sender: &mut mpsc::Sender<Message>, message: String) {
    eprintln!("watcher: {message}");
    if let Err(err) = sender.try_send(Message::WatcherUnavailable(message)) {
        eprintln!("watcher: failed to send watcher status: {err}");
    }
}

#[derive(Clone, Hash)]
struct WatchRoots(Vec<PathBuf>);

struct WatchStream {
    receiver: mpsc::Receiver<Message>,
    shutdown: std_mpsc::Sender<()>,
}

impl Stream for WatchStream {
    type Item = Message;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let receiver = Pin::new(&mut self.get_mut().receiver);
        receiver.poll_next(cx)
    }
}

impl Drop for WatchStream {
    fn drop(&mut self) {
        let _ = self.shutdown.send(());
    }
}

pub fn subscription_many(mut roots: Vec<PathBuf>) -> Subscription<Message> {
    roots.sort_by(|a, b| a.to_string_lossy().cmp(&b.to_string_lossy()));
    roots.dedup();
    if roots.is_empty() {
        return Subscription::none();
    }
    Subscription::run_with(WatchRoots(roots), watch_stream_many)
}

fn watch_stream_many(roots: &WatchRoots) -> BoxStream<'static, Message> {
    let roots = roots.0.clone();
    let (sender, receiver) = mpsc::channel(64);
    let (shutdown_tx, shutdown_rx) = std_mpsc::channel();

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

        let mut watched_any = false;
        for root in &roots {
            if let Err(err) = watcher.watch(root, RecursiveMode::Recursive) {
                send_watcher_unavailable(
                    &mut status_sender,
                    format!(
                        "Watcher unavailable: failed to watch {}: {err}",
                        root.display()
                    ),
                );
            } else {
                watched_any = true;
            }
        }

        if !watched_any {
            send_watcher_unavailable(
                &mut status_sender,
                "Watcher unavailable: no valid folders to watch".to_string(),
            );
            return;
        }

        // Block until the subscription drops so the watcher shuts down cleanly.
        let _ = shutdown_rx.recv();
    });

    WatchStream {
        receiver,
        shutdown: shutdown_tx,
    }
    .boxed()
}
