use comms::event::{self, Event, HistoryMessage};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use std::collections::VecDeque;

use super::{
    user_registry::UserRegistry, user_session_handle::UserSessionHandle, SessionAndUserId,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRoomMetadata {
    pub name: String,
    pub description: String,
}

const BROADCAST_CHANNEL_CAPACITY: usize = 100;
// MODIFIED: Added constant for history size
const MAX_HISTORY_SIZE: usize = 10;

#[derive(Debug)]
pub struct ChatRoom {
    metadata: ChatRoomMetadata,
    broadcast_tx: broadcast::Sender<Event>,
    user_registry: UserRegistry,
    // MODIFIED: Added message history
    message_history: VecDeque<HistoryMessage>,
}

impl ChatRoom {
    pub fn new(metadata: ChatRoomMetadata) -> Self {
        let (broadcast_tx, _) = broadcast::channel(BROADCAST_CHANNEL_CAPACITY);

        ChatRoom {
            metadata,
            broadcast_tx,
            user_registry: UserRegistry::new(),
            // MODIFIED: Initialize message history
            message_history: VecDeque::with_capacity(MAX_HISTORY_SIZE),
        }
    }

    // MODIFIED: Added method to get chat history
    pub fn get_history(&self) -> Vec<HistoryMessage> {
        self.message_history.iter().cloned().collect()
    }

    // MODIFIED: Added method to store message in history
    fn store_message(&mut self, user_id: String, content: String) {
        if self.message_history.len() >= MAX_HISTORY_SIZE {
            self.message_history.pop_front();
        }
        self.message_history.push_back(HistoryMessage {
            user_id,
            content,
        });
    }

    pub fn get_unique_user_ids(&self) -> Vec<String> {
        self.user_registry.get_unique_user_ids()
    }

    pub fn join(
        &mut self,
        session_and_user_id: &SessionAndUserId,
    ) -> (broadcast::Receiver<Event>, UserSessionHandle) {
        let broadcast_tx = self.broadcast_tx.clone();
        let broadcast_rx = broadcast_tx.subscribe();
        let user_session_handle = UserSessionHandle::new(
            self.metadata.name.clone(),
            broadcast_tx,
            session_and_user_id.clone(),
        );

        if self.user_registry.insert(&user_session_handle) {
            let _ = self.broadcast_tx.send(Event::RoomParticipation(
                event::RoomParticipationBroadcastEvent {
                    user_id: session_and_user_id.user_id.clone(),
                    room: self.metadata.name.clone(),
                    status: event::RoomParticipationStatus::Joined,
                },
            ));
        }

        (broadcast_rx, user_session_handle)
    }

    pub fn leave(&mut self, user_session_handle: UserSessionHandle) {
        if self.user_registry.remove(&user_session_handle) {
            let _ = self.broadcast_tx.send(Event::RoomParticipation(
                event::RoomParticipationBroadcastEvent {
                    user_id: String::from(user_session_handle.user_id()),
                    room: self.metadata.name.clone(),
                    status: event::RoomParticipationStatus::Left,
                },
            ));
        }
    }

    // MODIFIED: Added method to handle incoming messages and store in history
    pub fn handle_message(&mut self, user_id: String, content: String) -> anyhow::Result<()> {
        // Store the message in history
        self.store_message(user_id.clone(), content.clone());

        // Broadcast the message
        self.broadcast_tx
            .send(Event::UserMessage(
                event::UserMessageBroadcastEvent {
                    room: self.metadata.name.clone(),
                    user_id,
                    content,
                },
            ))
            .map_err(|e| anyhow::anyhow!("could not write to the broadcast channel: {}", e))?;

        Ok(())
    }

    // MODIFIED: Added method to send chat history to a specific session
    pub fn send_history_to_session(&self, _session_id: &str) -> anyhow::Result<()> {
        self.broadcast_tx
            .send(Event::ChatHistory(event::ChatHistoryReplyEvent {
                room: self.metadata.name.clone(),
                messages: self.get_history(),
            }))
            .map_err(|e| anyhow::anyhow!("could not send chat history: {}", e))?;

        Ok(())
    }
}