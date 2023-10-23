use std::collections::HashMap;
use std::error;
use std::io;
use std::path::Path;
use tui::{
    backend::CrosstermBackend,
    style::Color,
    text::{Line, Span},
    Terminal,
};

use crate::client::Client;
use crate::message::MessageType;
use crate::realms::realm::ChannelType;
use crate::realms::realms_manager::RealmsManager;
use crate::tui::command::Command;
use crate::tui::{
    event::{Event, EventHandler},
    handler::handle_key_events,
    stateful_list::StatefulList,
    tui::Tui,
};
use crate::types::{ChannelIdSize, RealmIdSize, UserIdSize};
use tui::style::Style;

use super::input_buffer::InputBuffer;
use super::popups::popup_traits::PopupTraits;
use crate::tui::popups::{
    add_channel_popup::AddChannelPopup, general_popup::GeneralPopup, member_popup::MemberPopup,
    remove_channel_popup::RemoveChannelPopup, yes_no_popup::YesNoPopup,
};

use chrono::Local;

/// Application result type.
pub type AppResult<T> = std::result::Result<T, Box<dyn error::Error>>;

#[derive(Debug)]
pub enum PopupType {
    General,
    YesNo,
    AddChannel,
    RemoveChannel,
    Member,
}

#[derive(Debug)]
pub enum Screen {
    Main,
    Settings,
    Personal,
}

#[derive(Debug)]
pub enum InputMode {
    Normal,
    Editing,
    ChannelType,
    TextChannel,
    VoiceChannel,
    Members,
    Realms,
    Loading,
    Popup,
}

#[derive(Debug)]
pub enum UiElement {
    TextChannelLabel,
    VoiceChannelLabel,
    None,
}

#[derive(Debug)]
pub enum Pane {
    RealmsPane,
    ChannelsPane,
    ChatPane,
    MembersPane,
    InputPane,
    None,
}

impl Pane {
    pub fn to_str(&self) -> String {
        match self {
            Pane::RealmsPane => String::from("Realms"),
            Pane::ChannelsPane => String::from("Channels"),
            Pane::ChatPane => String::from("Chat"),
            Pane::MembersPane => String::from("Members"),
            Pane::InputPane => String::from("Input"),
            _ => String::new(),
        }
    }
}

pub trait KaguFormatting {
    fn with_focus(self) -> Self;
    fn add_hashtag_with_space(self) -> Self;
    fn add_hashtag(self) -> Self;
    fn prepend_str(self, text: &str) -> Self;
    fn with_pre_post_spaces(self) -> Self;
}

impl KaguFormatting for String {
    fn with_focus(mut self) -> Self {
        self.insert(0, '[');
        self.push(']');
        self
    }

    fn with_pre_post_spaces(mut self) -> Self {
        self.insert(0, ' ');
        self.push(' ');
        self
    }

    fn add_hashtag_with_space(mut self) -> Self {
        self.insert_str(0, "# ");
        self
    }

    fn add_hashtag(mut self) -> Self {
        self.insert(0, '#');
        self
    }

    fn prepend_str(mut self, text: &str) -> Self {
        self.insert_str(0, text);
        self
    }
}

