use crate::state::AppStateContainer;
use ratatui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, List, ListItem, Paragraph},
    Frame,
};

/// Render the UI
pub fn render<B: Backend>(frame: &mut Frame, app: &AppStateContainer) {
    let size = frame.size();

    // Main layout
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3), // Status + VU meter
            Constraint::Min(10),   // History
            Constraint::Length(3), // Current text
            Constraint::Length(3), // Controls
        ])
        .split(size);

    render_status(frame, chunks[0], app);
    render_history(frame, chunks[1], app);
    render_current(frame, chunks[2], app);
    render_controls(frame, chunks[3]);
}

/// Render status line with VU meter
fn render_status<B: Backend>(frame: &mut Frame, area: Rect, app: &AppStateContainer) {
    let state_color = app.state.color();
    let state_text = app.state.display_text();

    // Split into status text and VU meter
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    // Status text
    let status_text = if let Some(ref error) = app.error_message {
        format!("Status: {} | Error: {}", state_text, error)
    } else {
        format!("Status: {}", state_text)
    };

    let status = Paragraph::new(status_text)
        .style(Style::default().fg(state_color))
        .block(Block::default().borders(Borders::ALL).title("TheHand"));

    frame.render_widget(status, chunks[0]);

    // VU meter
    let audio_percent = (app.audio_level * 100.0) as u16;
    let vu_meter = Gauge::default()
        .block(Block::default().borders(Borders::ALL))
        .gauge_style(
            Style::default()
                .fg(Color::Green)
                .bg(Color::Black)
                .add_modifier(Modifier::BOLD),
        )
        .percent(audio_percent);

    frame.render_widget(vu_meter, chunks[1]);
}

/// Render transcription history
fn render_history<B: Backend>(frame: &mut Frame, area: Rect, app: &AppStateContainer) {
    let items: Vec<ListItem> = app
        .history
        .iter()
        .map(|entry| {
            let time = entry.format_time();
            let content = format!("[{}] {}", time, entry.text);
            ListItem::new(content)
        })
        .collect();

    let history_list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("History"))
        .style(Style::default().fg(Color::White));

    frame.render_widget(history_list, area);
}

/// Render current text being processed
fn render_current<B: Backend>(frame: &mut Frame, area: Rect, app: &AppStateContainer) {
    let current_text = if app.current_text.is_empty() {
        "_".to_string()
    } else {
        app.current_text.clone()
    };

    let current = Paragraph::new(current_text)
        .style(Style::default().fg(Color::Cyan))
        .block(Block::default().borders(Borders::ALL).title("Current"));

    frame.render_widget(current, area);
}

/// Render control hints
fn render_controls<B: Backend>(frame: &mut Frame, area: Rect) {
    let controls = vec![
        Span::styled("[M]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw("ute  "),
        Span::styled("[C]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw("ancel  "),
        Span::styled("[Q]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw("uit"),
    ];

    let controls_line = Line::from(controls);
    let controls_widget = Paragraph::new(controls_line)
        .block(Block::default().borders(Borders::ALL));

    frame.render_widget(controls_widget, area);
}
