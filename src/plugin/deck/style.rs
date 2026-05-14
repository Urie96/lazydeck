use crate::plugin::deck::highlighter;
use crate::widgets::{LuaLine, LuaSpan, LuaText};
use ansi_to_tui::IntoText;
use mlua::prelude::*;
use ratatui::text::{Line, Span, Text};
use unicode_width::UnicodeWidthStr;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_align_columns() {
        let lua = Lua::new();

        // Create a 1D array of Lines
        let lines = lua.create_table().expect("Failed to create lines table");

        // Line 1: foo, 123, A
        let line1 = lua
            .create_userdata(LuaLine(Line::from(vec![
                Span::raw("foo"),
                Span::raw("123"),
                Span::raw("A"),
            ])))
            .expect("Failed to create line1");
        lines.set(1, line1).expect("Failed to set line 1");

        // Line 2: barbaz, 4, BC
        let line2 = lua
            .create_userdata(LuaLine(Line::from(vec![
                Span::raw("barbaz"),
                Span::raw("4"),
                Span::raw("BC"),
            ])))
            .expect("Failed to create line2");
        lines.set(2, line2).expect("Failed to set line 2");

        // Line 3: qux, 567890, D E F
        let line3 = lua
            .create_userdata(LuaLine(Line::from(vec![
                Span::raw("qux"),
                Span::raw("567890"),
                Span::raw("D E F"),
            ])))
            .expect("Failed to create line3");
        lines.set(3, line3).expect("Failed to set line 3");

        // Call align_columns
        let align_fn = align_columns(&lua).expect("Failed to create align_columns function");
        align_fn
            .call::<()>(lines.clone())
            .expect("Failed to align columns");

        // Check the alignment
        // Column 1: max width = 6 (barbaz is 6 chars)
        let line1: LuaAnyUserData = lines.get(1).expect("Failed to get line 1");
        let borrowed_line1 = line1.borrow::<LuaLine>().expect("Failed to borrow line1");
        assert_eq!(borrowed_line1.0.spans[0].content.as_ref(), "foo   "); // Padded to 6 chars (3 + 3)

        // Column 2: max width = 6 (567890 is 6 chars)
        let line2: LuaAnyUserData = lines.get(2).expect("Failed to get line 2");
        let borrowed_line2 = line2.borrow::<LuaLine>().expect("Failed to borrow line2");
        assert_eq!(borrowed_line2.0.spans[1].content.as_ref(), "4     "); // Padded to 6 chars (1 + 5)

        // Column 3: max width = 5 (D E F is 5 chars with spaces)
        let line3: LuaAnyUserData = lines.get(3).expect("Failed to get line 3");
        let borrowed_line3 = line3.borrow::<LuaLine>().expect("Failed to borrow line3");
        assert_eq!(borrowed_line3.0.spans[2].content.as_ref(), "D E F"); // Already max width (5)
    }

    #[test]
    fn test_align_columns_unicode() {
        let lua = Lua::new();

        // Test with Unicode/Chinese characters to ensure proper character counting
        let lines = lua.create_table().expect("Failed to create lines table");

        // Line 1: 姓名, 年龄, 描述
        let line1 = lua
            .create_userdata(LuaLine(Line::from(vec![
                Span::raw("姓名"),
                Span::raw("年龄"),
                Span::raw("描述"),
            ])))
            .expect("Failed to create line1");
        lines.set(1, line1).expect("Failed to set line 1");

        // Line 2: 张三, 25, 软件工程师
        let line2 = lua
            .create_userdata(LuaLine(Line::from(vec![
                Span::raw("张三"),
                Span::raw("25"),
                Span::raw("软件工程师"),
            ])))
            .expect("Failed to create line2");
        lines.set(2, line2).expect("Failed to set line 2");

        // Line 3: 李四, 30, 系统架构师
        let line3 = lua
            .create_userdata(LuaLine(Line::from(vec![
                Span::raw("李四"),
                Span::raw("30"),
                Span::raw("系统架构师"),
            ])))
            .expect("Failed to create line3");
        lines.set(3, line3).expect("Failed to set line 3");

        // Call align_columns
        let align_fn = align_columns(&lua).expect("Failed to create align_columns function");
        align_fn
            .call::<()>(lines.clone())
            .expect("Failed to align columns");

        // Check the alignment
        // Column 1: max width = 4 (all Chinese chars are 2 display columns each, 2 chars = 4)
        let line1: LuaAnyUserData = lines.get(1).expect("Failed to get line 1");
        let borrowed_line1 = line1.borrow::<LuaLine>().expect("Failed to borrow line1");
        assert_eq!(borrowed_line1.0.spans[0].content.as_ref(), "姓名"); // Already max width (4 display columns)

        // Column 2: max width = 4 ("年龄" is 4 display columns)
        let line2: LuaAnyUserData = lines.get(2).expect("Failed to get line 2");
        let borrowed_line2 = line2.borrow::<LuaLine>().expect("Failed to borrow line2");
        assert_eq!(borrowed_line2.0.spans[1].content.as_ref(), "25  "); // Padded to 4 display columns (2 + 2)

        // Column 3: max width = 10 (5 Chinese chars = 10 display columns)
        let line3: LuaAnyUserData = lines.get(3).expect("Failed to get line 3");
        let borrowed_line3 = line3.borrow::<LuaLine>().expect("Failed to borrow line3");
        assert_eq!(borrowed_line3.0.spans[2].content.as_ref(), "系统架构师"); // Already max width (10 display columns)
    }

    #[test]
    fn test_text_preserves_empty_string_as_blank_line() {
        let lua = Lua::new();
        let text_fn = text(&lua).expect("Failed to create text function");
        let args = lua.create_table().expect("Failed to create args");

        args.set(1, "").expect("Failed to set arg");

        let rendered: LuaAnyUserData = text_fn.call(args).expect("Failed to render text");
        let borrowed = rendered.borrow::<LuaText>().expect("Failed to borrow text");

        assert_eq!(borrowed.0.lines.len(), 1);
        assert_eq!(borrowed.0.lines[0], Line::raw(""));
    }
}

