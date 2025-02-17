use crate::audio::audio_manager::AudioManager;
use crate::message::message::{Message, MessageHeader, MessageType};
use crate::network_manager::network_manager::{ConnectionCommand, NetworkManager, ServerOrClient};
use crate::realms::realm::ChannelType;
use crate::realms::realm_desc::RealmDescription;
use crate::types::{ChannelIdSize, MessageIdSize, RealmIdSize, UserIdSize};
use crate::user::user::User;
use quinn::{Connection, ConnectionError, Endpoint};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::mpsc::{channel, error::TryRecvError, Receiver, Sender};
use tokio::sync::Mutex;

#[derive(Debug)]
pub enum ClientError {
    NotLoggedIn,
    FailedToJoinChannel,
}

pub enum AudioCommand {
    Start,
    Stop,
    Mute,
    Unmute,
}

#[derive(Debug)]
pub struct Client {
    #[allow(unused)]
    endpoint: Endpoint,
    connection: Connection,
    connection_sender: Arc<Mutex<Option<Sender<ConnectionCommand>>>>,
    messages: Arc<Mutex<VecDeque<Message>>>,

    // The current client, as a User
    // This is an option because it's possible this client isn't registered with the server yet
    user: Arc<Mutex<Option<User>>>,

    username: String,

    // Our known Realms
    realms: Arc<Mutex<Vec<RealmDescription>>>,

    // Sender for audio commands
    audio_sender: Option<Sender<(UserIdSize, Vec<u8>)>>,

    // Audio manager to handle audio recording and playback
    audio_manager: Arc<Mutex<Option<AudioManager>>>,

    // If the user is logged in
    is_logged_in: Arc<Mutex<bool>>,
}

impl Client {
    pub async fn new(server_address: String, server_port: u16, username: String) -> Client {
        let endpoint = NetworkManager::connect_endpoint(
            server_address.clone(),
            server_port,
            ServerOrClient::Client,
        )
        .await;

        let mut address = server_address;
        address.push(':');
        address.push_str(server_port.to_string().as_str());
        let address: std::net::SocketAddr = address.parse().unwrap();

        // Here "localhost" should match the server cert (but this is ignored right now)
        let connect = endpoint.connect(address, "localhost").unwrap();
        let connection = connect.await;

        let connection = match connection {
            Ok(conn) => conn,
            Err(ConnectionError::TimedOut) => {
                eprintln!("[client] Connection timed out. Is the server IP and port correct?");
                std::process::exit(1);
            }
            Err(e) => {
                eprintln!("[client] Error while connecting: {}", e);
                std::process::exit(1);
            }
        };

        // Generate a sender and receiver for audio data
        let (audio_to_am_tx, audio_to_am_rx) = channel(1000);

        // Make our AudioManager and give it our client's endpoint
        let audio_manager = AudioManager::default()
            .endpoint(endpoint.clone())
            .connection(connection.clone())
            .audio_receiver(audio_to_am_rx);

        Client {
            endpoint,
            connection,
            audio_sender: Some(audio_to_am_tx),
            connection_sender: Arc::new(Mutex::new(None)),
            messages: Arc::new(Mutex::new(VecDeque::new())),
            user: Arc::new(Mutex::new(None)),
            username,
            realms: Arc::new(Mutex::new(Vec::new())),
            audio_manager: Arc::new(Mutex::new(Some(audio_manager))),
            is_logged_in: Arc::new(Mutex::new(false)),
        }
    }

    pub async fn get_new_messages(&self) -> Vec<Message> {
        let mut new_messages: Vec<Message> = Vec::new();

        let messages = self.messages.clone();
        let mut messages = messages.lock().await;

        while messages.len() > 0 {
            new_messages.push(messages.pop_front().unwrap());
        }

        new_messages
    }

    pub async fn run_client(&self) {
        self.receive_data().await;
        self.login(self.username.clone()).await;
    }

    async fn login(&self, username: String) {
        let login_message = Message::from(MessageType::LoginAttempt(username));

        // Try to log in
        Client::send(
            login_message.into_vec_u8().unwrap().as_slice(),
            self.connection.clone(),
        )
        .await;
    }

