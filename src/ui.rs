use crate::state::{AppState, AppStateContainer};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, List, ListItem, Paragraph},
    Frame,
};

/// Render the UI
pub fn render(frame: &mut Frame, app: &AppStateContainer) {
    let size = frame.size();

    // Check if we're in device selection mode
    if app.state == AppState::DeviceSelection {
        render_device_selection(frame, size, app);
        return;
    }

    // Main layout
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3), // Status + VU meter
            Constraint::Min(10),   // History
            Constraint::Length(3), // Controls
        ])
        .split(size);

    render_status(frame, chunks[0], app);
    render_history(frame, chunks[1], app);
    render_controls(frame, chunks[2]);
}

/// Render status line with VU meter
fn render_status(frame: &mut Frame, area: Rect, app: &AppStateContainer) {
    let state_color = app.state.color();
    let state_text = app.state.display_text();

    // Split into status text and VU meter
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    // Status text
    let status_text = if let Some(ref error) = app.error_message {
        format!("Status: {} | Device: {} | Error: {}", state_text, app.current_device_name, error)
    } else {
        format!("Status: {} | Device: {}", state_text, app.current_device_name)
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
fn render_history(frame: &mut Frame, area: Rect, app: &AppStateContainer) {
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

/// Render control hints
fn render_controls(frame: &mut Frame, area: Rect) {
    let controls = vec![
        Span::styled("[F1]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw("Mute  "),
        Span::styled("[F2]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw("Cancel  "),
        Span::styled("[F3]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw("Device  "),
        Span::styled("[F4]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw("Reload  "),
        Span::styled("[F12]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw("Quit"),
    ];

    let controls_line = Line::from(controls);
    let controls_widget = Paragraph::new(controls_line)
        .block(Block::default().borders(Borders::ALL));

    frame.render_widget(controls_widget, area);
}

/// Render device selection screen
fn render_device_selection(frame: &mut Frame, area: Rect, app: &AppStateContainer) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3), // Title
            Constraint::Min(10),   // Device list
            Constraint::Length(3), // Controls
        ])
        .split(area);

    // Title
    let title_text = format!(
        "Select Audio Input Device ({}/{})",
        app.selected_device_index + 1,
        app.available_devices.len()
    );
    let title = Paragraph::new(title_text)
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .block(Block::default().borders(Borders::ALL).title("TheHand"));
    frame.render_widget(title, chunks[0]);

    // Device list with audio level bars
    let items: Vec<ListItem> = app
        .available_devices
        .iter()
        .enumerate()
        .map(|(i, device)| {
            let marker = if i == app.selected_device_index {
                "▶ "
            } else {
                "  "
            };
            let current_marker = if Some(device.index) == app.current_device_index {
                " [CURRENT]"
            } else {
                ""
            };

            // Show audio level bar for selected device
            let level_bar = if i == app.selected_device_index {
                let bar_width = 20;
                let filled = (app.preview_level * bar_width as f32) as usize;
                let filled = filled.min(bar_width);
                let empty = bar_width - filled;
                format!(" [{}{}]", "█".repeat(filled), "░".repeat(empty))
            } else {
                String::new()
            };

            let content = format!("{}{}{}{}", marker, device.name, current_marker, level_bar);
            let style = if i == app.selected_device_index {
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            ListItem::new(content).style(style)
        })
        .collect();

    let device_list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Available Devices"))
        .style(Style::default().fg(Color::White));
    frame.render_widget(device_list, chunks[1]);

    // Controls
    let controls = vec![
        Span::styled("[↑/↓]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw(" Navigate (tap mic to see level)  "),
        Span::styled("[Enter]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw(" Select  "),
        Span::styled("[F3/Esc]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw(" Cancel"),
    ];
    let controls_line = Line::from(controls);
    let controls_widget = Paragraph::new(controls_line)
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(controls_widget, chunks[2]);
}
