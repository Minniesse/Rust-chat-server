# Chat History Design Write-up

## Overview

The chat history implementation enables users to see previous messages when joining a room, enhancing the user experience by providing conversation context. The feature stores a configurable number of recent messages per room on the server side and synchronizes this history to clients upon joining a room.

## Design Architecture

1. **Commands**: Client-to-server instructions (e.g., join room, send message, get history)
2. **Events**: Server-to-client notifications (e.g., user joined, new message, history data)

The chat history feature extends this pattern with new commands and events while maintaining the existing architecture's principles.

## Server-Side Implementation

### Data Structure

The server stores chat history using a `VecDeque` (double-ended queue) in each `ChatRoom` instance:

```rust
pub struct ChatRoom {
    // Existing fields...
    message_history: VecDeque<HistoryMessage>,
}
```

This data structure was chosen because:
- It enables efficient rotation of messages (removing oldest when capacity is reached)
- It maintains message order (first in, first out)
- It supports efficient iteration for sending history to clients

### History Management

The server maintains a fixed-size history per room (10 messages by default):

```rust
const MAX_HISTORY_SIZE: usize = 10;
```

When a new message arrives, it's added to history using the `store_message` method:

```rust
fn store_message(&mut self, user_id: String, content: String) {
    if self.message_history.len() >= MAX_HISTORY_SIZE {
        self.message_history.pop_front(); // Remove oldest message
    }
    self.message_history.push_back(HistoryMessage {
        user_id,
        content,
    });
}
```

This ensures the history never exceeds the configured capacity while maintaining the most recent messages.

### History Retrieval Flow

1. The client sends a `GetHistory` command to request history for a specific room
2. The server processes this command in the session handler
3. The room manager locates the requested room and acquires a lock
4. The room creates a `ChatHistoryReplyEvent` containing all stored messages
5. This event is sent through the broadcast channel to the requesting client

The `send_history_to_session` method handles packaging and sending history:

```rust
pub fn send_history_to_session(&self, _session_id: &str) -> anyhow::Result<()> {
    self.broadcast_tx
        .send(Event::ChatHistory(event::ChatHistoryReplyEvent {
            room: self.metadata.name.clone(),
            messages: self.get_history(),
        }))
        .map_err(|e| anyhow::anyhow!("could not send chat history: {}", e))?;

    Ok(())
}
```

### Automatic History Retrieval

The implementation automatically requests history when a user joins a room:

```rust
// After successfully joining a room
self.room_manager.get_room_history(&cmd.room, &self.session_and_user_id.session_id).await?;
```

This ensures users immediately see previous messages without manual action.

## Client-Side Implementation

### Protocol Extension

New command and event types were added to the communication protocol:

1. **GetHistoryCommand**: Requests history for a specific room
   ```rust
   pub struct GetHistoryCommand {
       #[serde(rename = "r")]
       pub room: String,
   }
   ```

2. **ChatHistoryReplyEvent**: Contains the message history for a room
   ```rust
   pub struct ChatHistoryReplyEvent {
       #[serde(rename = "r")]
       pub room: String,
       #[serde(rename = "m")]
       pub messages: Vec<HistoryMessage>,
   }
   ```

3. **HistoryMessage**: Represents a single historical message
   ```rust
   pub struct HistoryMessage {
       #[serde(rename = "u")]
       pub user_id: String,
       #[serde(rename = "c")]
       pub content: String,
   }
   ```

### TUI State Management

The TUI client stores messages in a `CircularQueue` for each room:

```rust
pub struct RoomData {

    pub messages: CircularQueue<MessageBoxItem>,
}
```

When history is received, the client:
1. Clears any existing messages in the room
2. Adds each historical message to the queue
3. Updates the UI to reflect the new messages

```rust
event::Event::ChatHistory(event) => {
    if let Some(room_data) = self.room_data_map.get_mut(&event.room) {
        room_data.messages.clear();
        
        for msg in &event.messages {
            room_data.messages.push(MessageBoxItem::Message {
                user_id: msg.user_id.clone(),
                content: msg.content.clone(),
            });
        }
    }
}
```