    pub async fn get_realms(&self) -> Vec<RealmDescription> {
        let mut realms = self.realms.lock().await;

        let mut new_realms = Vec::new();

        while realms.len() > 0 {
            new_realms.push(realms.pop().unwrap());
        }

        new_realms
    }

    pub async fn get_user_id(&self) -> Option<UserIdSize> {
        let guard = self.user.lock().await;
        #[allow(clippy::manual_map)]
        match *guard {
            Some(ref user) => Some(user.get_id()),
            None => None,
        }
    }

    pub async fn get_username(&self) -> Option<String> {
        let guard = self.user.lock().await;
        #[allow(clippy::manual_map)]
        match *guard {
            Some(ref user) => Some(user.get_username().to_string()),
            None => None,
        }
    }

    pub async fn is_logged_in(&self) -> bool {
        let logged_in = self.is_logged_in.lock().await;
        *logged_in
    }

    async fn receive_data(&self) {
        let connection = self.connection.clone();
        let messages = self.messages.clone();

        let (tx, mut rx): (Sender<ConnectionCommand>, Receiver<ConnectionCommand>) = channel(1000);

        let connection_sender = self.connection_sender.clone();
        {
            let mut connection_sender = connection_sender.lock().await;
            *connection_sender = Some(tx);
        }

        let user_handle = self.user.clone();
        let is_logged_in = self.is_logged_in.clone();

        let audio_sender = self.audio_sender.clone().unwrap();

        // Spawn a tokio thread to listen for data
        // Here we only need one thread, since there will only be one connection to the server
        tokio::spawn(async move {
            loop {
                // Listen for channel messages to stop listening on this channel
                match rx.try_recv() {
                    Ok(command) => match command {
                        ConnectionCommand::StopReceiving => {
                            break;
                        }
                    },
                    Err(TryRecvError::Empty) => (), // Do nothing here, nothing to receive yet
                    Err(TryRecvError::Disconnected) => {
                        eprintln!("No sender available to receive from");
                        break;
                    }
                }

                let audio_sender = audio_sender.clone();
                let logged_in = is_logged_in.clone();

                let connection = connection.clone();
                let stream = connection.accept_bi().await;
                match stream {
                    Ok((_send_stream, mut read_stream)) => {
                        let message = read_stream.read_to_end(12000000).await.unwrap();

                        let mut messages = messages.lock().await;

                        let msg_clone = message.clone();
                        let msg_clone = Message::from_vec_u8(msg_clone).unwrap();

                        // Handle login attempt
                        match msg_clone.get_message() {
                            MessageType::LoginSuccess(user) => {
                                let mut guard = user_handle.lock().await;
                                let id = user.get_id();
                                *guard = Some(user);

                                let mut logged_in = logged_in.lock().await;
                                *logged_in = true;

                                // Now that we've logged in, let's request any realms we're part of
                                Client::send(
                                    Message::from(MessageType::GetRealms(id))
                                        .into_vec_u8()
                                        .unwrap()
                                        .as_slice(),
                                    connection.clone(),
                                )
                                .await;
                            }
                            MessageType::Audio(audio) => {
                                audio_sender.send((audio.0.user_id, audio.1)).await.unwrap();
                            }
                            _ => {
                                //println!("{:?}", Message::from_vec_u8(message.clone()).unwrap());
                                messages.push_back(Message::from_vec_u8(message).unwrap());
                            }
                        }
                    }
                    Err(quinn::ConnectionError::ApplicationClosed(ac)) => {
                        println!(
                            "Connection closed. Code: {}, Reason: {}",
                            ac.error_code,
                            String::from_utf8(ac.reason.to_vec()).unwrap()
                        );
                        let mut messages = messages.lock().await;
                        messages.push_back(Message::from(MessageType::Disconnect));
                        break;
                    }
                    Err(quinn::ConnectionError::LocallyClosed) => {
                        let mut messages = messages.lock().await;
                        messages.push_back(Message::from(MessageType::Disconnect));
                        break;
                    }
                    _ => {
                        eprintln!("[client] unhandled stream error");
                        let mut messages = messages.lock().await;
                        messages.push_back(Message::from(MessageType::Disconnect));
                        break;
                    }
                }
            }
        });
    }

