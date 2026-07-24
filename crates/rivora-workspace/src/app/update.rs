//! Input handling and application updates.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::actions::ActionContext;
use crate::conversation::WorkspaceMessage;
use crate::intent::execute::IntentExecutionResult;
use crate::intent::{
    execute_intent, interpret_prompt, InvestigationDraft, WorkspaceIntent, WorkspaceRoute,
};
use crate::ui::refresh_palette;

use super::state::{ComposerMode, NotificationKind, WorkspaceApp, WorkspaceFocus, WorkspaceModal};

fn refresh_app_palette(app: &mut WorkspaceApp) {
    let active = app.active_id();
    let prop = app.selected_proposal_id.is_some();
    let plan = app.selected_plan_id.is_some();
    refresh_palette(&mut app.palette, active, prop, plan);
}

/// Handle a key press. Returns Ok when processed.
pub fn handle_key(app: &mut WorkspaceApp, key: KeyEvent) -> Result<(), String> {
    // Global quit
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        if app.tasks.is_busy() {
            app.tasks.cancel_active();
            app.notify(NotificationKind::Warning, "Cancelled background task");
            app.composer.mode = ComposerMode::Prompt;
            return Ok(());
        }
        app.should_quit = true;
        return Ok(());
    }

    // Global palette
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('p') {
        open_palette(app, true);
        return Ok(());
    }

    // Modal takes priority
    if app.modal.is_some() {
        return handle_modal_key(app, key);
    }

    // Palette takes priority over composer
    if app.palette.open {
        return handle_palette_key(app, key);
    }

    // Pending confirmation via composer mode
    if matches!(app.composer.mode, ComposerMode::Confirm) || app.pending_intent.is_some() {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                confirm_pending(app);
                return Ok(());
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                cancel_pending(app);
                return Ok(());
            }
            _ => {}
        }
    }

    match key.code {
        KeyCode::Esc => {
            if app.palette.open {
                app.palette.open = false;
            } else if app.modal.is_some() {
                app.modal = None;
            } else if !matches!(app.route, WorkspaceRoute::Home) {
                app.route = WorkspaceRoute::Home;
                app.focus = WorkspaceFocus::Composer;
            }
            return Ok(());
        }
        KeyCode::Char('?')
            if app.focus != WorkspaceFocus::Composer || app.composer.input.is_empty() =>
        {
            app.modal = Some(WorkspaceModal::Help);
            return Ok(());
        }
        KeyCode::Tab => {
            cycle_focus(app, true);
            return Ok(());
        }
        KeyCode::BackTab => {
            cycle_focus(app, false);
            return Ok(());
        }
        _ => {}
    }

    match app.focus {
        WorkspaceFocus::Composer => handle_composer_key(app, key),
        WorkspaceFocus::Context => handle_context_key(app, key),
        WorkspaceFocus::List | WorkspaceFocus::Conversation => handle_list_key(app, key),
        WorkspaceFocus::Inspector => handle_inspector_key(app, key),
    }
}

fn handle_modal_key(app: &mut WorkspaceApp, key: KeyEvent) -> Result<(), String> {
    match key.code {
        KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => {
            if let Some(WorkspaceModal::Confirm { .. }) = &app.modal {
                cancel_pending(app);
            }
            app.modal = None;
        }
        KeyCode::Enter | KeyCode::Char('y') | KeyCode::Char('Y') => {
            if let Some(WorkspaceModal::Confirm { pending, .. }) = app.modal.take() {
                app.pending_intent = None;
                dispatch_intent(app, pending);
            } else {
                app.modal = None;
            }
        }
        _ => {}
    }
    Ok(())
}

fn handle_palette_key(app: &mut WorkspaceApp, key: KeyEvent) -> Result<(), String> {
    match key.code {
        KeyCode::Esc => {
            app.palette.open = false;
            app.palette.filter.clear();
            if app.composer.input.starts_with('/') {
                app.composer.clear();
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.palette.selected = app.palette.selected.saturating_sub(1);
        }
        KeyCode::Down | KeyCode::Char('j')
            if app.palette.selected + 1 < app.palette.filtered.len() =>
        {
            app.palette.selected += 1;
        }
        KeyCode::Enter => {
            activate_palette_selection(app);
        }
        KeyCode::Backspace => {
            app.palette.filter.pop();
            if let Some(c) = app.composer.input.pop() {
                let _ = c;
                app.composer.cursor = app.composer.input.chars().count();
            }
            refresh_app_palette(app);
        }
        KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.palette.filter.push(ch);
            if app.composer.mode == ComposerMode::Slash || app.composer.input.starts_with('/') {
                app.composer.insert(ch);
            }
            refresh_app_palette(app);
        }
        _ => {}
    }
    Ok(())
}

