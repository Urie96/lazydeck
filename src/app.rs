use anyhow::{bail, Context, Result};
use crossterm::cursor::{Hide, MoveTo, SetCursorStyle, Show};
use crossterm::event::Event as CrosstermEvent;
use crossterm::execute;

use mlua::prelude::*;
use ratatui::{
    prelude::*,
    widgets::{Block, BorderType, Paragraph},
};
use tokio::sync::mpsc;

use libc::{sigaction, sigemptyset, SIGINT, SIG_IGN};
use std::mem;

use crate::{
    confirm_handler,
    events::{Event, Events},
    input_handler, path_codec, plugin, select_handler,
    term::{self, Term},
    widgets::{
        confirm::ConfirmWidget, footer::FooterWidget, header::HeaderWidget,
        input::InputDialogState, input::InputDialogWidget, list::ListWidget, select::SelectWidget,
    },
    State,
};

pub struct App {
    event_sender: mpsc::UnboundedSender<Event>,
    state: State,
    term: Term,
    quitting: bool,
    dirty: bool,
    lua: Lua,
    initial_path: Vec<String>,
}

impl App {
    pub fn new(
        event_sender: mpsc::UnboundedSender<Event>,
        term: Term,
        initial_path: Vec<String>,
    ) -> Self {
        let mut state = State::new();
        let lua = Lua::new();

        plugin::scope(&lua, &mut state, &event_sender, || plugin::init_lua(&lua))
            .expect("Failed to initialize Lua");

        Self {
            lua,
            event_sender,
            state,
            term,
            dirty: false,
            quitting: false,
            initial_path,
        }
    }

    /// Get cursor information (position and style) based on current mode
    fn get_cursor_info(&self) -> Option<(u16, u16, SetCursorStyle)> {
        // Check if input dialog is open (takes priority over select dialog and filter mode)
        if let Some(dialog) = &self.state.input_dialog {
            return Some((dialog.cursor_x, dialog.cursor_y, SetCursorStyle::SteadyBar));
        }

        // Check if select dialog is open (takes priority)
        if let Some(dialog) = &self.state.select_dialog {
            return Some((dialog.cursor_x, dialog.cursor_y, SetCursorStyle::SteadyBar));
        }

        // No cursor in main mode
        None
    }

    /// Set cursor after rendering (called by scopeguard)
    fn routine<B: std::io::Write>(
        backend: &mut B,
        cursor_info: Option<(u16, u16, SetCursorStyle)>,
    ) {
        if let Some((x, y, style)) = cursor_info {
            let _ = execute!(backend, style, MoveTo(x, y), Show);
        } else {
            let _ = execute!(backend, Hide);
        }
        let _ = backend.flush();
    }

    /// Runs the main loop of the application, handling events and actions
    pub async fn run(&mut self, mut events: Events) -> Result<()> {
        let from_cache = self.state.go_to(self.initial_path.clone(), false);
        if from_cache {
            self.refresh_preview()?;
        }
        self.call_list()?;
        self.run_post_page_enter_hooks()?;
        self.dirty = true;

        // Initially hide cursor (Main mode)
        execute!(self.term.backend_mut(), Hide)?;
        std::io::Write::flush(self.term.backend_mut())?;

        loop {
            if let Some(e) = events.next().await {
                self.handle_event(e)?;
            }
            if self.quitting {
                plugin::flush_pending_cache()?;
                break;
            }

            if self.dirty {
                self.state.prune_expired_notifications();
                let native_allowed = self.state.input_dialog.is_none()
                    && self.state.select_dialog.is_none()
                    && self.state.confirm_dialog.is_none();
                if let Some(preview) = self.state.current_preview.as_mut() {
                    preview.set_native_enabled(native_allowed);
                }
                let _ = crate::widgets::native_image::clear(self.term.backend_mut())?;

                // Hide cursor during rendering
                execute!(self.term.backend_mut(), Hide)?;

                // Render
                self.term.draw(|frame| {
                    frame.render_stateful_widget(AppWidget, frame.area(), &mut self.state);
                })?;

                let _rendered_native = if native_allowed {
                    if let Some(preview) = self.state.current_preview.as_mut() {
                        preview.render_native(self.term.backend_mut())?
                    } else {
                        false
                    }
                } else {
                    false
                };
                // Get cursor info after rendering (uses updated filter_cursor_x/y)
                let cursor_info = self.get_cursor_info();

                // Restore cursor state after draw (like yazi)
                Self::routine(self.term.backend_mut(), cursor_info);

                self.dirty = false;
            }
        }
        Ok(())
    }