    pub async fn send_mention_message(
        &self,
        realm_id: RealmIdSize,
        channel_id: ChannelIdSize,
        message_chunks: Vec<(String, Option<UserIdSize>)>,
    ) {
        // Get our user id
        let guard = self.user.lock().await;
        if let Some(ref user) = *guard {
            // Send our mention message
            let message = Message::from(MessageType::Text((
                MessageHeader::new(user.get_id(), realm_id, channel_id),
                message_chunks,
            )));
            let serialized = message.into_vec_u8().unwrap();
            Client::send(serialized.as_slice(), self.connection.clone()).await;
        }
    }

    pub async fn send_reply_message(
        &self,
        realm_id: RealmIdSize,
        channel_id: ChannelIdSize,
        message_id: MessageIdSize,
        message_chunks: Vec<(String, Option<UserIdSize>)>,
    ) {
        // Get our user id
        let guard = self.user.lock().await;
        if let Some(ref user) = *guard {
            // Send our reply message
            let message = Message::from(MessageType::Reply((
                MessageHeader::new(user.get_id(), realm_id, channel_id),
                message_id,
                message_chunks,
            )));
            let serialized = message.into_vec_u8().unwrap();
            Client::send(serialized.as_slice(), self.connection.clone()).await;
        }
    }

    pub async fn send_image(
        &self,
        realm_id: RealmIdSize,
        channel_id: ChannelIdSize,
        image: Vec<u8>,
    ) {
        // Get our user id
        let guard = self.user.lock().await;
        if let Some(ref user) = *guard {
            // Send our mention message
            let message = Message::from(MessageType::Image((
                MessageHeader::new(user.get_id(), realm_id, channel_id),
                image,
            )));
            let serialized = message.into_vec_u8().unwrap();
            Client::send(serialized.as_slice(), self.connection.clone()).await;
        }
    }

    pub async fn hang_up(&self, realm_id: &RealmIdSize, channel_id: &ChannelIdSize) {
        // Get our user id
        let guard = self.user.lock().await;
        if let Some(ref user) = *guard {
            // Send our join message
            let message = Message::from(MessageType::UserLeftVoiceChannel(MessageHeader::new(
                user.get_id(),
                *realm_id,
                *channel_id,
            )));
            let serialized = message.into_vec_u8().unwrap();
            Client::send(serialized.as_slice(), self.connection.clone()).await;

            // If we have an AudioManager, tell it to stop
            let mut am = self.audio_manager.lock().await;
            match &mut *am {
                Some(manager) => {
                    manager.disconnect().await;
                }
                None => (),
            }
        }
    }

    pub async fn join_channel(
        &self,
        realm_id: RealmIdSize,
        channel_type: ChannelType,
        channel_id: ChannelIdSize,
    ) {
        match channel_type {
            // For, all text messages get sent to everyone
            ChannelType::TextChannel => {
                // Get our user id
                let guard = self.user.lock().await;
                if let Some(ref user) = *guard {
                    // Send our join message
                    let message = Message::from(MessageType::JoinChannel((
                        MessageHeader::new(user.get_id(), realm_id, channel_id),
                        channel_type,
                    )));
                    let serialized = message.into_vec_u8().unwrap();
                    Client::send(serialized.as_slice(), self.connection.clone()).await;
                }
            }
            ChannelType::VoiceChannel => {
                // Get our user id
                let guard = self.user.lock().await;
                if let Some(ref user) = *guard {
                    // Send our join message
                    let message = Message::from(MessageType::UserJoinedVoiceChannel(
                        MessageHeader::new(user.get_id(), realm_id, channel_id),
                    ));
                    let serialized = message.into_vec_u8().unwrap();
                    Client::send(serialized.as_slice(), self.connection.clone()).await;
                }
            }
        }
    }

    async fn send(buffer: &[u8], connection: Connection) {
        if let Ok((mut send, _recv)) = connection.open_bi().await {
            send.write_all(buffer).await.unwrap();
            send.finish().await.unwrap();
        }
    }