fn handle_composer_key(app: &mut WorkspaceApp, key: KeyEvent) -> Result<(), String> {
    if matches!(app.composer.mode, ComposerMode::Busy) {
        return Ok(());
    }

    match key.code {
        KeyCode::Char('/') if app.composer.input.is_empty() => {
            open_palette(app, false);
            app.composer.insert('/');
            app.composer.mode = ComposerMode::Slash;
        }
        KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.composer.insert(ch);
            if app.composer.mode == ComposerMode::Slash {
                // Keep palette filter in sync when typing slash command.
                if app.palette.open {
                    app.palette.filter = app.composer.input.trim_start_matches('/').to_string();
                    refresh_app_palette(app);
                } else {
                    open_palette(app, false);
                    app.palette.filter = app.composer.input.trim_start_matches('/').to_string();
                    refresh_app_palette(app);
                }
            }
        }
        KeyCode::Backspace => {
            app.composer.backspace();
            if app.palette.open && app.composer.mode == ComposerMode::Slash {
                app.palette.filter = app.composer.input.trim_start_matches('/').to_string();
                refresh_app_palette(app);
            }
        }
        KeyCode::Left => app.composer.move_left(),
        KeyCode::Right => app.composer.move_right(),
        KeyCode::Up => {
            if app.composer.history.is_empty() {
                return Ok(());
            }
            let idx = match app.composer.history_index {
                Some(i) => i.saturating_sub(1),
                None => app.composer.history.len().saturating_sub(1),
            };
            app.composer.history_index = Some(idx);
            if let Some(item) = app.composer.history.get(idx) {
                app.composer.input = item.clone();
                app.composer.cursor = app.composer.input.chars().count();
            }
        }
        KeyCode::Down => {
            if let Some(idx) = app.composer.history_index {
                if idx + 1 < app.composer.history.len() {
                    app.composer.history_index = Some(idx + 1);
                    if let Some(item) = app.composer.history.get(idx + 1) {
                        app.composer.input = item.clone();
                        app.composer.cursor = app.composer.input.chars().count();
                    }
                } else {
                    app.composer.history_index = None;
                    app.composer.clear();
                }
            }
        }
        KeyCode::Enter => {
            if app.palette.open {
                activate_palette_selection(app);
            } else {
                submit_composer(app);
            }
        }
        KeyCode::PageUp => {
            app.conversation.scroll = app.conversation.scroll.saturating_add(5);
        }
        KeyCode::PageDown => {
            app.conversation.scroll = app.conversation.scroll.saturating_sub(5);
        }
        _ => {}
    }
    Ok(())
}

fn handle_context_key(app: &mut WorkspaceApp, key: KeyEvent) -> Result<(), String> {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            app.context_selected = app.context_selected.saturating_sub(1);
        }
        KeyCode::Down | KeyCode::Char('j')
            if app.context_selected + 1 < app.recent_investigations.len() =>
        {
            app.context_selected += 1;
        }
        KeyCode::Enter => {
            if let Some(item) = app.recent_investigations.get(app.context_selected).cloned() {
                dispatch_intent(
                    app,
                    WorkspaceIntent::OpenInvestigation {
                        investigation_id: item.id,
                    },
                );
            }
        }
        KeyCode::Char('/') => {
            app.focus = WorkspaceFocus::Composer;
            open_palette(app, false);
        }
        KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.focus = WorkspaceFocus::Composer;
            app.composer.insert(ch);
        }
        _ => {}
    }
    Ok(())
}

fn handle_list_key(app: &mut WorkspaceApp, key: KeyEvent) -> Result<(), String> {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            app.list_selected = app.list_selected.saturating_sub(1);
            app.conversation.scroll = app.conversation.scroll.saturating_add(1);
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.list_selected + 1 < app.list_items.len() {
                app.list_selected += 1;
            }
            app.conversation.scroll = app.conversation.scroll.saturating_sub(1);
        }
        KeyCode::Enter => {
            if let Some(item) = app.list_items.get(app.list_selected).cloned() {
                dispatch_intent(
                    app,
                    WorkspaceIntent::OpenInvestigation {
                        investigation_id: item.id,
                    },
                );
            }
        }
        KeyCode::PageUp => {
            app.conversation.scroll = app.conversation.scroll.saturating_add(10);
        }
        KeyCode::PageDown => {
            app.conversation.scroll = app.conversation.scroll.saturating_sub(10);
        }
        KeyCode::Char('/') => {
            app.focus = WorkspaceFocus::Composer;
            open_palette(app, false);
        }
        KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.focus = WorkspaceFocus::Composer;
            app.composer.insert(ch);
        }
        _ => {}
    }
    Ok(())
}