pub fn span(lua: &Lua) -> mlua::Result<LuaFunction> {
    lua.create_function(|_lua, s: String| Ok(LuaSpan(Span::raw(s))))
}

/// Create a Line from a table of Spans or Strings
pub fn line(lua: &Lua) -> mlua::Result<LuaFunction> {
    lua.create_function(|_lua, args: LuaTable| {
        let len = args.raw_len();
        let mut spans = Vec::with_capacity(len);

        for pair in args.pairs::<LuaValue, LuaValue>() {
            let (_, arg) = pair?;
            match arg {
                LuaValue::String(s) => {
                    let content = s.to_str()?.to_string();
                    spans.push(Span::raw(content));
                }
                LuaValue::UserData(ud) => {
                    if let Ok(span) = ud.take::<LuaSpan>() {
                        spans.push(span.0);
                    } else {
                        return Err(LuaError::RuntimeError(
                            "Expected Span or String in table".to_string(),
                        ));
                    }
                }
                _ => {
                    return Err(LuaError::RuntimeError(
                        "Expected Span or String in table".to_string(),
                    ));
                }
            }
        }

        Ok(LuaLine(Line::from(spans)))
    })
}

/// Create a Text from a table of Texts, Lines, Spans, or Strings
pub fn text(lua: &Lua) -> mlua::Result<LuaFunction> {
    lua.create_function(|_lua, args: LuaTable| {
        let len = args.raw_len();
        let mut lines = Vec::with_capacity(len);

        for pair in args.pairs::<LuaValue, LuaValue>() {
            let (_, arg) = pair?;
            match arg {
                LuaValue::String(s) => {
                    let content = s.to_str()?;
                    if content.is_empty() {
                        lines.push(Line::raw(String::new()));
                    } else {
                        // Split string by newlines into multiple lines
                        for line in content.lines() {
                            lines.push(Line::raw(line.to_string()));
                        }
                    }
                }
                LuaValue::UserData(ud) => {
                    if let Ok(text) = ud.take::<LuaText>() {
                        lines.extend(text.0.lines);
                    } else if let Ok(line) = ud.take::<LuaLine>() {
                        lines.push(line.0);
                    } else if let Ok(span) = ud.take::<LuaSpan>() {
                        lines.push(Line::from(span.0));
                    } else {
                        return Err(LuaError::RuntimeError(
                            "Expected Text, Line, Span, or String in table".to_string(),
                        ));
                    }
                }
                _ => {
                    return Err(LuaError::RuntimeError(
                        "Expected Text, Line, Span, or String in table".to_string(),
                    ));
                }
            }
        }
        Ok(LuaText(Text::from(lines)))
    })
}

/// Highlight code with syntax highlighting
pub fn highlight(lua: &Lua) -> mlua::Result<LuaFunction> {
    lua.create_function(|_lua, (code, language): (String, String)| {
        highlighter::highlight(&code, &language)
            .map(|text| LuaText(text))
            .map_err(|e| LuaError::RuntimeError(format!("Highlighting failed: {}", e)))
    })
}

pub fn ansi(lua: &Lua) -> mlua::Result<LuaFunction> {
    lua.create_function(|_lua, s: String| Ok(LuaText(s.as_bytes().into_text().into_lua_err()?)))
}

/// Align columns in a 1D array of Lines, modifying them in place
/// Each Line contains multiple Spans, and the function ensures all cells in the same column have the same width
pub fn align_columns(lua: &Lua) -> mlua::Result<LuaFunction> {
    lua.create_function(|_lua, lines: LuaTable| -> mlua::Result<()> {
        if lines.is_empty() {
            return Ok(());
        }

        // First pass: calculate column widths
        let mut col_widths = Vec::with_capacity(lines.raw_len());

        for pair in lines.pairs::<LuaValue, LuaValue>() {
            let (_, line_value) = pair?;

            let line = match line_value {
                LuaValue::UserData(ud) => match ud.borrow::<LuaLine>() {
                    Ok(l) => l,
                    Err(_) => continue,
                },
                _ => continue,
            };

            // Collect width info for each span in this line
            let mut row_widths = Vec::with_capacity(line.0.spans.len());
            for span in &line.0.spans {
                let cell_width = span.width();
                row_widths.push(cell_width);

                // Update column max width
                let col_idx = row_widths.len() - 1;
                if col_idx >= col_widths.len() {
                    col_widths.push(cell_width);
                } else if cell_width > col_widths[col_idx] {
                    col_widths[col_idx] = cell_width;
                }
            }
        }

        // Second pass: modify Spans in place with aligned content
        for pair in lines.pairs::<LuaValue, LuaValue>() {
            let (_, line_value) = pair?;

            match line_value {
                LuaValue::UserData(ud) => {
                    let mut line = match ud.borrow_mut::<LuaLine>() {
                        Ok(l) => l,
                        Err(_) => continue,
                    };

                    for (col_idx, span) in line.0.spans.iter_mut().enumerate() {
                        if col_idx >= col_widths.len() - 1 {
                            break;
                        }

                        let content = span.content.as_ref();
                        let cell_width = content.width();

                        if cell_width < col_widths[col_idx] {
                            let width = col_widths[col_idx];
                            let padding = width - cell_width;
                            let padded = format!("{}{}", content, " ".repeat(padding));
                            span.content = padded.into();
                        }
                    }
                }
                _ => continue,
            }
        }

        Ok(())
    })
}