    fn call_list(&mut self) -> Result<()> {
        anyhow::Context::context(
            plugin::scope(&self.lua, &mut self.state, &self.event_sender, || {
                let lc: LuaTable = self.lua.globals().get("lc")?;
                let list_fn: LuaFunction = lc.get("_list")?;
                list_fn.call::<()>(())
            }),
            "Failed to call lc._list",
        )
    }

    fn call_preview(&mut self) -> Result<()> {
        anyhow::Context::context(
            plugin::scope(&self.lua, &mut self.state, &self.event_sender, || {
                let lc: LuaTable = self.lua.globals().get("lc")?;
                let preview_fn: LuaFunction = lc.get("_preview")?;
                preview_fn.call::<()>(())
            }),
            "Failed to call lc._preview",
        )
    }

    fn refresh_preview(&mut self) -> Result<()> {
        if !self.state.restore_preview_for_hovered() {
            self.call_preview()?;
        }
        Ok(())
    }

    fn navigate_to(&mut self, path: Vec<String>, record_history: bool) -> Result<()> {
        let from_cache = self.state.go_to(path, record_history);
        if from_cache {
            self.refresh_preview()?;
        }
        self.call_list()?;
        self.run_post_page_enter_hooks()?;
        self.dirty = true;
        Ok(())
    }

    fn run_post_page_enter_hooks(&mut self) -> Result<()> {
        let current_path = self.state.current_path.clone();
        let payload = plugin::scope(&self.lua, &mut self.state, &self.event_sender, || {
            let payload = self.lua.create_table()?;
            let path = self
                .lua
                .create_sequence_from(current_path.iter().cloned())?;
            payload.set("path", path)?;
            Ok::<LuaTable, mlua::Error>(payload)
        })?;

        for hook in self.state.post_page_enter_hooks.clone() {
            let payload = payload.clone();
            plugin::scope(&self.lua, &mut self.state, &self.event_sender, || {
                hook.call::<()>(payload.clone())
            })?;
        }

        Ok(())
    }

