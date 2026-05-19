use crate::{path_codec, State};
use ratatui::{prelude::*, widgets::*};
use unicode_width::UnicodeWidthStr;

pub struct HeaderWidget;

fn format_path(path: &[String]) -> String {
    if path.is_empty() {
        "/".to_string()
    } else {
        format!(
            "/{}",
            path.iter()
                .map(|segment| path_codec::encode_path_segment_for_display(segment))
                .collect::<Vec<_>>()
                .join("/")
        )
    }
}

impl StatefulWidget for HeaderWidget {
    type State = State;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let path_str = format_path(&state.current_path);

        // Get filter from current page
        let filter = state
            .current_page
            .as_ref()
            .map(|p| p.list_filter.as_str())
            .unwrap_or("");

        let mut left_spans = vec![Span::styled(path_str, Style::default().fg(Color::Cyan))];
        if !filter.is_empty() {
            left_spans.push(Span::styled(
                format!(" [filter: {}]", filter),
                Style::default().fg(Color::Yellow),
            ));
        }
        Paragraph::new(Text::from(Line::from(left_spans))).render(area, buf);

        let mut tab_spans = Vec::new();
        for (idx, (_id, title, path)) in state.tab_infos().into_iter().enumerate() {
            if idx > 0 {
                tab_spans.push(Span::raw(" "));
            }
            let label = title.unwrap_or_else(|| format_path(&path));
            let label = if label == "/" {
                label
            } else {
                label.trim_start_matches('/').to_string()
            };
            let is_active = idx == state.active_tab_index();
            let color = if is_active {
                Color::Cyan
            } else {
                Color::DarkGray
            };
            let text_style = if is_active {
                Style::default().fg(Color::Black).bg(color).bold()
            } else {
                Style::default().fg(Color::White).bg(color)
            };

            tab_spans.push(Span::styled("", Style::default().fg(color)));
            tab_spans.push(Span::styled(format!("{}:{}", idx + 1, label), text_style));
            tab_spans.push(Span::styled("", Style::default().fg(color)));
        }

        let tab_line = Line::from(tab_spans);
        let tab_width = UnicodeWidthStr::width(tab_line.to_string().as_str()) as u16;
        if tab_width == 0 || tab_width > area.width {
            return;
        }

        let tab_area = Rect {
            x: area.x + area.width.saturating_sub(tab_width),
            y: area.y,
            width: tab_width,
            height: area.height,
        };
        Paragraph::new(Text::from(tab_line)).render(tab_area, buf);
    }
}