fn handle_inspector_key(app: &mut WorkspaceApp, key: KeyEvent) -> Result<(), String> {
    match key.code {
        KeyCode::Char('i') => {
            app.inspector_visible = !app.inspector_visible;
        }
        KeyCode::Char('/') => {
            app.focus = WorkspaceFocus::Composer;
            open_palette(app, false);
        }
        KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.focus = WorkspaceFocus::Composer;
            app.composer.insert(ch);
        }
        _ => {}
    }
    Ok(())
}

fn cycle_focus(app: &mut WorkspaceApp, forward: bool) {
    let order = [
        WorkspaceFocus::Composer,
        WorkspaceFocus::Conversation,
        WorkspaceFocus::Context,
        WorkspaceFocus::Inspector,
    ];
    let idx = order.iter().position(|f| *f == app.focus).unwrap_or(0);
    let next = if forward {
        (idx + 1) % order.len()
    } else {
        (idx + order.len() - 1) % order.len()
    };
    app.focus = order[next];
}

fn open_palette(app: &mut WorkspaceApp, global: bool) {
    app.palette.open = true;
    app.palette.global = global;
    app.palette.filter.clear();
    app.palette.selected = 0;
    refresh_app_palette(app);
}

fn activate_palette_selection(app: &mut WorkspaceApp) {
    let selection = app
        .palette
        .filtered
        .get(app.palette.selected)
        .map(|(d, a)| (d.clone(), a.clone()));
    app.palette.open = false;
    app.composer.clear();
    let Some((desc, avail)) = selection else {
        return;
    };
    if !avail.is_available() {
        app.notify(
            NotificationKind::Warning,
            match avail {
                crate::actions::ActionAvailability::Disabled { reason } => reason,
                _ => "Action unavailable".into(),
            },
        );
        return;
    }
    let ctx = ActionContext {
        active_investigation: app.active_id(),
        has_selected_proposal: app.selected_proposal_id.is_some(),
        has_selected_plan: app.selected_plan_id.is_some(),
        filter: "",
    };
    if let Some(intent) = (desc.intent_builder)(ctx) {
        // Create investigation from palette with empty draft → ask for title via prompt default
        if let WorkspaceIntent::CreateInvestigation { draft } = &intent {
            if draft.title == "New Investigation" {
                app.conversation.push(WorkspaceMessage::assistant_text(
                    "Creating an Investigation. Describe the engineering question in the composer, or confirm the default title.",
                ));
                let pending = WorkspaceIntent::CreateInvestigation {
                    draft: InvestigationDraft {
                        title: "New Investigation".into(),
                        description: None,
                        suggested_sources: draft.suggested_sources.clone(),
                    },
                };
                request_confirmation(
                    app,
                    "Create Investigation",
                    "Title: New Investigation\n\nConfirm to create, or type a better description first.",
                    pending,
                );
                return;
            }
        }
        if matches!(intent, WorkspaceIntent::Quit) {
            request_confirmation(app, "Quit Rivora?", "Terminal will be restored.", intent);
            return;
        }
        dispatch_intent(app, intent);
    } else {
        app.notify(
            NotificationKind::Info,
            format!("{} needs more context", desc.label),
        );
    }
}

