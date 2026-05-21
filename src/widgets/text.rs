use mlua::prelude::*;
use ratatui::style::{Color, Modifier};
use ratatui::text::{Line, Span, Text};
use std::str::FromStr;

type AnyUserData = LuaAnyUserData;

fn span_plain_text(span: &Span<'static>) -> String {
    span.content.to_string()
}

fn line_plain_text(line: &Line<'static>) -> String {
    line.spans
        .iter()
        .map(span_plain_text)
        .collect::<Vec<_>>()
        .join("")
}

fn text_plain_text(text: &Text<'static>) -> String {
    text.lines
        .iter()
        .map(line_plain_text)
        .collect::<Vec<_>>()
        .join("\n")
}

#[derive(Clone)]
pub struct LuaText(pub Text<'static>);

#[derive(Clone)]
pub struct LuaLine(pub Line<'static>);

#[derive(Clone)]
pub struct LuaSpan(pub Span<'static>);

fn into_lines(value: LuaValue) -> mlua::Result<Vec<Line<'static>>> {
    match value {
        LuaValue::String(s) => Ok(vec![Line::raw(s.to_str()?.to_string())]),
        LuaValue::UserData(ud) => {
            if let Ok(line) = ud.borrow::<LuaLine>() {
                Ok(vec![line.0.clone()])
            } else if let Ok(span) = ud.borrow::<LuaSpan>() {
                Ok(vec![Line::from(span.0.clone())])
            } else if let Ok(text) = ud.borrow::<LuaText>() {
                Ok(text.0.lines.clone())
            } else {
                Err(mlua::Error::runtime(
                    "expected Text, Line, Span, or String for append",
                ))
            }
        }
        _ => Err(mlua::Error::runtime(
            "expected Text, Line, Span, or String for append",
        )),
    }
}

impl LuaUserData for LuaSpan {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_meta_method(mlua::MetaMethod::ToString, |_lua, this, ()| {
            Ok(span_plain_text(&this.0))
        });

        methods.add_function_mut("fg", |lua, (ud, color): (AnyUserData, String)| {
            let color = Color::from_str(&color).into_lua_err()?;
            ud.borrow_mut::<Self>()?.0.style.fg = Some(color);
            ud.into_lua(lua)
        });

        methods.add_function_mut("bg", |lua, (ud, color): (AnyUserData, String)| {
            let color = Color::from_str(&color).into_lua_err()?;
            ud.borrow_mut::<Self>()?.0.style.bg = Some(color);
            ud.into_lua(lua)
        });

        methods.add_function_mut("bold", |lua, ud: AnyUserData| {
            ud.borrow_mut::<Self>()?
                .0
                .style
                .add_modifier
                .insert(Modifier::BOLD);
            ud.into_lua(lua)
        });

        methods.add_function_mut("italic", |lua, ud: AnyUserData| {
            ud.borrow_mut::<Self>()?
                .0
                .style
                .add_modifier
                .insert(Modifier::ITALIC);
            ud.into_lua(lua)
        });

        methods.add_function_mut("underline", |lua, ud: AnyUserData| {
            ud.borrow_mut::<Self>()?
                .0
                .style
                .add_modifier
                .insert(Modifier::UNDERLINED);
            ud.into_lua(lua)
        });

        methods.add_meta_function_mut(
            mlua::MetaMethod::Concat,
            |lua, (lhs, rhs): (LuaSpan, LuaValue)| match rhs {
                LuaValue::String(s) => lua.create_userdata(LuaLine(Line::from(vec![
                    lhs.0,
                    Span::raw(s.to_str()?.to_string()),
                ]))),
                LuaValue::UserData(ud) => {
                    if let Ok(span_rhs) = ud.borrow::<LuaSpan>() {
                        lua.create_userdata(LuaLine(Line::from(vec![lhs.0, span_rhs.0.clone()])))
                    } else if let Ok(line_rhs) = ud.borrow::<LuaLine>() {
                        let mut spans = vec![lhs.0];
                        spans.extend(line_rhs.0.spans.clone());
                        lua.create_userdata(LuaLine(Line::from(spans)))
                    } else {
                        Err(mlua::Error::runtime("cannot concat Span with this type"))
                    }
                }
                _ => Err(mlua::Error::runtime(
                    "cannot concat Span with non-string/non-UserData value",
                )),
            },
        );
    }
}

impl LuaUserData for LuaLine {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_meta_method(mlua::MetaMethod::ToString, |_lua, this, ()| {
            Ok(line_plain_text(&this.0))
        });

        methods.add_function_mut("fg", |lua, (ud, color): (AnyUserData, String)| {
            let color = Color::from_str(&color).into_lua_err()?;
            ud.borrow_mut::<Self>()?.0.style.fg = Some(color);
            ud.into_lua(lua)
        });

        methods.add_function_mut("bg", |lua, (ud, color): (AnyUserData, String)| {
            let color = Color::from_str(&color).into_lua_err()?;
            ud.borrow_mut::<Self>()?.0.style.bg = Some(color);
            ud.into_lua(lua)
        });

        methods.add_function_mut("bold", |lua, ud: AnyUserData| {
            ud.borrow_mut::<Self>()?
                .0
                .style
                .add_modifier
                .insert(Modifier::BOLD);
            ud.into_lua(lua)
        });

        methods.add_function_mut("italic", |lua, ud: AnyUserData| {
            ud.borrow_mut::<Self>()?
                .0
                .style
                .add_modifier
                .insert(Modifier::ITALIC);
            ud.into_lua(lua)
        });

        methods.add_function_mut("underline", |lua, ud: AnyUserData| {
            ud.borrow_mut::<Self>()?
                .0
                .style
                .add_modifier
                .insert(Modifier::UNDERLINED);
            ud.into_lua(lua)
        });

        methods.add_meta_function_mut(
            mlua::MetaMethod::Concat,
            |lua, (lhs, rhs): (AnyUserData, LuaValue)| {
                let mut line_lhs = lhs.borrow_mut::<Self>()?;

                match rhs {
                    LuaValue::String(s) => {
                        line_lhs.0.push_span(Span::raw(s.to_str()?.to_string()));
                        lhs.into_lua(lua)
                    }
                    LuaValue::UserData(ud) => {
                        // 尝试转换为 Span
                        if let Ok(span_rhs) = ud.borrow::<LuaSpan>() {
                            line_lhs.0.push_span(span_rhs.0.clone());
                            lhs.into_lua(lua)
                        } else {
                            Err(mlua::Error::runtime("cannot concat Line with this type"))
                        }
                    }
                    _ => Err(mlua::Error::runtime(
                        "cannot concat Line with non-string/non-UserData value",
                    )),
                }
            },
        );
    }
}