/// Application.
#[derive(Debug)]
pub struct App<'a> {
    pub user_id: Option<UserIdSize>,
    /// Current input mode
    pub input_mode: InputMode,
    /// Current UI element selected
    pub ui_element: UiElement,
    /// Current pane in focus
    pub current_pane: Pane,
    /// Current screen type to draw
    pub current_screen: Screen,
    /// Is the application running?
    pub running: bool,
    /// Client to handle all interactions with the server
    pub client: Client,
    /// Chat history
    pub chat_history: Vec<Line<'a>>,
    /// User ID to usernames
    pub user_id_to_username: HashMap<UserIdSize, String>,
    /// Realms manager to manage our realms and channels
    pub realms_manager: RealmsManager,
    /// Current users online
    pub users_online: StatefulList<(UserIdSize, String)>,
    /// Realms
    pub realms: StatefulList<(RealmIdSize, String)>,
    /// Text channels to display
    pub text_channels: StatefulList<(ChannelIdSize, String)>,
    /// Voice channels to display
    pub voice_channels: StatefulList<(ChannelIdSize, String, Vec<UserIdSize>)>,
    /// Status indicating if we are connected via voice
    pub is_voice_connected: bool,
    /// Current Realm we are in
    pub current_realm_id: Option<RealmIdSize>,
    /// Current text channel we're in
    pub current_text_channel: Option<(ChannelIdSize, String)>,
    /// Current voice channel we're in
    pub current_voice_channel: Option<ChannelIdSize>,
    /// State for showing text commands
    pub is_commanding: bool,
    /// Vec of available user commands
    pub commands: Vec<Command>,
    /// Capture command text to match againt commands
    pub command_buffer: String,
    /// List to select a command
    pub command_list: StatefulList<(Command, String)>,
    /// Command the user is currently commanding
    pub current_command: Option<Command>,
    /// State for showing names to @mention
    pub is_mentioning: bool,
    /// Capture mention text to match against members
    pub mention_buffer: String,
    /// List to select a member while mentioning
    pub mention_list: StatefulList<(UserIdSize, String)>,
    /// Users mentioned in a TextMention
    pub users_mentioned: Vec<UserIdSize>,
    /// Struct to hold our input
    pub input_buffer: InputBuffer,
    /// If we are showing a popup to the user
    pub is_popup_shown: bool,
    /// Type of popup to render
    pub popup_type: PopupType,
    /// Text shown in popup
    pub popup_text: String,
    /// Title for the popup window
    pub popup_title: String,
    /// General popup
    pub general_popup: GeneralPopup,
    /// Add channel popup
    pub add_channel_popup: AddChannelPopup,
    /// Member info popup
    pub member_popup: MemberPopup,
    // Yes / No confirmation popup
    pub yes_no_popup: YesNoPopup,
    /// Remove channel popup
    pub remove_channel_popup: RemoveChannelPopup,
}

impl<'a> App<'a> {
    /// Constructs a new instance of [`App`].
    pub fn new(client: Client) -> Self {
        // There's likely a better way to populate these commands
        let mut commands_list = StatefulList::default();
        commands_list
            .items
            .push((Command::Image, Command::Image.to_str()));

        Self {
            user_id: None,
            input_mode: InputMode::Normal,
            current_screen: Screen::Main,
            ui_element: UiElement::None,
            current_pane: Pane::ChatPane,
            running: true,
            client,
            chat_history: Vec::new(),
            user_id_to_username: HashMap::new(),
            realms_manager: RealmsManager::default(),
            users_online: StatefulList::default(),
            realms: StatefulList::default(),
            text_channels: StatefulList::default(),
            voice_channels: StatefulList::default(),
            is_voice_connected: false,
            current_realm_id: None,
            current_text_channel: None,
            current_voice_channel: None,
            is_commanding: false,
            commands: Command::get_commands(),
            command_buffer: String::new(),
            command_list: commands_list,
            current_command: None,
            is_mentioning: false,
            mention_buffer: String::new(),
            mention_list: StatefulList::default(),
            users_mentioned: Vec::new(),
            input_buffer: InputBuffer::default(),
            is_popup_shown: false,
            popup_type: PopupType::General,
            popup_text: String::new(),
            popup_title: String::new(),
            general_popup: GeneralPopup::default(),
            add_channel_popup: AddChannelPopup::default(),
            member_popup: MemberPopup::default(),
            yes_no_popup: YesNoPopup::default(),
            remove_channel_popup: RemoveChannelPopup::default(),
        }
    }

    /// Handles the tick event of the terminal.
    pub fn tick(&self) {}

    /// Set running to false to quit the application.
    pub async fn quit(&mut self) {
        self.hang_up().await;
        self.client.disconnect().await;
        self.running = false;
    }