fn submit_composer(app: &mut WorkspaceApp) {
    let text = app.composer.input.trim().to_string();
    if text.is_empty() {
        return;
    }
    if text.chars().count() > crate::app::state::ComposerState::MAX_LEN {
        app.notify(NotificationKind::Warning, "Input too large");
        return;
    }

    app.composer.history.push(text.clone());
    if app.composer.history.len() > 100 {
        let drain = app.composer.history.len() - 100;
        app.composer.history.drain(0..drain);
    }
    app.composer.history_index = None;
    app.composer.clear();

    app.conversation
        .push(WorkspaceMessage::user_text(text.clone()));

    // Slash command without palette selection
    if text.starts_with('/') {
        let filter = text.trim_start_matches('/').trim();
        app.palette.filter = filter.to_string();
        refresh_app_palette(app);
        if let Some((desc, avail)) = app.palette.filtered.first() {
            if avail.is_available() {
                let ctx = ActionContext {
                    active_investigation: app.active_id(),
                    has_selected_proposal: app.selected_proposal_id.is_some(),
                    has_selected_plan: app.selected_plan_id.is_some(),
                    filter: "",
                };
                if let Some(intent) = (desc.intent_builder)(ctx) {
                    dispatch_intent(app, intent);
                    return;
                }
            }
        }
        app.conversation.push(WorkspaceMessage::assistant_text(
            "No matching action. Press / to browse.",
        ));
        return;
    }

    let interpreted = interpret_prompt(&text, app.active_id());
    if let Some(rationale) = &interpreted.rationale {
        if interpreted.confidence.is_low()
            || matches!(interpreted.intent, WorkspaceIntent::SubmitPrompt { .. })
        {
            app.conversation
                .push(WorkspaceMessage::assistant_text(rationale.clone()));
            if matches!(interpreted.intent, WorkspaceIntent::SubmitPrompt { .. }) {
                return;
            }
        }
    }

    if interpreted.requires_confirmation {
        let (title, body) =
            confirmation_copy(&interpreted.intent, interpreted.rationale.as_deref());
        request_confirmation(app, title, body, interpreted.intent);
        return;
    }

    dispatch_intent(app, interpreted.intent);
}

fn confirmation_copy(intent: &WorkspaceIntent, rationale: Option<&str>) -> (String, String) {
    match intent {
        WorkspaceIntent::CreateInvestigation { draft } => (
            "Create Investigation".into(),
            format!(
                "Title:\n{}\n\nSuggested sources:\n{}\n\n{}",
                draft.title,
                draft.suggested_sources.join(", "),
                rationale.unwrap_or("Create this Investigation?")
            ),
        ),
        WorkspaceIntent::Quit => (
            "Quit Rivora?".into(),
            "Leave the Workspace and restore the terminal.".into(),
        ),
        other => (
            "Confirm".into(),
            format!("{other:?}\n\n{}", rationale.unwrap_or("Proceed?")),
        ),
    }
}

fn request_confirmation(
    app: &mut WorkspaceApp,
    title: impl Into<String>,
    body: impl Into<String>,
    pending: WorkspaceIntent,
) {
    let title = title.into();
    let body = body.into();
    app.pending_intent = Some(pending.clone());
    app.composer.mode = ComposerMode::Confirm;
    app.conversation
        .push(WorkspaceMessage::confirmation(title.clone(), body.clone()));
    app.modal = Some(WorkspaceModal::Confirm {
        title,
        body,
        pending,
    });
}

fn confirm_pending(app: &mut WorkspaceApp) {
    app.modal = None;
    app.composer.mode = ComposerMode::Prompt;
    if let Some(intent) = app.pending_intent.take() {
        dispatch_intent(app, intent);
    }
}

fn cancel_pending(app: &mut WorkspaceApp) {
    app.modal = None;
    app.pending_intent = None;
    app.composer.mode = ComposerMode::Prompt;
    app.conversation
        .push(WorkspaceMessage::assistant_text("Cancelled."));
    app.notify(NotificationKind::Info, "Cancelled");
}

fn dispatch_intent(app: &mut WorkspaceApp, intent: WorkspaceIntent) {
    // UI-only fast path (no background thread needed).
    match &intent {
        WorkspaceIntent::Quit => {
            app.should_quit = true;
            return;
        }
        WorkspaceIntent::OpenHelp => {
            app.modal = Some(WorkspaceModal::Help);
            app.route = WorkspaceRoute::Help;
            return;
        }
        WorkspaceIntent::OpenHome => {
            app.route = WorkspaceRoute::Home;
            return;
        }
        WorkspaceIntent::Navigate { route } => {
            app.route = *route;
            return;
        }
        _ => {}
    }

    // Synchronous execute for responsiveness in local store ops.
    // Capability calls are local and typically fast; slow work can move to tasks later.
    app.composer.mode = ComposerMode::Busy;
    app.notify(NotificationKind::Progress, "Working…");
    let result = execute_intent(&app.caps, &intent);
    app.composer.mode = ComposerMode::Prompt;
    apply_result(app, result);
}

