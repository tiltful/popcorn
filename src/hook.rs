use std::sync::{Arc, Mutex, OnceLock};

use rdev::{Event, EventType, Key as RdevKey, listen};

use crate::config::should_expand;
use crate::expander::{InputBuffer, expand, match_snippet};
use crate::focus::get_foreground_process_name;
use crate::hotkeys;
use crate::layout::translate_en_to_ru;
use crate::state::AppState;

// The rdev callback does not know how to capture the environment by value
// as conveniently as a closure with move in a separate thread, so the state is static.
static STATE: OnceLock<Arc<AppState>> = OnceLock::new();
static BUFFER: OnceLock<Mutex<InputBuffer>> = OnceLock::new();

/// Runs the global keyboard listener on a separate thread.
pub fn run_hook_thread(state: Arc<AppState>) {
	STATE.set(state).ok();
	BUFFER.set(Mutex::new(InputBuffer::new())).ok();

	std::thread::spawn(|| {
		if let Err(error) = listen(handle_event) {
			eprintln!("[hook] failed to start global listener: {error:?}");
		}
	});
}

fn handle_event(event: Event) {
	println!("[event] {:?}", event.event_type);

	match event.event_type {
		EventType::KeyPress(key) => {
			hotkeys::update_modifier_state(key, true);

			// If the key combination worked, do not count it
			// as a character for the regular snippets buffer.
			if try_trigger_hotkey(key) {
				return;
			}

			// Keep the snippets buffer in sync when editing text.
			if key == RdevKey::Backspace {
				handle_backspace();
				return;
			}

			// event.name is already translated for the current keyboard layout.
			if let Some(text) = event.name {
				for ch in text.chars() {
					handle_char(ch);
				}
			}
		}
		EventType::KeyRelease(key) => {
			hotkeys::update_modifier_state(key, false);
		}
		_ => {}
	}
}

fn handle_backspace() {
	let Some(state) = STATE.get() else {
		return;
	};

	let Some(buffer_lock) = BUFFER.get() else {
		return;
	};

	if state.is_paused() {
		return;
	}

	let config = state.config.read().unwrap();
	let current_process = get_foreground_process_name();

	if !should_expand(&config, current_process.as_deref()) {
		return;
	}

	let mut buffer = buffer_lock.lock().unwrap();
	buffer.pop();
}

/// Checks whether the current key combination matches a configured hotkey.
fn try_trigger_hotkey(key: RdevKey) -> bool {
	let Some(state) = STATE.get() else {
		return false;
	};

	if state.is_paused() {
		return false;
	}

	let config = state.config.read().unwrap();
	let current_process = get_foreground_process_name();

	if !should_expand(&config, current_process.as_deref()) {
		return false;
	}

	match hotkeys::find_match(key, &config.keybinds) {
		Some(text) => {
			let text = text.to_string();
			let insertion_mode = config.general.insertion_mode;

			println!(
				"[hook] hotkey triggered (process: {current_process:?}, mode: {insertion_mode:?})"
			);

			drop(config);

			expand(insertion_mode, 0, &text);
			true
		}
		None => false,
	}
}

fn handle_char(ch: char) {
	let Some(state) = STATE.get() else {
		return;
	};

	let Some(buffer_lock) = BUFFER.get() else {
		return;
	};

	if state.is_paused() {
		return;
	}

	let config = state.config.read().unwrap();
	let current_process = get_foreground_process_name();

	if !should_expand(&config, current_process.as_deref()) {
		return;
	}

	let mut buffer = buffer_lock.lock().unwrap();
	buffer.push(ch);

	// Snippets are matched only when the user presses Space.
	if ch != ' ' {
		return;
	}

	let text = buffer.as_string();

	// Remove the just-entered space before matching.
	let prefix = &text[..text.len() - ch.len_utf8()];

	// First try the original text, then optionally try EN -> RU translation.
	let translated;
	let matched = match match_snippet(prefix, &config.snippets) {
		Some(found) => Some(found),
		None if config.general.translate_layout => {
			translated = translate_en_to_ru(prefix);
			match_snippet(&translated, &config.snippets)
		}
		None => None,
	};

	if let Some((key, replacement)) = matched {
		let key = key.to_string();
		let replacement = replacement.to_string();
		let insertion_mode = config.general.insertion_mode;

		println!(
			"[hook] snippet triggered {key:?} \
             (process: {current_process:?}, mode: {insertion_mode:?})"
		);

		// Erase the abbreviation and the triggering space.
		let backspaces = key.chars().count() + 1;

		// Preserve the original behavior: append a space unless
		// the replacement already ends with whitespace.
		let text_to_insert = if replacement.ends_with(char::is_whitespace) {
			replacement
		} else {
			format!("{replacement} ")
		};

		println!(
			"[debug] snippet={key:?}, replacement={text_to_insert:?}, backspaces={backspaces}"
		);

		buffer.clear();

		drop(buffer);
		drop(config);

		std::thread::spawn(move || {
			// Let the physical spacebar event complete first.
			std::thread::sleep(std::time::Duration::from_millis(30));

			expand(insertion_mode, backspaces, &text_to_insert);
		});
	}
}
