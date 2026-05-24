use crate::widgets::{LuaLine, LuaSpan, LuaText};
use anyhow::bail;
use mlua::prelude::*;
use ratatui::{text::Line, widgets};

#[derive(Clone)]
pub struct PageEntry {
    pub key: String,
    pub tbl: LuaTable,
}

impl FromLua for PageEntry {
    fn from_lua(value: mlua::Value, lua: &mlua::Lua) -> mlua::Result<Self> {
        let tbl = LuaTable::from_lua(value, lua)?;
        let key: String = tbl.get("key")?;
        Ok(Self { key, tbl })
    }
}

impl PageEntry {
    pub fn keymap_table(&self) -> LuaResult<Option<LuaTable>> {
        match self.tbl.get::<LuaValue>("keymap")? {
            LuaValue::Nil => Ok(None),
            LuaValue::Table(tbl) => Ok(Some(tbl)),
            other => Err(LuaError::RuntimeError(format!(
                "entry.keymap must be a table, got {}",
                other.type_name()
            ))),
        }
    }

    pub fn selectable(&self) -> LuaResult<bool> {
        match self.tbl.get::<LuaValue>("selectable")? {
            LuaValue::Nil => Ok(true),
            LuaValue::Boolean(value) => Ok(value),
            other => Err(LuaError::RuntimeError(format!(
                "entry.selectable must be a boolean, got {}",
                other.type_name()
            ))),
        }
    }

    fn extract_line_field(&self, field: &str) -> anyhow::Result<Option<Line<'static>>> {
        match self.tbl.get::<LuaValue>(field)? {
            LuaValue::Nil => Ok(None),
            LuaValue::String(s) => Ok(Some(Line::from(s.to_string_lossy().to_string()))),
            LuaValue::UserData(ud) => {
                if let Ok(span) = ud.borrow::<LuaSpan>() {
                    Ok(Some(Line::from(span.0.clone())))
                } else if let Ok(line) = ud.borrow::<LuaLine>() {
                    Ok(Some(line.0.clone()))
                } else {
                    bail!("Expected Span, Line, string, or nil")
                }
            }
            _ => bail!("Expected Span, Line, string, or nil"),
        }
    }

    fn extract_callable_line_field(&self, field: &str) -> anyhow::Result<Option<Line<'static>>> {
        let value = match self.tbl.get::<LuaValue>(field)? {
            LuaValue::Function(f) => f.call(())?,
            other => other,
        };

        match value {
            LuaValue::Nil => Ok(None),
            LuaValue::String(s) => Ok(Some(Line::from(s.to_string_lossy().to_string()))),
            LuaValue::UserData(ud) => {
                if let Ok(span) = ud.borrow::<LuaSpan>() {
                    Ok(Some(Line::from(span.0.clone())))
                } else if let Ok(line) = ud.borrow::<LuaLine>() {
                    Ok(Some(line.0.clone()))
                } else {
                    bail!("Expected Span, Line, string, function, or nil")
                }
            }
            other => bail!(
                "Expected Span, Line, string, function, or nil, got {}",
                other.type_name()
            ),
        }
    }

    /// Extract the Text content from the display field
    pub fn display(&self) -> Line<'static> {
        self.extract_line_field("display")
            .and_then(|line| Ok(line.unwrap_or_else(|| Line::from(self.key.clone()))))
            .unwrap_or_else(|e| Line::from(e.to_string()))
    }

    pub fn bottom_line(&self) -> Option<Line<'static>> {
        self.extract_callable_line_field("bottom_line")
            .unwrap_or_else(|e| Some(Line::from(e.to_string())))
    }
}

#[derive(Default)]
pub struct Page {
    pub list: Vec<PageEntry>,
    pub filtered_list: Vec<PageEntry>,
    pub list_state: widgets::ListState,
    /// List filter string for this page
    pub list_filter: String,
}

impl Page {
    /// Extract display text from a PageEntry
    fn extract_display_text(&self, entry: &PageEntry) -> String {
        match entry.tbl.get::<LuaValue>("display") {
            Ok(LuaValue::Nil) => entry.key.clone(),
            Ok(LuaValue::String(s)) => s.to_string_lossy().to_string(),
            Ok(LuaValue::UserData(ud)) => {
                if let Ok(span) = ud.borrow::<LuaSpan>() {
                    span.0.to_string()
                } else if let Ok(line) = ud.borrow::<LuaLine>() {
                    line.0.to_string()
                } else if let Ok(text) = ud.borrow::<LuaText>() {
                    // Text implements Display, to_string() returns lines joined by '\n'
                    text.0.to_string()
                } else {
                    entry.key.clone()
                }
            }
            _ => entry.key.clone(),
        }
    }

    /// Apply filter to the list, updating filtered_list
    pub fn apply_filter(&mut self) {
        self.filtered_list = if self.list_filter.is_empty() {
            self.list.clone()
        } else {
            let filter_lower = self.list_filter.to_lowercase();
            self.list
                .iter()
                .filter(|entry| {
                    let key_lower = entry.key.to_lowercase();
                    let display_lower = self.extract_display_text(entry).to_lowercase();
                    key_lower.contains(&filter_lower) || display_lower.contains(&filter_lower)
                })
                .cloned()
                .collect()
        };

        // Reset selection to first item or none if empty
        if self.filtered_list.is_empty() {
            self.list_state.select(None);
        } else {
            self.list_state.select(Some(0));
        }
    }
}