impl LuaUserData for LuaText {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_meta_method(mlua::MetaMethod::ToString, |_lua, this, ()| {
            Ok(text_plain_text(&this.0))
        });

        methods.add_method_mut("append", |_lua, this, value: LuaValue| {
            this.0.lines.extend(into_lines(value)?);
            Ok(())
        });
    }
}

impl FromLua for LuaText {
    fn from_lua(value: LuaValue, _lua: &Lua) -> mlua::Result<Self> {
        match value {
            LuaValue::UserData(ud) => {
                // Try LuaText first
                if let Ok(text) = ud.borrow::<LuaText>() {
                    return Ok(text.clone());
                }
                // Try LuaLine (convert single Line to Text)
                if let Ok(line) = ud.borrow::<LuaLine>() {
                    return Ok(LuaText(Text::from(vec![line.0.clone()])));
                }
                // Try LuaSpan (convert single Span to Text via Line)
                if let Ok(span) = ud.borrow::<LuaSpan>() {
                    return Ok(LuaText(Text::from(Line::from(span.0.clone()))));
                }
                Err(mlua::Error::FromLuaConversionError {
                    from: "UserData",
                    to: "LuaText".to_string(),
                    message: Some("UserData is not a LuaText, LuaLine, or LuaSpan".to_string()),
                })
            }
            LuaValue::String(s) => {
                let s = s.to_str()?.to_string();
                Ok(LuaText(Text::raw(s)))
            }
            _ => Err(mlua::Error::FromLuaConversionError {
                from: value.type_name(),
                to: "LuaText".to_string(),
                message: Some("expected UserData, String".to_string()),
            }),
        }
    }
}

impl FromLua for LuaSpan {
    fn from_lua(value: LuaValue, _lua: &Lua) -> mlua::Result<Self> {
        match value {
            LuaValue::UserData(ud) => {
                ud.borrow::<LuaSpan>()
                    .map(|span| span.clone())
                    .map_err(|_| mlua::Error::FromLuaConversionError {
                        from: "UserData",
                        to: "LuaSpan".to_string(),
                        message: Some("UserData is not a LuaSpan".to_string()),
                    })
            }
            LuaValue::String(s) => {
                let s = s.to_str()?.to_string();
                Ok(LuaSpan(Span::raw(s)))
            }
            _ => Err(mlua::Error::FromLuaConversionError {
                from: value.type_name(),
                to: "LuaSpan".to_string(),
                message: Some("expected UserData or String".to_string()),
            }),
        }
    }
}

