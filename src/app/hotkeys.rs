use iced::{Subscription, keyboard};

use super::messages::Message;

pub fn subscription() -> Subscription<Message> {
    keyboard::on_key_press(|key, modifiers| match key {
        keyboard::Key::Character(c) if c.eq_ignore_ascii_case("s") && modifiers.command() => {
            Some(Message::Save)
        }
        keyboard::Key::Named(keyboard::key::Named::Enter) if modifiers.command() => {
            Some(Message::Send)
        }
        _ => None,
    })
}