    fn handle_event(&mut self, e: Event) -> Result<()> {
        if self.state.prune_expired_notifications() {
            self.dirty = true;
        }

        match e {
            Event::Quit => {
                self.quitting = true;
            }
            // Event::Tick => Some(Action::Tick),
            Event::Render => {
                self.dirty = true;
            }
            Event::RefreshPreview => {
                self.refresh_preview()?;
                self.dirty = true;
            }
            Event::Crossterm(CrosstermEvent::Resize(_, _)) => {
                self.dirty = true;
            }
            Event::Crossterm(CrosstermEvent::Key(key)) => {
                // If confirm dialog is shown, handle its keyboard input first
                if self.state.confirm_dialog.is_some() {
                    if confirm_handler::handle_confirm_dialog_key(
                        &self.lua,
                        &mut self.state,
                        &self.event_sender,
                        key,
                    )? {
                        self.dirty = true;
                    }
                    return Ok(());
                }

                // If select dialog is shown, handle its keyboard input first
                if self.state.select_dialog.is_some() {
                    if select_handler::handle_select_dialog_key(
                        &self.lua,
                        &mut self.state,
                        &self.event_sender,
                        key,
                    )? {
                        self.dirty = true;
                    }
                    return Ok(());
                }

                // If input dialog is shown, handle its keyboard input
                if self.state.input_dialog.is_some() {
                    if input_handler::handle_input_dialog_key(
                        &self.lua,
                        &mut self.state,
                        &self.event_sender,
                        key,
                    )? {
                        self.dirty = true;
                    }
                    return Ok(());
                }

                // Handle key events in main mode
                let cb = { self.state.tap_key(key)? };
                if let Some(cb) = cb {
                    plugin::scope(&self.lua, &mut self.state, &self.event_sender, || {
                        cb.call::<()>(())
                    })?;
                }
            }
            Event::Crossterm(_) => {}
            Event::Command(command) => self.handle_command(command.as_str())?,
            Event::AddKeymap(keymap) => self.state.add_keymap(keymap),
            Event::Enter(path) => self.navigate_to(path, true)?,
            Event::LuaCallback(cb) => {
                plugin::scope(&self.lua, &mut self.state, &self.event_sender, || {
                    cb(&self.lua)?;
                    Ok(())
                })?;
            }
            Event::InteractiveCommand {
                cmd,
                on_complete,
                wait_confirm,
            } => {
                // Execute the interactive command
                let result = self.execute_interactive_command(cmd, wait_confirm);

                self.dirty = true;

                // Call the completion callback if provided
                if let Some(cb) = on_complete {
                    let exit_code = match result {
                        Ok(code) => code,
                        Err(e) => {
                            // Log the error and use -1 as exit code
                            eprintln!("Error executing interactive command: {}", e);
                            -1
                        }
                    };
                    plugin::scope(&self.lua, &mut self.state, &self.event_sender, || {
                        cb.call::<()>(exit_code)?;
                        Ok(())
                    })?;
                }
            }
            Event::Notify(message) => {
                self.state.push_notification(message);
                self.dirty = true;
            }
            Event::ShowConfirm {
                title,
                prompt,
                on_confirm,
                on_cancel,
            } => {
                self.state
                    .show_confirm_dialog(title, prompt, on_confirm, on_cancel);
                self.dirty = true;
            }
            Event::ShowSelect {
                prompt,
                options,
                on_selection,
            } => {
                self.state.select_dialog =
                    Some(crate::SelectDialog::new(prompt, options, on_selection));
                self.dirty = true;
            }
            Event::ShowInput {
                prompt,
                placeholder,
                value,
                on_submit,
                on_cancel,
                on_change,
            } => {
                self.state.show_input_dialog(
                    prompt,
                    placeholder,
                    value,
                    on_submit,
                    on_cancel,
                    on_change,
                );
                self.dirty = true;
            }
        }
        Ok(())
    }

    fn execute_interactive_command(
        &mut self,
        cmd: Vec<String>,
        wait_confirm: Option<LuaFunction>,
    ) -> Result<i32> {
        if cmd.is_empty() {
            bail!("Interactive command cannot be empty");
        }

        let mut it = cmd.iter();
        let program = it.next().unwrap();
        let args: Vec<&String> = it.collect();

        // Temporarily ignore SIGINT during interactive command execution
        // This prevents Ctrl-C from terminating lazycmd itself
        let mut old_action: libc::sigaction = unsafe { mem::zeroed() };
        let mut new_action: libc::sigaction = unsafe { mem::zeroed() };

        unsafe {
            // Get the current SIGINT handler
            sigaction(SIGINT, std::ptr::null(), &mut old_action);

            // Set SIGINT to ignore (SIG_IGN)
            new_action.sa_sigaction = SIG_IGN;
            sigemptyset(&mut new_action.sa_mask);
            new_action.sa_flags = 0;
            sigaction(SIGINT, &new_action, std::ptr::null_mut());
        }

        // Temporarily restore the terminal to let the subprocess take control
        term::restore();

        // Execute the command and wait for it to complete
        let result = std::process::Command::new(program)
            .args(&args)
            .status()
            .context(format!("Failed to execute command: {}", program))?;

        let exit_code = result.code().unwrap_or(-1);

        // Restore the original SIGINT handler
        unsafe {
            sigaction(SIGINT, &old_action, std::ptr::null_mut());
        }

        // If wait_confirm function is provided, call it to decide whether to wait
        let should_wait = if let Some(ref wait_fn) = wait_confirm {
            plugin::scope(&self.lua, &mut self.state, &self.event_sender, || {
                let result: bool = wait_fn.call::<bool>(exit_code)?;
                Ok(result)
            })
            .unwrap_or(false)
        } else {
            false
        };

        if should_wait {
            println!("\nPress Enter to return to lazycmd...");
            let _ = std::io::stdin().read_line(&mut String::new());
        }

        // Re-initialize the terminal for TUI
        self.term = term::init()?;

        // Clear any pending input events to prevent spurious key presses
        // This handles the case where the subprocess (e.g., vim) leaves
        // input in the terminal buffer that would otherwise be captured
        while crossterm::event::poll(std::time::Duration::from_millis(10)).unwrap_or(false) {
            let _ = crossterm::event::read();
        }

        // Return the exit code
        Ok(exit_code)
    }

