use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use std::collections::{HashMap, VecDeque};

use crate::clipboard;
use crate::config::InsertionMode;

const MAX_BUFFER: usize = 32;

/// A sliding buffer of the last printed characters.
pub struct InputBuffer {
	buf: VecDeque<char>,
}

impl InputBuffer {
	pub fn new() -> Self {
		Self {
			buf: VecDeque::with_capacity(MAX_BUFFER),
		}
	}

	pub fn push(&mut self, c: char) {
		self.buf.push_back(c);

		while self.buf.len() > MAX_BUFFER {
			self.buf.pop_front();
		}
	}

	pub fn clear(&mut self) {
		self.buf.clear();
	}

	pub fn as_string(&self) -> String {
		self.buf.iter().collect()
	}
}

/// Looks for the longest snippet key that ends the buffer.
pub fn match_snippet<'a>(
	buffer: &str,
	snippets: &'a HashMap<String, String>,
) -> Option<(&'a str, &'a str)> {
	snippets
		.iter()
		.filter(|(key, _)| !key.is_empty() && buffer.ends_with(key.as_str()))
		.max_by_key(|(key, _)| key.len())
		.map(|(key, value)| (key.as_str(), value.as_str()))
}

/// Expands text using the selected insertion strategy.
///
/// `backspaces` specifies how many characters should be erased
/// before inserting the replacement.
pub fn expand(mode: InsertionMode, backspaces: usize, text: &str) {
	match mode {
		InsertionMode::Type => type_text(backspaces, text),
		InsertionMode::Paste => paste_text(backspaces, text),
	}
}

/// Erases the specified number of characters and types the text using Enigo.
fn type_text(backspaces: usize, text: &str) {
	let mut enigo = match Enigo::new(&Settings::default()) {
		Ok(enigo) => enigo,
		Err(error) => {
			eprintln!("[expander] failed to initialize enigo: {error}");
			return;
		}
	};

	for _ in 0..backspaces {
		if let Err(error) = enigo.key(Key::Backspace, Direction::Click) {
			eprintln!("[expander] failed to press Backspace: {error}");
			return;
		}
	}

	if let Err(error) = enigo.text(text) {
		eprintln!("[expander] failed to type text: {error}");
	}
}

/// Erases the specified number of characters and pastes the text using
/// the system clipboard.
fn paste_text(backspaces: usize, text: &str) {
	if backspaces > 0 {
		let mut enigo = match Enigo::new(&Settings::default()) {
			Ok(enigo) => enigo,
			Err(error) => {
				eprintln!("[expander] failed to initialize enigo: {error}");
				return;
			}
		};

		for _ in 0..backspaces {
			if let Err(error) = enigo.key(Key::Backspace, Direction::Click) {
				eprintln!("[expander] failed to press Backspace: {error}");
				return;
			}
		}
	}

	if let Err(error) = clipboard::set_text(text) {
		eprintln!("[expander] failed to set clipboard text: {error:#}");
		return;
	}

	if let Err(error) = clipboard::paste() {
		eprintln!("[expander] failed to paste clipboard text: {error:#}");
	}
}