/// Apply a typed intent result to application state.
pub fn apply_result(app: &mut WorkspaceApp, result: IntentExecutionResult) {
    match result {
        IntentExecutionResult::Quit => {
            app.should_quit = true;
        }
        IntentExecutionResult::Navigate(route) => {
            app.route = route;
            app.notify(
                NotificationKind::Info,
                format!("Opened {}", app.route_label()),
            );
        }
        IntentExecutionResult::InvestigationCreated { id, title, summary } => {
            app.set_active_investigation(id, title.clone(), "created");
            app.route = WorkspaceRoute::Investigation;
            app.conversation.push(
                WorkspaceMessage::assistant_card("Investigation created", summary)
                    .with_refs(vec![id.to_string()]),
            );
            app.notify(NotificationKind::Success, "Investigation created");
            app.refresh_recent();
        }
        IntentExecutionResult::InvestigationOpened {
            id,
            title,
            status,
            summary,
        } => {
            app.set_active_investigation(id, title, &status);
            app.route = WorkspaceRoute::Investigation;
            app.conversation.push(
                WorkspaceMessage::assistant_card("Investigation", summary)
                    .with_refs(vec![id.to_string()]),
            );
            app.notify(NotificationKind::Success, "Investigation opened");
        }
        IntentExecutionResult::InvestigationList { items, summary } => {
            app.list_items = items;
            app.list_selected = 0;
            app.route = WorkspaceRoute::Search;
            app.focus = WorkspaceFocus::List;
            app.conversation
                .push(WorkspaceMessage::assistant_text(summary));
            app.notify(NotificationKind::Info, "Investigation list");
        }
        IntentExecutionResult::SearchResults {
            query,
            items,
            summary,
        } => {
            app.list_items = items;
            app.list_selected = 0;
            app.route = WorkspaceRoute::Search;
            app.focus = WorkspaceFocus::List;
            app.conversation
                .push(WorkspaceMessage::assistant_text(format!(
                    "Search: {query}\n{summary}"
                )));
            app.notify(NotificationKind::Info, "Search complete");
        }
        IntentExecutionResult::Info { title, body, route } => {
            if let Some(r) = route {
                app.route = r;
            }
            app.conversation
                .push(WorkspaceMessage::assistant_card(title.clone(), body));
            app.notify(NotificationKind::Info, title);
        }
        IntentExecutionResult::Panel {
            title,
            lines,
            route,
        } => {
            app.route = route;
            app.panel_title = title.clone();
            app.panel_lines = lines.clone();
            app.conversation.push(WorkspaceMessage::assistant_card(
                title.clone(),
                lines.join("\n"),
            ));
            app.notify(NotificationKind::Info, title);
        }
        IntentExecutionResult::CapabilityWork {
            title,
            body,
            investigation_id,
            object_refs,
            route,
        } => {
            if let Some(r) = route {
                app.route = r;
            }
            if let Some(id) = investigation_id {
                if let Ok(inv) = app.caps.open_investigation(id) {
                    app.set_active_investigation(id, inv.title, inv.status.as_str());
                }
            }
            // Capture first proposal-like ref when reviewing proposals.
            if title.to_lowercase().contains("proposal") {
                if let Some(first) = object_refs.first() {
                    if let Ok(pid) = first.parse::<rivora::domain::ObjectId>() {
                        app.selected_proposal_id = Some(pid);
                    }
                }
            }
            app.conversation
                .push(WorkspaceMessage::assistant_card(title.clone(), body).with_refs(object_refs));
            app.notify(NotificationKind::Success, title);
        }
        IntentExecutionResult::NeedsConfirmation {
            preview_title,
            preview_body,
            pending,
        } => {
            request_confirmation(app, preview_title, preview_body, pending);
        }
        IntentExecutionResult::Error(view) => {
            app.conversation.push(WorkspaceMessage::error(
                view.title.clone(),
                view.summary.clone(),
            ));
            app.notify(NotificationKind::Error, view.title.clone());
            app.modal = Some(WorkspaceModal::Error {
                title: view.title.clone(),
                body: view.display_lines().join("\n"),
            });
        }
        IntentExecutionResult::Clarification { message } => {
            app.conversation
                .push(WorkspaceMessage::assistant_text(message));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intent::IntentConfidence;

    #[test]
    fn confirmation_copy_for_create() {
        let intent = WorkspaceIntent::CreateInvestigation {
            draft: InvestigationDraft {
                title: "t".into(),
                description: None,
                suggested_sources: vec!["GitHub Actions".into()],
            },
        };
        let (title, body) = confirmation_copy(&intent, Some("confirm"));
        assert!(title.contains("Investigation"));
        assert!(body.contains("GitHub Actions"));
        let _ = IntentConfidence::new(1.0);
    }
}