    fn resolve_command_path(current_path: &[String], raw_path: &str) -> Result<Vec<String>> {
        let raw_path = raw_path.trim();
        if raw_path.is_empty() {
            bail!("cd requires a target path");
        }

        let mut path = if raw_path.starts_with('/') {
            Vec::new()
        } else {
            current_path.to_vec()
        };

        for segment in raw_path.split('/') {
            match segment {
                "" | "." => {}
                ".." => {
                    path.pop();
                }
                _ => path.push(path_codec::decode_path_segment_input(segment)?),
            }
        }

        Ok(path)
    }


    fn open_command_prompt(&mut self, initial_value: String) -> Result<()> {
        let sender = self.event_sender.clone();
        let on_submit = self
            .lua
            .create_function(move |_, input: String| -> mlua::Result<()> {
                let command = input.trim().to_string();
                if !command.is_empty() {
                    sender
                        .send(Event::Command(command))
                        .map_err(|err| mlua::Error::RuntimeError(err.to_string()))?;
                }
                Ok(())
            })?
            .to_owned();
        let on_cancel = self.lua.create_function(|_, ()| Ok(()))?.to_owned();
        let on_change = self.lua.create_function(|_, ()| Ok(()))?.to_owned();

        self.state.show_input_dialog(
            ":".to_string(),
            "输入命令...".to_string(),
            initial_value,
            on_submit,
            on_cancel,
            on_change,
        );
        self.dirty = true;
        Ok(())
    }

