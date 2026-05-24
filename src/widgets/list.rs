use ratatui::prelude::*;

use crate::Page;

/// List widget with scrolloff - keeps cursor away from edges (like vim's scrolloff)
pub struct ListWidget {
    pub scrolloff: usize,
    pub selected_entry_keys: Vec<String>,
}

impl Default for ListWidget {
    fn default() -> Self {
        Self {
            scrolloff: 5,
            selected_entry_keys: Vec::new(),
        }
    }
}

impl StatefulWidget for ListWidget {
    type State = Page;

    fn render(self, area: Rect, buf: &mut Buffer, page: &mut Self::State) {
        // Adjust offset based on scrolloff before rendering
        if let Some(selected) = page.list_state.selected() {
            let height = area.height as usize;
            let scrolloff = self.scrolloff.min(height / 2);
            let offset = page.list_state.offset();
            let cursor_pos = selected.saturating_sub(offset);
            let len = page.filtered_list.len();

            // When cursor is in the top scrolloff zone, scroll up to keep cursor at scrolloff
            if cursor_pos < scrolloff && offset > 0 {
                // Keep cursor at scrolloff position
                let new_offset = selected.saturating_sub(scrolloff);
                *page.list_state.offset_mut() = new_offset;
            }
            // When cursor is in the bottom scrolloff zone, scroll down
            else if cursor_pos >= height.saturating_sub(scrolloff) {
                let desired_pos = height.saturating_sub(scrolloff).saturating_sub(1);
                if selected >= desired_pos {
                    let new_offset = selected.saturating_sub(desired_pos);
                    // Limit offset so the last item is at or near bottom
                    let max_offset = if len > height { len - height } else { 0 };
                    *page.list_state.offset_mut() = new_offset.min(max_offset);
                }
            }
        }

        // Custom rendering with padding and selection markers
        let offset = page.list_state.offset();
        let selected = page.list_state.selected();
        let height = area.height as usize;

        for (i, entry) in page
            .filtered_list
            .iter()
            .enumerate()
            .skip(offset)
            .take(height)
        {
            let y = area.top() + (i - offset) as u16;
            let is_selected = Some(i) == selected;
            let is_api_selected = self.selected_entry_keys.iter().any(|key| key == &entry.key);

            // Get display text
            let line = entry.display();

            if is_selected {
                let selected_color = Color::Rgb(60, 60, 70);
                // Selected: render with background and white foreground for contrast
                let selected_style = Style::default().fg(Color::White).bg(selected_color);

                // Left marker  with selected color foreground only (no background)
                buf[(area.left(), y)]
                    .set_char('')
                    .set_style(Style::default().fg(selected_color));

                // Right marker  with selected color foreground only (no background)
                buf[(area.right() - 1, y)]
                    .set_char('')
                    .set_style(Style::default().fg(selected_color));

                // Content area (with one space padding on each side)
                let content_area = Rect {
                    x: area.left() + 1,
                    y,
                    width: area.width.saturating_sub(2),
                    height: 1,
                };

                // Clear and fill content area with background
                for x in content_area.left()..content_area.right() {
                    buf[(x, y)].set_char(' ').set_style(selected_style);
                }

                // Render page-level selection marker before entry content.
                let marker_style = if is_api_selected {
                    Style::default().fg(Color::Yellow).bg(selected_color)
                } else {
                    selected_style
                };
                buf[(content_area.left(), y)]
                    .set_char(if is_api_selected { '▌' } else { ' ' })
                    .set_style(marker_style);

                let display_area = Rect {
                    x: content_area.left().saturating_add(1),
                    y,
                    width: content_area.width.saturating_sub(1),
                    height: 1,
                };

                // Create a new line with white foreground and background
                line.patch_style(selected_style).render(display_area, buf);
            } else {
                // Normal: render with padding on both sides
                // Clear the entire line
                for x in area.left()..area.right() {
                    buf[(x, y)].set_char(' ').set_style(Style::default());
                }

                // Content area (with one space padding on each side)
                let content_area = Rect {
                    x: area.left() + 1,
                    y,
                    width: area.width.saturating_sub(2),
                    height: 1,
                };

                // Render page-level selection marker before entry content.
                buf[(content_area.left(), y)]
                    .set_char(if is_api_selected { '▌' } else { ' ' })
                    .set_style(if is_api_selected {
                        Style::default().fg(Color::Yellow)
                    } else {
                        Style::default()
                    });

                let display_area = Rect {
                    x: content_area.left().saturating_add(1),
                    y,
                    width: content_area.width.saturating_sub(1),
                    height: 1,
                };

                // Render content using Line widget
                line.render(display_area, buf);
            }
        }
    }
}
