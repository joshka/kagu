use crate::tui::app::{App, KaguFormatting, Pane};
use tui::{
    layout::{Constraint, Direction, Layout},
    prelude::Alignment,
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

pub fn render(app: &mut App, frame: &mut Frame<'_>) {
    let top_and_bottom_layout = Layout::default()
        .direction(Direction::Vertical)
        .margin(0)
        .constraints([Constraint::Max(1), Constraint::Max(frame.size().width - 1)].as_ref())
        .split(frame.size());

    let kagu_bar = Layout::default()
        .direction(Direction::Horizontal)
        .margin(0)
        .constraints(
            [
                Constraint::Max(10),                                // Kagu logo
                Constraint::Max(20),                                // Voice status
                Constraint::Max(frame.size().width - 10 - 20 - 15), // Blank space
                Constraint::Max(15),                                // Current time
            ]
            .as_ref(),
        )
        .split(top_and_bottom_layout[0]);

    let kagu_logo = Paragraph::new("Kagu");
    let time = Paragraph::new(app.get_current_time_string()).alignment(Alignment::Right);
    let connected_label = Paragraph::new(match app.is_voice_connected {
        true => Span::styled("Voice connected", Style::default().fg(Color::LightGreen)),
        false => Span::styled("Voice off", Style::default()),
    });
    frame.render_widget(kagu_logo, kagu_bar[0]);
    frame.render_widget(connected_label, kagu_bar[1]);
    frame.render_widget(time, kagu_bar[3]);

    let back_panel = Layout::default()
        .direction(Direction::Horizontal)
        .margin(0)
        .constraints(
            [
                Constraint::Max(10),
                Constraint::Max(20),
                Constraint::Max(frame.size().width - 10 - 20),
            ]
            .as_ref(),
        )
        .split(top_and_bottom_layout[1]);

    let left_panel = Layout::default()
        .direction(Direction::Vertical)
        .margin(0)
        .constraints([Constraint::Max(3), Constraint::Max(frame.size().height - 4)].as_ref())
        .split(back_panel[0]);

    let realms_list: Vec<ListItem> = app
        .realms
        .items
        .iter()
        .map(|i| ListItem::new(i.1.clone()).style(Style::default().fg(Color::LightBlue)))
        .collect();
    let realms = List::new(realms_list)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(match app.current_pane {
                    Pane::RealmsPane => Pane::to_str(&app.current_pane).with_focus(),
                    _ => Pane::to_str(&Pane::RealmsPane),
                }),
        )
        .highlight_style(Style::default().add_modifier(Modifier::BOLD))
        .highlight_symbol(">");
    frame.render_stateful_widget(realms, left_panel[1], &mut app.realms.state);

    let middle_panel = Layout::default()
        .direction(Direction::Vertical)
        .margin(0)
        .constraints([Constraint::Max(3), Constraint::Max(frame.size().height - 3)].as_ref())
        .split(back_panel[1]);

    let right_panel = Layout::default()
        .direction(Direction::Vertical)
        .margin(0)
        .constraints([Constraint::Max(3), Constraint::Max(frame.size().height - 3)].as_ref())
        .split(back_panel[2]);

    let test_top_bar =
        Paragraph::new("Friends or friend_name").block(Block::default().borders(Borders::ALL));
    let test_dm_chat =
        Paragraph::new("DM history here").block(Block::default().borders(Borders::ALL));

    frame.render_widget(test_top_bar, right_panel[0]);
    frame.render_widget(test_dm_chat, right_panel[1]);

    let test_friends_button =
        Paragraph::new("Friends btn").block(Block::default().borders(Borders::ALL));
    let test_friends_list = Paragraph::new("DMs go here").block(
        Block::default()
            .title("Direct Messages")
            .borders(Borders::ALL),
    );

    frame.render_widget(test_friends_button, middle_panel[0]);
    frame.render_widget(test_friends_list, middle_panel[1]);
}
