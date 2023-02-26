use std::borrow::Cow;

use tungstenite::{Message, protocol::{CloseFrame, frame::coding::CloseCode}};

pub enum WorldHostOutMessage {
    Error(String)
}

impl WorldHostOutMessage {
    pub fn write(&self) -> Message {
        match self {
            WorldHostOutMessage::Error(message) => Message::Close(Option::Some(CloseFrame {
                code: CloseCode::Protocol,
                reason: Cow::Owned(message.to_owned())
            }))
        }
    }
}