    pub async fn run_app(&mut self) -> AppResult<()> {
        // Initialize the terminal user interface.
        let backend = CrosstermBackend::new(io::stderr());
        let terminal = Terminal::new(backend)?;
        let events = EventHandler::new(250);
        let mut tui = Tui::new(terminal, events);
        tui.init()?;

        let user_id = self.client.get_user_id().await;
        match user_id {
            Some(id) => self.user_id = Some(id),
            None => {
                eprintln!("Failed to get User ID");
                self.quit().await;
            }
        };

        let username = self.client.get_username().await;
        match username {
            Some(_) => (),
            None => {
                eprintln!("Failed to get Username");
                self.quit().await;
            }
        };

        // We should be logged in and have our own User, so use this to show our name
        self.user_id_to_username
            .insert(self.user_id.unwrap(), username.unwrap());

        // Start the main loop.
        while self.running {
            // Update any new messages received by the Client
            for message in self.client.get_new_messages().await {
                match message.message {
                    MessageType::UserJoined(user) => {
                        // We should already know we're online, so ignore anything about us
                        if let Some(id) = self.client.get_user_id().await {
                            if id == user.get_id() {
                                continue;
                            }
                        }

                        // Add this user to a map to know who is who
                        self.user_id_to_username
                            .insert(user.get_id(), String::from(user.get_username()));

                        // Now add them to our list of users currently online
                        self.users_online
                            .items
                            .push((user.get_id(), String::from(user.get_username())));
                    }
                    MessageType::UserLeft(user_id) => {
                        //self.user_id_to_username.remove(&user_id);
                        let index = self
                            .users_online
                            .items
                            .iter()
                            .position(|x| x.0 == user_id)
                            .unwrap();
                        self.users_online.items.remove(index);
                    }
                    MessageType::UserJoinedVoiceChannel(join) => {
                        // Add this user to that channel's connected_users
                        self.realms_manager.add_user_to_voice_channel(
                            join.user_id,
                            join.realm_id,
                            join.channel_id,
                        );

                        // For now let's update the voice_channels list with
                        // what we already have saved elsewhere
                        for channel in &mut self.voice_channels.items {
                            if channel.0 == join.channel_id {
                                channel.2.push(join.user_id);
                            }
                        }

                        // If this is us, let us know we've been connected via voice
                        if let Some(id) = self.client.get_user_id().await {
                            if id == join.user_id {
                                self.is_voice_connected = true;
                                // Update our current voice channel ID
                                self.current_voice_channel = Some(join.channel_id);
                            }
                        }
                    }
                    MessageType::UserLeftVoiceChannel(left) => {
                        self.realms_manager.remove_user_from_voice_channel(
                            left.user_id,
                            left.realm_id,
                            left.channel_id,
                        );

                        // For now let's update the voice_channels list with
                        // what we already have saved elsewhere
                        for channel in &mut self.voice_channels.items {
                            if let Some(index) = channel.2.iter().position(|x| x == &left.user_id) {
                                channel.2.remove(index);
                            }
                        }
                    }
                    MessageType::AllUsers(users) => {
                        for user in users {
                            self.user_id_to_username
                                .insert(user.get_id(), String::from(user.get_username()));
                            self.users_online
                                .items
                                .push((user.get_id(), String::from(user.get_username())));
                        }
                    }
                    MessageType::Text(text) => {
                        self.chat_history.push(Line::from(vec![
                            Span::raw(self.get_username_from_id(message.user_id)),
                            Span::raw(": "),
                            Span::styled(
                                String::from_utf8(text).unwrap(),
                                Style::default().fg(Color::DarkGray),
                            ),
                        ]));
                    }
                    MessageType::TextMention(message) => {
                        // Add this message to its respective channel's history
                        // Get our realm
                        if let Some(realm) = self.realms_manager.get_realm_mut(message.0.realm_id) {
                            // Get this text channel
                            if let Some(channel) = realm.get_text_channel_mut(message.0.channel_id)
                            {
                                // Add this message to our that channel's chat history
                                channel.chat_history.push((
                                    message.0.user_id,
                                    None,
                                    message.1.clone(),
                                ));

                                for chunk in message.1 {
                                    if let Some(id) = chunk.1 {
                                        if id == self.user_id.unwrap() {
                                            // If we are currently in this channel, don't mark a pending mention
                                            if let Some(current_channel) =
                                                &self.current_text_channel
                                            {
                                                if current_channel.0 != *channel.get_id() {
                                                    channel.pending_mention = true;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    MessageType::Image(_message) => {
                        // For now, don't add the message we've received
                        // Better logic is needed on how to save and display an image

                        // Add this message to its respective channel's history
                        // Get our realm
                        // if let Some(realm) = self.realms_manager.get_realm_mut(message.0.realm_id) {
                        //     // Get this text channel
                        //     if let Some(channel) = realm.get_text_channel_mut(message.0.channel_id)
                        //     {
                        //         // Add this message to our that channel's chat history
                        //         channel.push_image(message.0.user_id, message.1);
                        //     }
                        // }
                    }
                    MessageType::Realms(realms) => {
                        //First clear our realms manager
                        self.realms_manager.clear();

                        // Clear our current list of realms and channels
                        self.realms = StatefulList::default();

                        for realm in &realms {
                            // Save realm id and names
                            self.realms.items.push((realm.id, realm.name.clone()));

                            // Save text channel id and names
                            self.text_channels = StatefulList::default();
                            let text_channels = realm.get_text_channels();
                            for (id, mut name) in text_channels {
                                let hashtag = String::from("# ");
                                name.insert_str(0, &hashtag);
                                self.text_channels.items.push((id, name.clone()));
                            }

                            // Save voice channel id and names
                            self.voice_channels = StatefulList::default();
                            let voice_channels = realm.get_voice_channels();
                            for (id, name) in voice_channels {
                                self.voice_channels.items.push((id, name, Vec::new()));
                            }

                            // Auto-join the first available text channel if one wasn't already joined
                            if self.current_text_channel.is_none()
                                && !self.text_channels.items.is_empty()
                            {
                                self.join_channel(
                                    realm.id,
                                    ChannelType::TextChannel,
                                    self.text_channels.items[0].0,
                                )
                                .await;
                            }
                        }
                    }
                    MessageType::RealmsManager(rm) => {
                        // First clear everything that we know
                        self.realms.items.clear();
                        self.text_channels.items.clear();
                        self.voice_channels.items.clear();

                        // Now move this new RealmsManager into our app
                        self.realms_manager = rm;

                        // Now that we have all realms and channels,
                        // let's update references to them to be displayed
                        for realm in self.realms_manager.get_realms() {
                            // Update our Realms list
                            self.realms.items.push((*realm.0, realm.1.clone()));
                        }

                        // For now, let's initally join the first text channel of the first realm
                        if !self.realms.items.is_empty() {
                            self.current_realm_id = Some(self.realms.items[0].0);
                            self.enter_realm(self.current_realm_id.unwrap()).await;
                        }
                    }
                    MessageType::ChannelAdded(ca) => {
                        // Add this new channel to the proper realm
                        self.realms_manager.add_channel_with_id(
                            ca.0,
                            ca.2,
                            ca.1.clone(),
                            ca.3.clone(),
                        );

                        // Refresh this realm if we're in it
                        // Otherwise the realm will be refreshed when it is joined again
                        if let Some(realm_id) = self.current_realm_id {
                            if realm_id == ca.0 {
                                self.refresh_realm(realm_id);
                            }
                        }
                    }
                    MessageType::ChannelRemoved(cr) => {
                        // Need to account for when we're in a voice channel and that channel is removed
                        // here

                        // Add this new channel to the proper realm
                        self.realms_manager.remove_channel(cr.0, cr.1, cr.2);

                        // Refresh this realm if we're in it
                        // Otherwise the realm will be refreshed when it is joined again
                        if let Some(realm_id) = self.current_realm_id {
                            if realm_id == cr.0 {
                                self.refresh_realm(realm_id);
                            }
                        }
                    }
                    _ => (),
                };
            }
            // Render the user interface.
            tui.draw(self)?;
            // Handle events.
            match tui.events.next()? {
                Event::Tick => self.tick(),
                Event::Key(key_event) => handle_key_events(key_event, self).await?,
                Event::Mouse(_) => {}
                Event::Resize(_, _) => {}
            }
        }

        // Exit the user interface.
        tui.exit()?;

        Ok(())
    }

    pub fn get_username_from_id(&self, user_id: UserIdSize) -> String {
        match self.user_id_to_username.get(&user_id) {
            Some(username) => username.to_string(),
            None => user_id.to_string(),
        }
    }

    // Join the client to a text or voice channel
    pub async fn join_channel(
        &mut self,
        realm_id: RealmIdSize,
        channel_type: ChannelType,
        channel_id: ChannelIdSize,
    ) {
        match channel_type {
            ChannelType::TextChannel => {
                self.client
                    .join_channel(realm_id, channel_type, channel_id)
                    .await;

                // Update our current text channel
                for channel in &self.text_channels.items {
                    if channel.0 == channel_id {
                        self.current_text_channel = Some((channel_id, channel.1.clone()));

                        // Now that we're in this text channel, unmark any pending mentions
                        if let Some(realm) = self.realms_manager.get_realm_mut(realm_id) {
                            if let Some(channel) = realm.get_text_channel_mut(channel_id) {
                                channel.pending_mention = false;
                            }
                        }

                        break;
                    }
                }
            }
            ChannelType::VoiceChannel => {
                self.client
                    .join_channel(realm_id, channel_type, channel_id)
                    .await;

                // Let the voices be heard
                self.connect_voice(realm_id, channel_id).await;
            }
        }
    }

    pub fn refresh_realm(&mut self, realm_id: RealmIdSize) {
        if let Some(realm) = self.realms_manager.get_realm(realm_id) {
            // Update our text channels list
            self.text_channels.items.clear();
            for text_channel in realm.get_text_channels() {
                self.text_channels.items.push((
                    *text_channel.0,
                    text_channel
                        .1
                        .get_name()
                        .to_string()
                        .add_hashtag_with_space(),
                ));
            }

            // Update our voice channels list
            self.voice_channels.items.clear();
            for voice_channel in realm.get_voice_channels() {
                self.voice_channels.items.push((
                    *voice_channel.0,
                    voice_channel.1.get_name().to_string(),
                    voice_channel.1.get_connected_users().clone(),
                ));
            }
        }
    }

    pub async fn enter_realm(&mut self, realm_id: RealmIdSize) {
        if let Some(realm) = self.realms_manager.get_realm(realm_id) {
            // Update our text channels list
            self.text_channels.items.clear();
            for text_channel in realm.get_text_channels() {
                self.text_channels.items.push((
                    *text_channel.0,
                    text_channel
                        .1
                        .get_name()
                        .to_string()
                        .add_hashtag_with_space(),
                ));
            }

            // Update our voice channels list
            self.voice_channels.items.clear();
            for voice_channel in realm.get_voice_channels() {
                self.voice_channels.items.push((
                    *voice_channel.0,
                    voice_channel.1.get_name().to_string(),
                    voice_channel.1.get_connected_users().clone(),
                ));
            }

            if !self.text_channels.items.is_empty() {
                self.join_channel(
                    realm_id,
                    ChannelType::TextChannel,
                    self.text_channels.items[0].0,
                )
                .await;
            }

            self.current_realm_id = Some(realm_id);
        }
    }

    pub async fn connect_voice(&mut self, realm_id: RealmIdSize, channel_id: ChannelIdSize) {
        self.client.connect_voice(realm_id, channel_id).await;
    }

    pub async fn hang_up(&mut self) {
        if let Some(channel) = self.current_voice_channel {
            self.client
                .hang_up(self.current_realm_id.as_ref().unwrap(), &channel)
                .await;

            self.is_voice_connected = false;
            self.current_voice_channel = None;
        }
    }

    pub async fn handle_input(&mut self) {
        // First check to see if this is a command message
        match self.current_command {
            Some(command) => match command {
                Command::Image => {
                    self.send_image().await;
                }
            },
            None => {
                self.client
                    .send_mention_message(
                        self.current_realm_id.unwrap(),
                        self.current_text_channel.as_ref().unwrap().0,
                        self.input_buffer.get_input_without_style(),
                    )
                    .await;
            }
        }

        self.current_command = None;
    }

    pub async fn send_image(&mut self) {
        // First check to see if the image exists
        if let Some(input) = self.input_buffer.input.last() {
            let path = input.0.as_str();
            if Path::new(path).exists() {
                // Check the size of the file. Don't send it if it's more than 10MB
                let metadata = std::fs::metadata(path).unwrap();
                if metadata.len() > 10000000 {
                    self.general_popup.setup(
                        Some(String::from("Image Error")),
                        Some(String::from("File size exceeds 10MB")),
                    );
                    self.show_popup(PopupType::General);
                } else {
                    let image = std::fs::read(path);
                    match image {
                        Ok(img) => {
                            self.client
                                .send_image(
                                    self.current_realm_id.unwrap(),
                                    self.current_text_channel.as_ref().unwrap().0,
                                    img,
                                )
                                .await;
                        }
                        Err(_) => {
                            self.general_popup.setup(
                                Some(String::from("Image Error")),
                                Some(String::from("Failed to load file")),
                            );
                            self.show_popup(PopupType::General);
                        }
                    }
                }
            } else {
                self.general_popup.setup(
                    Some(String::from("Image Error")),
                    Some(format!("{} does not exist", path)),
                );
                self.show_popup(PopupType::General);
            }
        }
    }

    pub fn show_popup(&mut self, popup_type: PopupType) {
        self.popup_type = popup_type;
        self.input_mode = InputMode::Popup;
        self.is_popup_shown = true;
    }

    pub fn dismiss_popup(&mut self) {
        self.is_popup_shown = false;
        self.input_mode = InputMode::Normal;
        self.current_pane = Pane::ChatPane;
        self.popup_type = PopupType::General;
        self.popup_title = String::new();
        self.popup_text = String::new();
    }

    pub fn show_yes_no_popup(&mut self, title: String, message: String) {
        self.yes_no_popup.setup(Some(title), Some(message));
        self.show_popup(PopupType::YesNo);
    }

    pub fn show_add_channel_popup(&mut self) {
        self.add_channel_popup.setup(None, None);
        self.show_popup(PopupType::AddChannel);
    }

    pub fn show_remove_channel_popup(
        &mut self,
        realm_id: RealmIdSize,
        channel_type: ChannelType,
        channel_id: ChannelIdSize,
    ) {
        self.remove_channel_popup.setup(None, None);
        self.remove_channel_popup.realm_id = realm_id;
        self.remove_channel_popup.channel_type = channel_type;
        self.remove_channel_popup.channel_id = channel_id;
        self.show_popup(PopupType::RemoveChannel)
    }

    pub fn show_member_popup(
        &mut self,
        user_id: UserIdSize,
        username: String,
        selected_index: usize,
    ) {
        self.member_popup.setup(None, None);
        self.member_popup.selected_index = selected_index;
        self.member_popup.user_id = user_id;
        self.member_popup.username = username;
        self.show_popup(PopupType::Member);
    }

    pub fn get_current_time_string(&self) -> String {
        let local_time = Local::now();
        local_time.format("%H:%M").to_string()
    }

    pub async fn add_channel(&mut self, channel_type: ChannelType, channel_name: String) {
        // Tell client to add the channel (send a AddChannel message)
        self.client
            .add_channel(self.current_realm_id.unwrap(), channel_type, channel_name)
            .await;
    }

    pub async fn remove_channel(&mut self, channel_type: ChannelType, channel_id: ChannelIdSize) {
        // Tell client to remove a channel (send a RemoveChannel message)
        self.client
            .remove_channel(self.current_realm_id.unwrap(), channel_type, channel_id)
            .await;
    }
}
