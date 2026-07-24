//! Ratatui widgets and layout for the Unified Workspace.

mod theme;

pub use theme::palette;

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::state::{
    CommandPaletteState, ComposerMode, NotificationKind, WorkspaceApp, WorkspaceFocus,
    WorkspaceModal,
};
use crate::conversation::MessageContent;
use crate::intent::WorkspaceRoute;

/// Draw the full Workspace frame from application state.
pub fn draw(frame: &mut Frame, app: &WorkspaceApp) {
    let size = frame.area();
    let layout = responsive_layout(size);

    draw_header(frame, app, layout.header);
    draw_body(frame, app, &layout);
    draw_composer(frame, app, layout.composer);
    draw_status(frame, app, layout.status);

    if app.palette.open {
        draw_palette(frame, app, size);
    }
    if let Some(modal) = &app.modal {
        draw_modal(frame, modal, size);
    }
}

struct PaneLayout {
    header: Rect,
    left: Option<Rect>,
    center: Rect,
    right: Option<Rect>,
    composer: Rect,
    status: Rect,
}

fn responsive_layout(area: Rect) -> PaneLayout {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .split(area);

    let header = chunks[0];
    let body = chunks[1];
    let composer = chunks[2];
    let status = chunks[3];

    let width = area.width;
    if width >= 120 {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(22),
                Constraint::Percentage(50),
                Constraint::Percentage(28),
            ])
            .split(body);
        PaneLayout {
            header,
            left: Some(cols[0]),
            center: cols[1],
            right: Some(cols[2]),
            composer,
            status,
        }
    } else if width >= 80 {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
            .split(body);
        PaneLayout {
            header,
            left: None,
            center: cols[0],
            right: Some(cols[1]),
            composer,
            status,
        }
    } else {
        PaneLayout {
            header,
            left: None,
            center: body,
            right: None,
            composer,
            status,
        }
    }
}

fn draw_header(frame: &mut Frame, app: &WorkspaceApp, area: Rect) {
    let runtime = if app.runtime_healthy { "●" } else { "○" };
    let inv = app
        .active_investigation
        .as_ref()
        .map(|i| format!("INV {}", short_id(&i.id.to_string())))
        .unwrap_or_else(|| "no investigation".into());
    let title = format!(
        " Rivora  ·  {}  ·  Runtime {runtime}  ·  {}  ·  Ctrl+P ",
        app.route_label(),
        inv
    );
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(palette().border))
        .title(Span::styled(
            title,
            Style::default()
                .fg(palette().accent)
                .add_modifier(Modifier::BOLD),
        ));
    frame.render_widget(block, area);
}

fn draw_body(frame: &mut Frame, app: &WorkspaceApp, layout: &PaneLayout) {
    if let Some(left) = layout.left {
        draw_context(frame, app, left);
    }
    draw_primary(frame, app, layout.center);
    if let Some(right) = layout.right {
        if app.inspector_visible {
            draw_inspector(frame, app, right);
        } else {
            draw_context(frame, app, right);
        }
    }
}

