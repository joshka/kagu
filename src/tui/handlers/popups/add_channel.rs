use crate::{
    realms::realm::ChannelType,
    tui::{
        app::{App, AppResult, InputMode, Pane},
        popups::add_channel_popup::{AddChannelInputMode, AddChannelUiElement},
    },
};
use crossterm::event::{KeyCode, KeyEvent};

pub async fn handle_key_events(key_event: KeyEvent, app: &mut App<'_>) -> AppResult<()> {
    match app.add_channel_popup.input_mode {
        AddChannelInputMode::Normal => match key_event.code {
            KeyCode::Char('q') | KeyCode::Char('Q') => {
                app.dismiss_popup();
                app.input_mode = InputMode::ChannelType;
                app.current_pane = Pane::ChannelsPane;
            }
            KeyCode::Char(' ') => match app.add_channel_popup.current_ui_element {
                AddChannelUiElement::TextOption => {
                    if !app.add_channel_popup.is_text_channel {
                        app.add_channel_popup.is_text_channel = true;
                    }
                }
                AddChannelUiElement::VoiceOption => {
                    if app.add_channel_popup.is_text_channel {
                        app.add_channel_popup.is_text_channel = false;
                    }
                }
                _ => (),
            },
            KeyCode::Up => match app.add_channel_popup.current_ui_element {
                AddChannelUiElement::TextOption => (),
                AddChannelUiElement::VoiceOption => {
                    app.add_channel_popup.current_ui_element = AddChannelUiElement::TextOption;
                }
                AddChannelUiElement::ChannelName => {
                    app.add_channel_popup.current_ui_element = AddChannelUiElement::VoiceOption;
                }
            },
            KeyCode::Down => match app.add_channel_popup.current_ui_element {
                AddChannelUiElement::TextOption => {
                    app.add_channel_popup.current_ui_element = AddChannelUiElement::VoiceOption;
                }
                AddChannelUiElement::VoiceOption => {
                    app.add_channel_popup.current_ui_element = AddChannelUiElement::ChannelName;
                }
                AddChannelUiElement::ChannelName => (),
            },
            KeyCode::Enter => {
                if let AddChannelUiElement::ChannelName = app.add_channel_popup.current_ui_element {
                    app.add_channel_popup.input_mode = AddChannelInputMode::Editing;
                }
            }
            _ => (),
        },
        AddChannelInputMode::Editing => match key_event.code {
            KeyCode::Char(c) => app.add_channel_popup.channel_name_buffer.push(c),
            KeyCode::Esc => app.add_channel_popup.input_mode = AddChannelInputMode::Normal,
            KeyCode::Backspace => {
                app.add_channel_popup.channel_name_buffer.pop();
            }
            KeyCode::Enter => {
                app.add_channel(
                    match app.add_channel_popup.is_text_channel {
                        true => ChannelType::TextChannel,
                        false => ChannelType::VoiceChannel,
                    },
                    app.add_channel_popup.channel_name_buffer.clone(),
                )
                .await;

                app.dismiss_popup();
                app.text_channels.unselect();
                app.voice_channels.unselect();
                app.input_mode = InputMode::Normal;
                app.current_pane = Pane::ChannelsPane;
            }
            _ => (),
        },
    };

    Ok(())
}
