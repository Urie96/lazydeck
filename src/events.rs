use mlua::prelude::*;
use ratatui::text::Text;
use std::{hash::Hash, pin::Pin, time::Duration};

use crossterm::event::{Event as CrosstermEvent, *};
use futures::{Stream, StreamExt};
use tokio::{sync::mpsc, time::interval};
use tokio_stream::{wrappers::IntervalStream, StreamMap};

use crate::Keymap;

const RENDER_INTERVAL: Duration = Duration::from_secs(20);

pub struct Events {
    tx: EventSender,
    streams: StreamMap<StreamName, Pin<Box<dyn Stream<Item = Event>>>>,
}

pub type EventSender = mpsc::UnboundedSender<Event>;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
enum StreamName {
    Render,
    Crossterm,
    Text,
}

pub enum Event {
    Quit,
    Render,
    RefreshPreview,
    Enter(Vec<String>),
    Command(String),
    Crossterm(CrosstermEvent),
    AddKeymap(Keymap),
    LuaCallback(Box<dyn FnOnce(&Lua) -> mlua::Result<()>>),
    InteractiveCommand {
        cmd: Vec<String>,
        on_complete: Option<LuaFunction>,
        wait_confirm: Option<LuaFunction>,
    },
    Notify(Text<'static>),
    ShowConfirm {
        title: Option<String>,
        prompt: String,
        on_confirm: LuaFunction,
        on_cancel: Option<LuaFunction>,
    },
    ShowSelect {
        prompt: Option<String>,
        options: Vec<crate::SelectOption>,
        on_selection: LuaFunction,
    },
    ShowInput {
        prompt: String,
        placeholder: String,
        value: String,
        /// Whether the input should be masked (e.g. password entry)
        secret: bool,
        on_submit: LuaFunction,
        on_cancel: LuaFunction,
        on_change: LuaFunction,
    },
}

impl Events {
    pub fn new() -> Self {
        let (tx, mut rx) = mpsc::unbounded_channel();

        let stream = async_stream::stream! {
            while let Some(item) = rx.recv().await {
                yield item;
            }
        };
        let stream = Box::pin(stream);

        Self {
            tx,
            streams: StreamMap::from_iter([
                (StreamName::Render, render_stream()),
                (StreamName::Crossterm, crossterm_stream()),
                (StreamName::Text, stream),
            ]),
        }
    }

    pub fn send(&self, event: Event) {
        self.tx.send(event).unwrap()
    }

    pub fn sender(&self) -> EventSender {
        self.tx.clone()
    }

    pub async fn next(&mut self) -> Option<Event> {
        self.streams.next().await.map(|(_name, event)| event)
    }
}

impl Default for Events {
    fn default() -> Self {
        Self::new()
    }
}

fn render_stream() -> Pin<Box<dyn Stream<Item = Event>>> {
    let render_interval = interval(RENDER_INTERVAL);
    Box::pin(IntervalStream::new(render_interval).map(|_| Event::Render))
}

fn crossterm_stream() -> Pin<Box<dyn Stream<Item = Event>>> {
    Box::pin(EventStream::new().fuse().filter_map(|event| async move {
        match event {
            // Ignore key release / repeat events
            Ok(CrosstermEvent::Key(key)) if key.kind == KeyEventKind::Release => None,
            Ok(event) => Some(Event::Crossterm(event)),
            Err(e) => panic!("{}", e),
        }
    }))
}