fn draw_context(frame: &mut Frame, app: &WorkspaceApp, area: Rect) {
    let focused = app.focus == WorkspaceFocus::Context;
    let mut items: Vec<ListItem> = Vec::new();
    items.push(ListItem::new(Line::from(Span::styled(
        "Recent Investigations",
        Style::default().add_modifier(Modifier::BOLD),
    ))));
    if app.recent_investigations.is_empty() {
        items.push(ListItem::new("  (none yet)"));
    } else {
        for (idx, inv) in app.recent_investigations.iter().enumerate() {
            let selected = app.context_selected == idx;
            let mark = if selected { "›" } else { " " };
            let line = format!("{mark} {} [{}]", truncate(&inv.title, 28), inv.status);
            items.push(ListItem::new(line));
        }
    }
    items.push(ListItem::new(""));
    items.push(ListItem::new(Line::from(Span::styled(
        "Suggested",
        Style::default().add_modifier(Modifier::BOLD),
    ))));
    for s in &[
        "Investigate today's failed deployment",
        "Explain the latest CI failure",
        "Show Kubernetes investigations",
        "Check connector health",
    ] {
        items.push(ListItem::new(format!("  · {s}")));
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Context ")
        .border_style(focus_border(focused));
    let list = List::new(items).block(block);
    frame.render_widget(list, area);
}

fn draw_primary(frame: &mut Frame, app: &WorkspaceApp, area: Rect) {
    let focused = app.focus == WorkspaceFocus::Conversation;
    let title = match app.route {
        WorkspaceRoute::Home => " Conversation ",
        WorkspaceRoute::Investigation => " Investigation ",
        WorkspaceRoute::Search => " Search ",
        WorkspaceRoute::ProposalReview => " Proposal Review ",
        WorkspaceRoute::ExecutionReview => " Execution Review ",
        WorkspaceRoute::Connectors => " Connectors ",
        WorkspaceRoute::Doctor => " Doctor ",
        WorkspaceRoute::Learning => " Learning ",
        WorkspaceRoute::Settings => " Settings ",
        WorkspaceRoute::Help => " Help ",
    };

    let mut lines: Vec<Line> = Vec::new();
    if app.conversation.messages.is_empty() {
        lines.extend(onboarding_lines());
    } else {
        for msg in &app.conversation.messages {
            for l in msg.lines() {
                let style = match msg.content {
                    MessageContent::Error { .. } => Style::default().fg(palette().error),
                    MessageContent::Warning { .. } => Style::default().fg(palette().warning),
                    MessageContent::Confirmation { .. } => Style::default().fg(palette().accent),
                    _ => Style::default().fg(palette().primary),
                };
                lines.push(Line::from(Span::styled(l, style)));
            }
            lines.push(Line::from(""));
        }
    }

    // Panel overlay content for non-home routes with panel_lines.
    if !app.panel_lines.is_empty()
        && !matches!(
            app.route,
            WorkspaceRoute::Home | WorkspaceRoute::Investigation
        )
    {
        lines.push(Line::from(Span::styled(
            format!("── {} ──", app.panel_title),
            Style::default().add_modifier(Modifier::BOLD),
        )));
        for l in &app.panel_lines {
            lines.push(Line::from(l.as_str()));
        }
    }

    if !app.list_items.is_empty() {
        lines.push(Line::from(Span::styled(
            "Results",
            Style::default().add_modifier(Modifier::BOLD),
        )));
        for (idx, item) in app.list_items.iter().enumerate() {
            let mark = if app.list_selected == idx { "›" } else { " " };
            lines.push(Line::from(format!(
                "{mark} {} [{}] {}",
                truncate(&item.title, 40),
                item.status,
                short_id(&item.id.to_string())
            )));
        }
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(focus_border(focused));
    let para = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((app.conversation.scroll, 0));
    frame.render_widget(para, area);
}

fn draw_inspector(frame: &mut Frame, app: &WorkspaceApp, area: Rect) {
    let focused = app.focus == WorkspaceFocus::Inspector;
    let mut text = app.inspector_text.clone();
    if text.is_empty() {
        text = "Select an object or investigation\nto inspect identity, provenance,\nevidence, and available actions."
            .into();
    }
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Inspector ")
        .border_style(focus_border(focused));
    let para = Paragraph::new(text).block(block).wrap(Wrap { trim: false });
    frame.render_widget(para, area);
}

fn draw_composer(frame: &mut Frame, app: &WorkspaceApp, area: Rect) {
    let focused = app.focus == WorkspaceFocus::Composer;
    let mode_label = match app.composer.mode {
        ComposerMode::Prompt => "Ask Rivora…",
        ComposerMode::Slash => "Action /",
        ComposerMode::Confirm => "Confirm",
        ComposerMode::Busy => "Working…",
    };
    let placeholder =
        if app.composer.input.is_empty() && !matches!(app.composer.mode, ComposerMode::Busy) {
            match app.composer.mode {
                ComposerMode::Prompt => " Ask Rivora…  ( / actions · Ctrl+P commands ) ",
                ComposerMode::Slash => " Filter actions… ",
                ComposerMode::Confirm => " y confirm · n cancel ",
                ComposerMode::Busy => " Please wait… ",
            }
            .to_string()
        } else {
            format!(" {} ", app.composer.display_with_cursor())
        };
    let style = if focused {
        Style::default()
            .fg(palette().focused)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(palette().muted)
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" {mode_label} "))
        .border_style(focus_border(focused));
    let para = Paragraph::new(placeholder).style(style).block(block);
    frame.render_widget(para, area);
}

fn draw_status(frame: &mut Frame, app: &WorkspaceApp, area: Rect) {
    let notif = app
        .notifications
        .last()
        .map(|n| {
            let icon = match n.kind {
                NotificationKind::Success => "✓",
                NotificationKind::Info => "i",
                NotificationKind::Warning => "!",
                NotificationKind::Error => "✗",
                NotificationKind::Progress => "…",
            };
            format!("{icon} {}", n.text)
        })
        .unwrap_or_default();
    let busy = if app.tasks.is_busy() { " busy" } else { "" };
    let line = format!(
        " Tab focus · / actions · Ctrl+P · ? help · Esc close · Ctrl+C quit{busy}  {notif}"
    );
    frame.render_widget(
        Paragraph::new(line).style(Style::default().fg(palette().muted)),
        area,
    );
}

fn draw_palette(frame: &mut Frame, app: &WorkspaceApp, area: Rect) {
    let popup = centered_rect(60, 60, area);
    frame.render_widget(Clear, popup);
    let title = if app.palette.global {
        " Command Palette (Ctrl+P) "
    } else {
        " Actions (/) "
    };
    let items: Vec<ListItem> = if app.palette.filtered.is_empty() {
        vec![ListItem::new("No matching actions")]
    } else {
        app.palette
            .filtered
            .iter()
            .enumerate()
            .map(|(idx, (desc, avail))| {
                let mark = if idx == app.palette.selected {
                    "›"
                } else {
                    " "
                };
                let disabled = if avail.is_available() {
                    String::new()
                } else {
                    format!(" — {}", avail_reason(avail))
                };
                let line = format!("{mark} {}  {}{}", desc.label, desc.description, disabled);
                let style = if avail.is_available() {
                    Style::default()
                } else {
                    Style::default().fg(palette().disabled)
                };
                ListItem::new(Line::from(Span::styled(line, style)))
            })
            .collect()
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(Style::default().fg(palette().accent));
    let filter = format!("Filter: {}", app.palette.filter);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(3)])
        .split(popup);
    frame.render_widget(Paragraph::new(filter).block(block.clone()), chunks[0]);
    frame.render_widget(List::new(items).block(block), chunks[1]);
}