impl FromLua for LuaLine {
    fn from_lua(value: LuaValue, _lua: &Lua) -> mlua::Result<Self> {
        match value {
            LuaValue::UserData(ud) => {
                if let Ok(line) = ud.borrow::<LuaLine>() {
                    Ok(line.clone())
                } else if let Ok(span) = ud.borrow::<LuaSpan>() {
                    Ok(LuaLine(Line::from(span.0.clone())))
                } else {
                    Err(mlua::Error::FromLuaConversionError {
                        from: "UserData",
                        to: "LuaLine".to_string(),
                        message: Some("UserData is not a LuaLine or LuaSpan".to_string()),
                    })
                }
            }
            LuaValue::String(s) => {
                let s = s.to_str()?.to_string();
                Ok(LuaLine(Line::raw(s)))
            }
            _ => Err(mlua::Error::FromLuaConversionError {
                from: value.type_name(),
                to: "LuaLine".to_string(),
                message: Some("expected UserData or String".to_string()),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lua_span_style_methods_apply_modifiers() {
        let lua = Lua::new();
        let span = lua
            .create_userdata(LuaSpan(Span::raw("repo")))
            .expect("create span userdata");
        lua.globals().set("span", span).expect("set span global");

        let styled: LuaAnyUserData = lua
            .load("return span:bold():italic():underline()")
            .eval()
            .expect("style span in lua");
        let span = styled.borrow::<LuaSpan>().expect("borrow styled span");

        assert!(span.0.style.add_modifier.contains(Modifier::BOLD));
        assert!(span.0.style.add_modifier.contains(Modifier::ITALIC));
        assert!(span.0.style.add_modifier.contains(Modifier::UNDERLINED));
    }

    #[test]
    fn lua_line_style_methods_apply_modifiers() {
        let lua = Lua::new();
        let line = lua
            .create_userdata(LuaLine(Line::raw("repo")))
            .expect("create line userdata");
        lua.globals().set("line", line).expect("set line global");

        let styled: LuaAnyUserData = lua
            .load("return line:bold():italic():underline()")
            .eval()
            .expect("style line in lua");
        let line = styled.borrow::<LuaLine>().expect("borrow styled line");

        assert!(line.0.style.add_modifier.contains(Modifier::BOLD));
        assert!(line.0.style.add_modifier.contains(Modifier::ITALIC));
        assert!(line.0.style.add_modifier.contains(Modifier::UNDERLINED));
    }

    #[test]
    fn lua_text_widgets_tostring_return_plain_text() {
        let lua = Lua::new();
        let span = lua
            .create_userdata(LuaSpan(Span::raw("hello")))
            .expect("create span userdata");
        let line = lua
            .create_userdata(LuaLine(Line::from(vec![Span::raw("foo"), Span::raw("bar")])))
            .expect("create line userdata");
        let text = lua
            .create_userdata(LuaText(Text::from(vec![
                Line::raw("line1"),
                Line::raw("line2"),
            ])))
            .expect("create text userdata");
        lua.globals().set("span", span).expect("set span global");
        lua.globals().set("line", line).expect("set line global");
        lua.globals().set("text", text).expect("set text global");

        let plain: (String, String, String) = lua
            .load("return tostring(span), tostring(line), tostring(text)")
            .eval()
            .expect("tostring widgets in lua");

        assert_eq!(plain.0, "hello");
        assert_eq!(plain.1, "foobar");
        assert_eq!(plain.2, "line1\nline2");
    }

    #[test]
    fn lua_text_append_accepts_text() {
        let lua = Lua::new();
        let text = lua
            .create_userdata(LuaText(Text::from(vec![Line::raw("head")])))
            .expect("create text userdata");
        let extra = lua
            .create_userdata(LuaText(Text::from(vec![
                Line::raw("tail-1"),
                Line::raw("tail-2"),
            ])))
            .expect("create extra text userdata");
        lua.globals().set("text", text).expect("set text global");
        lua.globals().set("extra", extra).expect("set extra global");

        let appended: LuaAnyUserData = lua
            .load(
                r#"
                text:append(extra)
                return text
            "#,
            )
            .eval()
            .expect("append text in lua");
        let text = appended.borrow::<LuaText>().expect("borrow appended text");

        assert_eq!(text.0.lines.len(), 3);
        assert_eq!(text.0.lines[0], Line::raw("head"));
        assert_eq!(text.0.lines[1], Line::raw("tail-1"));
        assert_eq!(text.0.lines[2], Line::raw("tail-2"));
    }
}
