//! Ratatui widget layout for the LCD dashboard

extern crate alloc;

use alloc::format;
use alloc::vec;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::messages::DisplayState;

/// Render the system dashboard onto a ratatui frame.
///
/// Layout (3 rows):
/// - Header: system name + uptime
/// - Body: two columns — control status | maintenance status
/// - Footer: button state indicator
pub fn render_dashboard(frame: &mut Frame<'_>, state: &DisplayState) {
    let area = frame.area();

    // 3-row vertical layout: header (3), body (fill), footer (3)
    let rows = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(1),
        Constraint::Length(3),
    ])
    .split(area);

    render_header(frame, rows[0], state);
    render_body(frame, rows[1], state);
    render_footer(frame, rows[2], state);
}

fn render_header(frame: &mut Frame<'_>, area: Rect, state: &DisplayState) {
    let uptime_m = state.uptime_secs / 60;
    let uptime_s = state.uptime_secs % 60;

    let header = Paragraph::new(Line::from(vec![
        Span::styled("PicoTemplate", Style::default().fg(Color::Cyan)),
        Span::raw("  "),
        Span::styled(
            format!("up {uptime_m}m{uptime_s:02}s"),
            Style::default().fg(Color::DarkGray),
        ),
    ]))
    .block(Block::default().borders(Borders::BOTTOM));

    frame.render_widget(header, area);
}

fn render_body(frame: &mut Frame<'_>, area: Rect, state: &DisplayState) {
    // Two-column horizontal layout
    let cols =
        Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]).split(area);

    // Left column: control status
    let ctrl_text = format!("cycle: {}", state.control_cycle);
    let ctrl =
        Paragraph::new(ctrl_text).block(Block::default().title("Control").borders(Borders::ALL));
    frame.render_widget(ctrl, cols[0]);

    // Right column: maintenance status
    let led_indicator = if state.led_state { "ON" } else { "OFF" };
    let maint_ok_indicator = if state.maintenance_ok { "OK" } else { "ERR" };
    let maint_text = format!(
        "status: {}\nLED: {}\ntick: {}",
        maint_ok_indicator, led_indicator, state.maintenance_tick
    );
    let maint = Paragraph::new(maint_text)
        .block(Block::default().title("Maintenance").borders(Borders::ALL));
    frame.render_widget(maint, cols[1]);
}

fn render_footer(frame: &mut Frame<'_>, area: Rect, state: &DisplayState) {
    let btn_style = if state.button_pressed {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let btn_label = if state.button_pressed {
        "[PRESSED]"
    } else {
        "[ idle ]"
    };

    let footer = Paragraph::new(Line::from(vec![
        Span::raw("Button: "),
        Span::styled(btn_label, btn_style),
    ]))
    .block(Block::default().borders(Borders::TOP));

    frame.render_widget(footer, area);
}