fn draw_modal(frame: &mut Frame, modal: &WorkspaceModal, area: Rect) {
    let popup = centered_rect(50, 40, area);
    frame.render_widget(Clear, popup);
    let (title, body) = match modal {
        WorkspaceModal::Confirm { title, body, .. } => (title.as_str(), body.as_str()),
        WorkspaceModal::Help => (
            "Help",
            "Type naturally · / actions · Ctrl+P palette · Esc close · Ctrl+C quit",
        ),
        WorkspaceModal::Error { title, body } => (title.as_str(), body.as_str()),
    };
    let text = format!("{title}\n\n{body}\n\n[Enter/y] confirm  [Esc/n] cancel");
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Confirm ")
        .border_style(Style::default().fg(palette().warning));
    frame.render_widget(
        Paragraph::new(text).block(block).wrap(Wrap { trim: false }),
        popup,
    );
}

fn onboarding_lines() -> Vec<Line<'static>> {
    vec![
        Line::from(Span::styled(
            "Welcome to Rivora",
            Style::default()
                .fg(palette().accent)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("Ask a question about your engineering system."),
        Line::from(""),
        Line::from("Try:"),
        Line::from("  · Investigate today's failed deployment"),
        Line::from("  · Explain the latest CI failure"),
        Line::from("  · Show Kubernetes-related investigations"),
        Line::from("  · Check connector health"),
        Line::from(""),
        Line::from("Press / to browse actions"),
        Line::from("Conversation is the interface — Runtime keeps authority."),
    ]
}

fn focus_border(focused: bool) -> Style {
    if focused {
        Style::default().fg(palette().focused)
    } else {
        Style::default().fg(palette().border)
    }
}

fn avail_reason(a: &crate::actions::ActionAvailability) -> String {
    match a {
        crate::actions::ActionAvailability::Available => String::new(),
        crate::actions::ActionAvailability::Disabled { reason } => reason.clone(),
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let t: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{t}…")
    }
}

fn short_id(id: &str) -> String {
    id.chars().take(8).collect()
}

/// Refresh palette filtered list from registry.
pub fn refresh_palette(
    palette: &mut CommandPaletteState,
    active_investigation: Option<rivora::domain::InvestigationId>,
    has_selected_proposal: bool,
    has_selected_plan: bool,
) {
    use crate::actions::{filter_actions, ActionContext};
    let filter = palette.filter.clone();
    let ctx = ActionContext {
        active_investigation,
        has_selected_proposal,
        has_selected_plan,
        filter: "",
    };
    palette.filtered = filter_actions(&filter, ctx);
    if palette.selected >= palette.filtered.len() {
        palette.selected = palette.filtered.len().saturating_sub(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn large_layout_has_three_panes() {
        let l = responsive_layout(Rect::new(0, 0, 160, 40));
        assert!(l.left.is_some());
        assert!(l.right.is_some());
    }

    #[test]
    fn small_layout_is_single_pane() {
        let l = responsive_layout(Rect::new(0, 0, 60, 24));
        assert!(l.left.is_none());
        assert!(l.right.is_none());
    }

    #[test]
    fn medium_layout_two_regions() {
        let l = responsive_layout(Rect::new(0, 0, 90, 30));
        assert!(l.left.is_none());
        assert!(l.right.is_some());
    }
}
