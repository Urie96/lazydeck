use ratatui::{prelude::*, widgets::*};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// Alias for backward compatibility (used by filter mode)
pub type InputState = InputDialogState;

#[derive(Debug)]
pub struct InputDialogState {
    pub text: String,
    pub cursor_position: usize,
    pub cursor_x: u16,
    pub cursor_y: u16,
    /// Custom prompt to display
    pub prompt: String,
    /// Placeholder text (shown when text is empty)
    pub placeholder: String,
    /// Whether the input should be masked (e.g. password entry)
    pub secret: bool,
}

impl InputDialogState {
    fn prev_char_boundary(text: &str, cursor_position: usize) -> usize {
        text[..cursor_position]
            .char_indices()
            .next_back()
            .map(|(idx, _)| idx)
            .unwrap_or(0)
    }

    fn next_char_boundary(text: &str, cursor_position: usize) -> usize {
        text[cursor_position..]
            .char_indices()
            .nth(1)
            .map(|(idx, _)| cursor_position + idx)
            .unwrap_or(text.len())
    }

    pub fn new(prompt: &str, placeholder: &str) -> Self {
        Self {
            text: String::new(),
            cursor_position: 0,
            cursor_x: 0,
            cursor_y: 0,
            prompt: prompt.to_string(),
            placeholder: placeholder.to_string(),
            secret: false,
        }
    }

    /// Create from filter input (for filter mode compatibility)
    pub fn from_filter_input(s: &str) -> Self {
        Self {
            text: s.to_string(),
            cursor_position: s.len(),
            cursor_x: 0,
            cursor_y: 0,
            prompt: " ".to_string(),
            placeholder: String::new(),
            secret: false,
        }
    }

    pub fn insert_char(&mut self, c: char) {
        self.text.insert(self.cursor_position, c);
        self.cursor_position += c.len_utf8();
    }

    pub fn backspace(&mut self) {
        if self.cursor_position > 0 {
            let prev_pos = Self::prev_char_boundary(&self.text, self.cursor_position);
            self.text.remove(prev_pos);
            self.cursor_position = prev_pos;
        }
    }

    pub fn delete(&mut self) {
        if self.cursor_position < self.text.len() {
            self.text.remove(self.cursor_position);
        }
    }

    pub fn clear(&mut self) {
        self.text.clear();
        self.cursor_position = 0;
    }

    pub fn cursor_left(&mut self) {
        if self.cursor_position > 0 {
            self.cursor_position = Self::prev_char_boundary(&self.text, self.cursor_position);
        }
    }

    pub fn cursor_right(&mut self) {
        if self.cursor_position < self.text.len() {
            self.cursor_position = Self::next_char_boundary(&self.text, self.cursor_position);
        }
    }

    pub fn cursor_to_start(&mut self) {
        self.cursor_position = 0;
    }

    pub fn cursor_to_end(&mut self) {
        self.cursor_position = self.text.len();
    }
}

#[cfg(test)]
mod tests {
    use super::{InputDialogState, InputDialogWidget};
    use ratatui::{buffer::Buffer, layout::Rect, widgets::StatefulWidget};

    #[test]
    fn input_dialog_state_backspace_handles_utf8() {
        let mut state = InputDialogState::new("Search", "keyword");
        state.insert_char('搜');
        state.insert_char('索');

        state.backspace();
        assert_eq!(state.text, "搜");
        assert_eq!(state.cursor_position, '搜'.len_utf8());
    }

    #[test]
    fn visible_window_keeps_cursor_at_right_edge_when_input_overflows() {
        let text = "abcdef";
        let (_, visible, cursor_offset) = InputDialogWidget::visible_window(text, text.len(), 3);

        assert_eq!(visible, "ef");
        assert_eq!(cursor_offset, 2);
    }

    #[test]
    fn input_dialog_render_scrolls_text_horizontally() {
        let mut state = InputDialogState::new("Search", "keyword");
        state.text = "abcdef".to_string();
        state.cursor_position = state.text.len();

        let area = Rect::new(0, 0, 8, 3);
        let mut buf = Buffer::empty(area);
        InputDialogWidget::new().render(area, &mut buf, &mut state);

        assert_eq!(state.cursor_x, 6);
        assert_eq!(buf[(4, 1)].symbol(), "e");
        assert_eq!(buf[(5, 1)].symbol(), "f");
        assert_eq!(buf[(6, 1)].symbol(), " ");
    }

    #[test]
    fn input_dialog_secret_mode_masks_text() {
        let mut state = InputDialogState::new("Password", "");
        state.secret = true;
        state.text = "hunter2".to_string();
        state.cursor_position = state.text.len();

        let area = Rect::new(0, 0, 20, 3);
        let mut buf = Buffer::empty(area);
        InputDialogWidget::new().render(area, &mut buf, &mut state);

        // All visible characters should be masked bullets (arrow occupies x=1..2, text starts at x=3)
        for x in 0.."hunter2".len() {
            assert_eq!(
                buf[((3 + x) as u16, 1)].symbol(),
                "•",
                "cell {} should be masked",
                3 + x
            );
        }
        // The real text must never appear in the buffer
        for cell in buf.content.iter() {
            assert!(
                !cell.symbol().contains("hunter2"),
                "secret text leaked into render buffer"
            );
        }
    }
}

