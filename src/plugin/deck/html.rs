use mlua::prelude::*;
use scraper::{ElementRef, Html, Selector};
use std::sync::Arc;

#[derive(Clone)]
struct LuaHtmlDocument {
    inner: Arc<HtmlDocumentInner>,
}

struct HtmlDocumentInner {
    raw_html: String,
    document: Html,
}

#[derive(Clone)]
struct LuaHtmlNode {
    tag_name: String,
    html: String,
    inner_html: String,
    text: String,
    attrs: Vec<(String, String)>,
}

#[derive(Clone)]
struct LuaHtmlNodeList {
    nodes: Vec<LuaHtmlNode>,
}

impl LuaHtmlDocument {
    fn parse_document(html: String) -> Self {
        Self {
            inner: Arc::new(HtmlDocumentInner {
                document: Html::parse_document(&html),
                raw_html: html,
            }),
        }
    }

    fn parse_fragment(html: String) -> Self {
        Self {
            inner: Arc::new(HtmlDocumentInner {
                document: Html::parse_fragment(&html),
                raw_html: html,
            }),
        }
    }

    fn select(&self, selector: &str) -> mlua::Result<LuaHtmlNodeList> {
        let selector = parse_selector(selector)?;
        Ok(LuaHtmlNodeList {
            nodes: self
                .inner
                .document
                .select(&selector)
                .map(snapshot_node)
                .collect(),
        })
    }

    fn first(&self, selector: &str) -> mlua::Result<Option<LuaHtmlNode>> {
        Ok(self.select(selector)?.nodes.into_iter().next())
    }
}

impl LuaHtmlNode {
    fn select(&self, selector: &str) -> mlua::Result<LuaHtmlNodeList> {
        let selector = parse_selector(selector)?;
        let fragment = Html::parse_fragment(&self.html);
        Ok(LuaHtmlNodeList {
            nodes: fragment.select(&selector).map(snapshot_node).collect(),
        })
    }

    fn first(&self, selector: &str) -> mlua::Result<Option<LuaHtmlNode>> {
        Ok(self.select(selector)?.nodes.into_iter().next())
    }

    fn attr(&self, name: &str) -> Option<String> {
        self.attrs
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.clone())
    }
}

impl LuaHtmlNodeList {
    fn get(&self, index: usize) -> Option<LuaHtmlNode> {
        self.nodes.get(index).cloned()
    }
}

impl LuaUserData for LuaHtmlDocument {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("select", |lua, this, selector: String| {
            lua.create_userdata(this.select(&selector)?)
        });

        methods.add_method("first", |lua, this, selector: String| {
            match this.first(&selector)? {
                Some(node) => Ok(LuaValue::UserData(lua.create_userdata(node)?)),
                None => Ok(LuaValue::Nil),
            }
        });

        methods.add_method("html", |_, this, ()| Ok(this.inner.raw_html.clone()));

        methods.add_meta_method(LuaMetaMethod::ToString, |_, this, ()| {
            Ok(this.inner.raw_html.clone())
        });
    }
}

impl LuaUserData for LuaHtmlNode {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("name", |_, this, ()| Ok(this.tag_name.clone()));
        methods.add_method("html", |_, this, ()| Ok(this.html.clone()));
        methods.add_method("inner_html", |_, this, ()| Ok(this.inner_html.clone()));
        methods.add_method("text", |_, this, ()| Ok(this.text.clone()));
        methods.add_method("attr", |_, this, name: String| Ok(this.attr(&name)));
        methods.add_method("attrs", |lua, this, ()| attrs_to_lua(lua, &this.attrs));

        methods.add_method("select", |lua, this, selector: String| {
            lua.create_userdata(this.select(&selector)?)
        });

        methods.add_method("first", |lua, this, selector: String| {
            match this.first(&selector)? {
                Some(node) => Ok(LuaValue::UserData(lua.create_userdata(node)?)),
                None => Ok(LuaValue::Nil),
            }
        });

        methods.add_meta_method(LuaMetaMethod::ToString, |_, this, ()| Ok(this.html.clone()));
    }
}

impl LuaUserData for LuaHtmlNodeList {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("len", |_, this, ()| Ok(this.nodes.len()));

        methods.add_method("get", |lua, this, index: usize| {
            match index.checked_sub(1) {
                Some(idx) => match this.get(idx) {
                    Some(node) => Ok(LuaValue::UserData(lua.create_userdata(node)?)),
                    None => Ok(LuaValue::Nil),
                },
                None => Ok(LuaValue::Nil),
            }
        });

        methods.add_method("to_table", |lua, this, ()| {
            let table = lua.create_table()?;
            for (index, node) in this.nodes.iter().cloned().enumerate() {
                table.set(index + 1, lua.create_userdata(node)?)?;
            }
            Ok(table)
        });

        methods.add_meta_method(LuaMetaMethod::Len, |_, this, ()| Ok(this.nodes.len()));
    }
}

fn parse_selector(selector: &str) -> mlua::Result<Selector> {
    Selector::parse(selector).map_err(|err| {
        LuaError::RuntimeError(format!("Invalid CSS selector '{}': {}", selector, err))
    })
}

fn attrs_to_lua(lua: &Lua, attrs: &[(String, String)]) -> mlua::Result<LuaTable> {
    let table = lua.create_table()?;
    for (name, value) in attrs {
        table.set(name.as_str(), value.as_str())?;
    }
    Ok(table)
}

fn snapshot_node(element: ElementRef<'_>) -> LuaHtmlNode {
    let attrs = element
        .value()
        .attrs()
        .map(|(name, value)| (name.to_string(), value.to_string()))
        .collect();

    LuaHtmlNode {
        tag_name: element.value().name().to_string(),
        html: element.html(),
        inner_html: element.inner_html(),
        text: element.text().collect::<Vec<_>>().join(""),
        attrs,
    }
}

pub(super) fn new_table(lua: &Lua) -> mlua::Result<LuaTable> {
    lua.create_table_from([
        (
            "parse",
            lua.create_function(|lua, html: String| {
                lua.create_userdata(LuaHtmlDocument::parse_document(html))
            })?,
        ),
        (
            "parse_fragment",
            lua.create_function(|lua, html: String| {
                lua.create_userdata(LuaHtmlDocument::parse_fragment(html))
            })?,
        ),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn document_and_node_selection_work() -> mlua::Result<()> {
        let html = LuaHtmlDocument::parse_document(
            r#"
                <div class="repo">
                    <a href="/rust-lang/rust">rust-lang/rust</a>
                    <span class="stars">100k</span>
                </div>
            "#
            .to_string(),
        );

        let repos = html.select(".repo")?;
        assert_eq!(repos.nodes.len(), 1);

        let repo = repos.nodes[0].clone();
        assert_eq!(repo.tag_name, "div");
        assert_eq!(repo.attr("class").as_deref(), Some("repo"));

        let link = repo.first("a")?.expect("expected link node");
        assert_eq!(link.text, "rust-lang/rust");
        assert_eq!(link.attr("href").as_deref(), Some("/rust-lang/rust"));

        Ok(())
    }
}