    fn handle_command(&mut self, command: &str) -> Result<()> {
        let splits = shell_words::split(command)?;
        if splits.is_empty() {
            bail!("Empty command {}", command)
        }
        let mut it = splits.iter();
        match it.next().unwrap().as_str() {
            "quit" => {
                for hook in self.state.pre_quit_hooks.clone() {
                    plugin::scope(&self.lua, &mut self.state, &self.event_sender, || {
                        hook.call::<()>(())
                    })?;
                }
                self.quitting = true;
            }
            "scroll_by" => {
                let num = match it.next() {
                    Some(num) => num.parse::<i16>().context("wrong format for scroll_by")?,
                    None => 1,
                };
                self.state.scroll_by(num);
                self.refresh_preview()?;
                self.dirty = true;
            }
            "scroll_preview_by" => {
                let num = match it.next() {
                    Some(num) => num
                        .parse::<i16>()
                        .context("wrong format for scroll_preview_by")?,
                    None => 1,
                };
                self.state.scroll_preview_by(num);
                self.dirty = true;
            }
            "reload" => {
                // Call all pre_reload hooks before executing reload
                for hook in self.state.pre_reload_hooks.clone() {
                    plugin::scope(&self.lua, &mut self.state, &self.event_sender, || {
                        hook.call::<()>(())
                    })?;
                }
                // Save the selected entry key to restore later
                let selected_key = self.state.hovered().map(|e| e.key.clone());
                let selected_hovered_path = self.state.hovered_path();
                if let Some(path) = &selected_hovered_path {
                    self.state.clear_preview_for_path(path);
                }
                self.state.clear_current_cache();
                self.call_list()?;
                // Restore selection by finding the entry with the same key
                let mut selection_changed = false;
                if let Some(key) = selected_key {
                    if let Some(page) = &mut self.state.current_page {
                        // Find the index of the entry with the same key
                        if let Some(idx) = page.filtered_list.iter().position(|e| e.key == key) {
                            selection_changed = page.list_state.selected() != Some(idx);
                            page.list_state.select(Some(idx));
                        } else if !page.filtered_list.is_empty() {
                            // Entry not found, keep the current selection or select the first item
                            if page.list_state.selected().is_none() {
                                page.list_state.select(Some(0));
                                selection_changed = true;
                            }
                        }
                    }
                }
                if selection_changed {
                    self.refresh_preview()?;
                }
                self.dirty = true;
            }
            "cd" => {
                let raw_path = it.cloned().collect::<Vec<_>>().join(" ");
                let path = Self::resolve_command_path(&self.state.current_path, &raw_path)?;
                self.navigate_to(path, true)?;
            }
            "command_prompt" => {
                let initial_value = it.cloned().collect::<Vec<_>>().join(" ");
                self.open_command_prompt(initial_value)?;
            }
            "enter" => {
                if let Some(hovered) = self.state.hovered() {
                    let mut path = self.state.current_path.clone();
                    path.push(hovered.key.clone());
                    self.navigate_to(path, true)?;
                }
            }
            "back" => {
                let mut path = self.state.current_path.clone();
                if !path.is_empty() {
                    path.pop();
                    self.navigate_to(path, true)?;
                }
            }
            "history_back" => {
                if let Some(path) = self.state.pop_history_path() {
                    self.navigate_to(path, false)?;
                }
            }
            "input_submit" => {
                if let Some((text, on_submit)) = self.state.input_dialog_submit() {
                    plugin::scope(&self.lua, &mut self.state, &self.event_sender, || {
                        on_submit.call::<()>(text)
                    })?;
                    self.dirty = true;
                }
            }
            "input_cancel" => {
                if let Some(on_cancel) = self.state.input_dialog_cancel() {
                    plugin::scope(&self.lua, &mut self.state, &self.event_sender, || {
                        on_cancel.call::<()>(())
                    })?;
                    self.dirty = true;
                }
            }
            "input_clear_before_cursor" => {
                if let Some((text, on_change)) = self.state.input_dialog_clear_before_cursor() {
                    plugin::scope(&self.lua, &mut self.state, &self.event_sender, || {
                        on_change.call::<()>(text)
                    })?;
                    self.dirty = true;
                }
            }
            "input_cursor_to_start" => {
                if self.state.input_dialog_cursor_to_start() {
                    self.dirty = true;
                }
            }
            "input_cursor_to_end" => {
                if self.state.input_dialog_cursor_to_end() {
                    self.dirty = true;
                }
            }
            _ => {
                self.state
                    .push_notification(Text::from(format!("Unsupported command {}", command)));
                self.dirty = true;
            }
        };
        Ok(())
    }
}

struct AppWidget;

fn render_loading_placeholder(area: Rect, buf: &mut Buffer) {
    if area.width < 12 || area.height < 5 {
        Paragraph::new("Loading")
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::Cyan).bold())
            .render(area, buf);
        return;
    }

    let phase = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| (d.as_millis() as usize / 200) % 5)
        .unwrap_or(0);
    let pulse = ["▰▱▱▱▱", "▰▰▱▱▱", "▰▰▰▱▱", "▰▰▰▰▱", "▰▰▰▰▰"][phase];

    let text = Text::from(vec![
        Line::from(vec![
            Span::styled("◌", Style::default().fg(Color::LightCyan)),
            Span::raw(" "),
            Span::styled("Loading", Style::default().fg(Color::White).bold()),
            Span::raw(" "),
            Span::styled("◌", Style::default().fg(Color::LightBlue)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(pulse, Style::default().fg(Color::LightCyan)),
            Span::raw(" "),
            Span::styled("fetching page", Style::default().fg(Color::White)),
        ]),
        Line::from(vec![Span::styled(
            "please hold tight",
            Style::default().fg(Color::DarkGray).italic(),
        )]),
    ]);

    Paragraph::new(text)
        .alignment(Alignment::Center)
        .render(area, buf);
}

