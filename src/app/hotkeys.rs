use iced::{Subscription, keyboard};

use super::messages::Message;

pub fn subscription() -> Subscription<Message> {
    keyboard::listen().filter_map(|event| match event {
        keyboard::Event::KeyPressed { key, modifiers, .. } => match key {
            keyboard::Key::Character(c) if c.eq_ignore_ascii_case("s") && modifiers.command() => {
                Some(Message::Save)
            }
            keyboard::Key::Named(keyboard::key::Named::Enter) if modifiers.command() => {
                Some(Message::Send)
            }
            _ => None,
        },
        _ => None,
    })
}
