use std::sync::mpsc::{Receiver, Sender, channel};

use alacritty_terminal::Term;
use alacritty_terminal::event::{Event, EventListener, WindowSize};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::term::{Config, TermMode};
use alacritty_terminal::vte::ansi::{Processor, Rgb};

use crate::Size;

pub(crate) struct Listener(Sender<Event>);

impl EventListener for Listener {
    fn send_event(&self, event: Event) {
        let _ = self.0.send(event);
    }
}

struct Dim(Size);

impl Dimensions for Dim {
    fn total_lines(&self) -> usize {
        usize::from(self.0.rows())
    }
    fn screen_lines(&self) -> usize {
        usize::from(self.0.rows())
    }
    fn columns(&self) -> usize {
        usize::from(self.0.cols())
    }
}

#[derive(Default)]
pub(crate) struct Advance {
    pub replies: Vec<u8>,
    pub bell: bool,
}

pub(crate) struct Emu {
    pub(crate) term: Term<Listener>,
    parser: Processor,
    events: Receiver<Event>,
    pub(crate) size: Size,
    pub(crate) title: Option<String>,
}

impl Emu {
    pub(crate) fn new(size: Size, scrollback: usize) -> Self {
        let (tx, events) = channel();
        let config = Config {
            scrolling_history: scrollback,
            ..Config::default()
        };
        Self {
            term: Term::new(config, &Dim(size), Listener(tx)),
            parser: Processor::new(),
            events,
            size,
            title: None,
        }
    }

    pub(crate) fn resize(&mut self, size: Size) {
        if size != self.size {
            self.term.resize(Dim(size));
            self.size = size;
        }
    }

    pub(crate) fn mode(&self) -> TermMode {
        *self.term.mode()
    }

    pub(crate) fn advance(&mut self, bytes: &[u8]) -> Advance {
        self.parser.advance(&mut self.term, bytes);
        let mut out = Advance::default();
        while let Ok(event) = self.events.try_recv() {
            match event {
                Event::Title(t) => self.title = Some(t),
                Event::ResetTitle => self.title = None,
                Event::Bell => out.bell = true,
                Event::PtyWrite(text) => out.replies.extend_from_slice(text.as_bytes()),
                Event::ColorRequest(index, format) => {
                    let rgb = self.term.colors()[index].unwrap_or_else(|| default_color(index));
                    out.replies.extend_from_slice(format(rgb).as_bytes());
                }
                Event::TextAreaSizeRequest(format) => {
                    let size = WindowSize {
                        num_lines: self.size.rows(),
                        num_cols: self.size.cols(),
                        cell_width: 1,
                        cell_height: 1,
                    };
                    out.replies.extend_from_slice(format(size).as_bytes());
                }
                _ => {}
            }
        }
        out
    }
}

fn default_color(index: usize) -> Rgb {
    match index {
        257 => Rgb { r: 0, g: 0, b: 0 },
        _ => Rgb {
            r: 0xdd,
            g: 0xdd,
            b: 0xdd,
        },
    }
}