impl StatefulWidget for AppWidget {
    type State = State;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut State) {
        use Constraint::*;

        // Layout: header (1), main (remaining), footer (1)
        let [header_area, main_area, footer_area] =
            Layout::vertical([Length(1), Min(3), Length(1)]).areas(area);

        HeaderWidget.render(header_area, buf, state);

        // Render footer with list counter
        FooterWidget.render(footer_area, buf, state);

        let block_color = Color::DarkGray;

        // Draw outer border and split into list/preview areas
        let main_block = Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(block_color));
        let main_inner = main_block.inner(main_area);
        main_block.render(main_area, buf);

        // Split into list, divider(1), preview areas
        let [list_area, _divider_area, preview_area] =
            Layout::horizontal([Percentage(50), Length(1), Fill(1)]).areas(main_inner);

        let scrolloff = state.scrolloff;

        if let Some(page) = &mut state.current_page {
            let list_widget = ListWidget { scrolloff };
            list_widget.render(list_area, buf, page);
        } else {
            render_loading_placeholder(list_area, buf);
        }

        // Draw vertical divider line from top to bottom of the outer border
        for y in main_area.top()..main_area.bottom() {
            buf[(_divider_area.left(), y)]
                .set_symbol(symbols::line::VERTICAL)
                .set_style(Style::default().fg(block_color));
        }

        // Connect divider to top border - replace corner with ┬
        buf[(_divider_area.left(), main_area.top())]
            .set_symbol("┬")
            .set_style(Style::default().fg(block_color));

        // Connect divider to bottom border - replace corner with ┴
        buf[(_divider_area.left(), main_area.bottom() - 1)]
            .set_symbol("┴")
            .set_style(Style::default().fg(block_color));

        if let Some(p) = state.current_preview.as_mut() {
            p.render(preview_area, buf);
        }

        // Draw notifications in bottom-right corner, stacked upward
        let min_width = 20u16;
        let min_height = 1u16;
        let right_padding = 2u16;
        let bottom_padding = 1u16;
        let gap = 1u16;
        let mut bottom = area.height.saturating_sub(bottom_padding);

        for item in state.notifications.iter().rev() {
            let message = &item.message;
            let line_count = message.lines.len().max(min_height as usize);
            let max_line_width = message
                .lines
                .iter()
                .map(|l| l.width() as u16)
                .max()
                .unwrap_or(0);
            let notification_width = (max_line_width + 2).max(min_width).min(area.width);
            let notification_height = ((line_count as u16).max(min_height) + 2).min(area.height);

            if notification_height > bottom {
                break;
            }
            let y = bottom.saturating_sub(notification_height);
            let x = area.width.saturating_sub(notification_width + right_padding);
            let notification_area = Rect {
                x,
                y,
                width: notification_width,
                height: notification_height,
            };

            let block = Block::bordered()
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Yellow));
            let inner = block.inner(notification_area);
            block.render(notification_area, buf);
            Paragraph::new(message.clone())
                .style(Style::default().fg(Color::Yellow))
                .render(inner, buf);

            if y < gap {
                break;
            }
            bottom = y.saturating_sub(gap);
        }

        // Render confirm dialog (render last to appear on top of everything)
        if let Some(dialog) = &mut state.confirm_dialog {
            // Fixed size: width 40, height 10
            let dialog_width = 40u16;
            let dialog_height = 10u16;

            // Center the dialog
            let x = (area.width.saturating_sub(dialog_width)) / 2;
            let y = (area.height.saturating_sub(dialog_height)) / 2;

            // Ensure dialog is within bounds
            let x = x.min(area.width.saturating_sub(dialog_width));
            let y = y.min(area.height.saturating_sub(dialog_height));

            let dialog_area = Rect {
                x,
                y,
                width: dialog_width,
                height: dialog_height,
            };

            ConfirmWidget.render(dialog_area, buf, dialog);
        }

        // Render select dialog (render last to appear on top of everything)
        if let Some(dialog) = &mut state.select_dialog {
            // Calculate dialog dimensions: fixed size or clamped to fit
            let dialog_width = 80.min(area.width).max(40);
            let dialog_height = 20.min(area.height).max(10);

            // Center the dialog
            let x = (area.width.saturating_sub(dialog_width)) / 2;
            let y = (area.height.saturating_sub(dialog_height)) / 2;

            let dialog_area = Rect {
                x,
                y,
                width: dialog_width,
                height: dialog_height,
            };

            SelectWidget.render(dialog_area, buf, dialog);
        }

        // Render input dialog (render last to appear on top of everything)
        if let Some(dialog) = &mut state.input_dialog {
            // Fixed size: width 50, height 3
            let dialog_width = 50u16;
            let dialog_height = 3u16;

            // Center the dialog
            let x = (area.width.saturating_sub(dialog_width)) / 2;
            let y = (area.height.saturating_sub(dialog_height)) / 2;

            // Ensure dialog is within bounds
            let x = x.min(area.width.saturating_sub(dialog_width));
            let y = y.min(area.height.saturating_sub(dialog_height));

            let dialog_area = Rect {
                x,
                y,
                width: dialog_width,
                height: dialog_height,
            };

            let mut input_state = InputDialogState::new(&dialog.prompt, &dialog.placeholder);
            input_state.text = dialog.text.clone();
            input_state.cursor_position = dialog.cursor_position;
            InputDialogWidget::new().render(dialog_area, buf, &mut input_state);

            // Store cursor position for use in app.rs after draw completes
            dialog.cursor_x = input_state.cursor_x;
            dialog.cursor_y = input_state.cursor_y;
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::events::Event;

    use super::App;

    #[test]
    fn command_prompt_submit_triggers_command_event() {
        let lua = mlua::Lua::new();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

        let sender = tx.clone();
        let on_submit = lua
            .create_function(move |_, input: String| -> mlua::Result<()> {
                let command = input.trim().to_string();
                if !command.is_empty() {
                    sender
                        .send(Event::Command(command))
                        .map_err(|err| mlua::Error::RuntimeError(err.to_string()))?;
                }
                Ok(())
            })
            .unwrap();

        on_submit.call::<()>("reload".to_string()).unwrap();

        match rx.try_recv().unwrap() {
            Event::Command(command) => assert_eq!(command, "reload"),
            _ => panic!("expected command event"),
        }
    }

    #[test]
    fn resolve_command_path_supports_absolute_relative_and_parent_segments() {
        let current_path = vec!["github".to_string(), "search".to_string()];

        assert_eq!(
            App::resolve_command_path(&current_path, "/github/repo/lazygit").unwrap(),
            vec![
                "github".to_string(),
                "repo".to_string(),
                "lazygit".to_string()
            ]
        );
        assert_eq!(
            App::resolve_command_path(&current_path, "repo/lazygit").unwrap(),
            vec![
                "github".to_string(),
                "search".to_string(),
                "repo".to_string(),
                "lazygit".to_string()
            ]
        );
        assert_eq!(
            App::resolve_command_path(&current_path, "../repo/lazygit").unwrap(),
            vec![
                "github".to_string(),
                "repo".to_string(),
                "lazygit".to_string()
            ]
        );
        assert_eq!(
            App::resolve_command_path(&current_path, "/").unwrap(),
            Vec::<String>::new()
        );
    }

    #[test]
    fn resolve_command_path_decodes_percent_encoded_segments() {
        let current_path = vec![
            "github".to_string(),
            "repo".to_string(),
            "tpope".to_string(),
        ];

        assert_eq!(
            App::resolve_command_path(&current_path, "vim-abolish/tags/feature%2Ftest").unwrap(),
            vec![
                "github".to_string(),
                "repo".to_string(),
                "tpope".to_string(),
                "vim-abolish".to_string(),
                "tags".to_string(),
                "feature/test".to_string(),
            ]
        );
    }
}