    pub async fn disconnect(&mut self) {
        // Tell the server we are disconnecting
        // Get our user id
        let guard = self.user.lock().await;
        if let Some(ref user) = *guard {
            // Send our disconnecting message
            let message = Message::from(MessageType::Disconnecting(user.get_id()));
            let serialized = message.into_vec_u8().unwrap();
            Client::send(serialized.as_slice(), self.connection.clone()).await;
        }

        // Send our QUIC disconnect
        self.connection.close(0u32.into(), b"done");
        self.connection.closed().await;

        // Tell our receiving thread to stop receiving data
        let connection_sender = self.connection_sender.clone();
        let mut conn_sender = connection_sender.lock().await;
        if let Some(conn_sender) = conn_sender.take() {
            let _ = conn_sender.send(ConnectionCommand::StopReceiving).await;
        }
    }

    pub async fn connect_voice(&mut self, realm_id: RealmIdSize, channel_id: ChannelIdSize) {
        if let Some(user_id) = self.get_user_id().await {
            let mut am = self.audio_manager.lock().await;
            if let Some(ref mut manager) = *am {
                // Set our user id before recording and broadcasting
                manager.set_user_id(user_id);

                // Start recording for broadcasting
                manager
                    .start_recording(MessageHeader::new(user_id, realm_id, channel_id))
                    .await;
                manager.start_listening().await;
            }
        }
    }

    pub async fn add_channel(
        &self,
        realm_id: RealmIdSize,
        channel_type: ChannelType,
        channel_name: String,
    ) {
        if let Some(user_id) = self.get_user_id().await {
            // Send our add channel message
            let message = Message::from(MessageType::AddChannel((
                MessageHeader::new(user_id, realm_id, 0),
                channel_type,
                channel_name,
            )));
            let serialized = message.into_vec_u8().unwrap();
            Client::send(serialized.as_slice(), self.connection.clone()).await;
        }
    }

    pub async fn remove_channel(
        &self,
        realm_id: RealmIdSize,
        channel_type: ChannelType,
        channel_id: ChannelIdSize,
    ) {
        if let Some(user_id) = self.get_user_id().await {
            // Send our add channel message
            let message = Message::from(MessageType::RemoveChannel((
                MessageHeader::new(user_id, realm_id, channel_id),
                channel_type,
            )));
            let serialized = message.into_vec_u8().unwrap();
            Client::send(serialized.as_slice(), self.connection.clone()).await;
        }
    }

    pub async fn add_realm(&self, realm_name: String) {
        if let Some(user_id) = self.get_user_id().await {
            // Send our add realm message
            let message = Message::from(MessageType::AddRealm((
                MessageHeader::new(user_id, 0, 0),
                realm_name,
            )));
            let serialized = message.into_vec_u8().unwrap();
            Client::send(serialized.as_slice(), self.connection.clone()).await;
        }
    }

    pub async fn remove_realm(&self, realm_id: RealmIdSize) {
        if let Some(user_id) = self.get_user_id().await {
            // Send our remove realm message
            let message = Message::from(MessageType::RemoveRealm((
                MessageHeader::new(user_id, 0, 0),
                realm_id,
            )));
            let serialized = message.into_vec_u8().unwrap();
            Client::send(serialized.as_slice(), self.connection.clone()).await;
        }
    }

    pub async fn add_friend(&self, friend_id: UserIdSize) {
        if let Some(user_id) = self.get_user_id().await {
            // Send our add friend message
            let message = Message::from(MessageType::NewFriendRequest((
                MessageHeader::new(user_id, 0, 0),
                friend_id,
            )));
            let serialized = message.into_vec_u8().unwrap();
            Client::send(serialized.as_slice(), self.connection.clone()).await;
        }
    }

    pub async fn remove_friend(&self, friend_id: UserIdSize) {
        if let Some(user_id) = self.get_user_id().await {
            // Send our remove friend message
            let message = Message::from(MessageType::RemoveFriend((
                MessageHeader::new(user_id, 0, 0),
                friend_id,
            )));
            let serialized = message.into_vec_u8().unwrap();
            Client::send(serialized.as_slice(), self.connection.clone()).await;
        }
    }

    pub async fn send_typing(&self, realm_id: RealmIdSize, channel_id: ChannelIdSize) {
        if let Some(user_id) = self.get_user_id().await {
            // Send our typing message
            let message = Message::from(MessageType::Typing(MessageHeader::new(
                user_id, realm_id, channel_id,
            )));
            let serialized = message.into_vec_u8().unwrap();
            Client::send(serialized.as_slice(), self.connection.clone()).await;
        }
    }
}
