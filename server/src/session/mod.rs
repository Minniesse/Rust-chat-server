use std::sync::Arc;

use comms::{
    command::UserCommand,
    event::{self, RoomDetail},
    transport,
};
use nanoid::nanoid;
use tokio::{net::TcpStream, sync::broadcast};
use tokio_stream::StreamExt;

use crate::room_manager::RoomManager;

use self::chat_session::ChatSession;

mod chat_session;

pub async fn handle_user_session(
    room_manager: Arc<RoomManager>,
    mut quit_rx: broadcast::Receiver<()>,
    stream: TcpStream,
) -> anyhow::Result<()> {
    let session_id = nanoid!();
    // Generate a random id for the user, since we don't have a login system
    let user_id = String::from(&nanoid!()[0..5]);
    // Split the tcp stream into a command stream and an event writer with better ergonomics
    let (mut commands, mut event_writer) = transport::server::split_tcp_stream(stream);

    // Welcoming the user with a login successful event and necessary information about the server
    event_writer
        .write(&event::Event::LoginSuccessful(
            event::LoginSuccessfulReplyEvent {
                session_id: session_id.clone(),
                user_id: user_id.clone(),
                rooms: room_manager
                    .chat_room_metadata()
                    .iter()
                    .map(|metadata| RoomDetail {
                        name: metadata.name.clone(),
                        description: metadata.description.clone(),
                    })
                    .collect(),
            },
        ))
        .await?;

    // Create a chat session with the given room manager
    // Chat Session will abstract the user session handling logic for multiple rooms
    let mut chat_session = ChatSession::new(&session_id, &user_id, room_manager);

    loop {
        tokio::select! {
            cmd = commands.next() => match cmd {
                None | Some(Ok(UserCommand::Quit(_))) => {
                    chat_session.leave_all_rooms().await?;
                    break;
                }
                Some(Ok(cmd)) => match cmd {
                    // For user session related commands, we need to handle them in the chat session
                    UserCommand::JoinRoom(_) | UserCommand::SendMessage(_) | UserCommand::LeaveRoom(_) | UserCommand::GetHistory(_) => {
                        chat_session.handle_user_command(cmd).await?;
                    }
                    _ => {}
                }
                _ => {}
            },
            Ok(event) = chat_session.recv() => {
                event_writer.write(&event).await?;
            }
            Ok(_) = quit_rx.recv() => {
                drop(event_writer);
                println!("Gracefully shutting down user tcp stream.");
                break;
            }
        }
    }

    Ok(())
}