/// Widget for rendering an input dialog with customizable prompt and title
pub struct InputDialogWidget;

impl InputDialogWidget {
    pub fn new() -> Self {
        Self
    }

    fn char_width(c: char) -> u16 {
        c.width().unwrap_or(0) as u16
    }

    fn display_width(text: &str) -> u16 {
        text.chars().map(Self::char_width).sum()
    }

    fn visible_window(text: &str, cursor_position: usize, max_width: u16) -> (usize, String, u16) {
        if text.is_empty() || max_width == 0 {
            return (0, String::new(), 0);
        }

        let cursor_position = cursor_position.min(text.len());
        let total_width_before_cursor = Self::display_width(&text[..cursor_position]);
        let max_width_before_cursor = if total_width_before_cursor >= max_width {
            max_width.saturating_sub(1)
        } else {
            max_width
        };

        let chars: Vec<(usize, char)> = text.char_indices().collect();
        let char_count = chars.len();
        let cursor_char_idx = chars
            .iter()
            .position(|(idx, _)| *idx >= cursor_position)
            .unwrap_or(char_count);

        let mut start_char_idx = 0usize;
        let mut width_before_cursor = 0u16;
        for idx in (0..cursor_char_idx).rev() {
            let next_width = width_before_cursor.saturating_add(Self::char_width(chars[idx].1));
            if next_width > max_width_before_cursor {
                start_char_idx = idx + 1;
                break;
            }
            width_before_cursor = next_width;
        }

        let start_byte = chars
            .get(start_char_idx)
            .map(|(idx, _)| *idx)
            .unwrap_or(text.len());

        let mut end_byte = text.len();
        let mut visible_width = 0u16;
        for idx in start_char_idx..char_count {
            let ch_width = Self::char_width(chars[idx].1);
            if visible_width.saturating_add(ch_width) > max_width {
                end_byte = chars[idx].0;
                break;
            }
            visible_width = visible_width.saturating_add(ch_width);
        }

        let visible_text = text[start_byte..end_byte].to_string();
        let cursor_offset = Self::display_width(&text[start_byte..cursor_position]);

        (
            start_byte,
            visible_text,
            cursor_offset.min(max_width_before_cursor),
        )
    }
}

impl StatefulWidget for InputDialogWidget {
    type State = InputDialogState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        // Clear the area first to prevent previous content from leaking through
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_char(' ');
                }
            }
        }

        // Arrow prefix " " for inside the input box (like select component)
        let arrow = " ";

        let inner_width = area.width.saturating_sub(2);
        let arrow_width = arrow.width() as u16;
        let text_max_width = inner_width.saturating_sub(arrow_width);

        // Determine the text to display: actual text or placeholder
        let (display_text, cursor_char_width) = if state.text.is_empty() {
            let placeholder = if text_max_width == 0 {
                String::new()
            } else {
                let (_, text, _) = Self::visible_window(&state.placeholder, 0, text_max_width);
                text
            };
            (placeholder, 0)
        } else {
            let (_, text, cursor_offset) =
                Self::visible_window(&state.text, state.cursor_position, text_max_width);
            // Mask the input when in secret mode (e.g. password entry)
            let text = if state.secret {
                text.chars().map(|_| '•').collect()
            } else {
                text
            };
            (text, cursor_offset)
        };

        // Determine text color: gray for placeholder, white for actual input
        let text_color = if state.text.is_empty() {
            Color::DarkGray
        } else {
            Color::White
        };

        // Create text with arrow + user input (prompt is shown in title bar only)
        let text = Text::from(Line::from(vec![
            Span::styled(arrow, Style::default().fg(Color::Cyan)),
            Span::styled(display_text, Style::default().fg(text_color)),
        ]));

        // Title shows the prompt (like select component)
        // If prompt is empty, show "Input" as default
        let title_text = if state.prompt.is_empty() {
            "Input".to_string()
        } else {
            state.prompt.clone()
        };

        let paragraph = Paragraph::new(text).block(
            Block::bordered()
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Cyan))
                .title_style(
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )
                .title_alignment(Alignment::Center)
                .title(title_text),
        );

        paragraph.render(area, buf);

        // Calculate cursor position (arrow + cursor position in text)
        let cursor_x = (area.x + 1 + arrow_width + cursor_char_width)
            .min(area.x + area.width.saturating_sub(2));
        let cursor_y = area.y + 1; // Inside the bordered area

        // Store cursor position for use by app.rs
        state.cursor_x = cursor_x;
        state.cursor_y = cursor_y;
    }
}
