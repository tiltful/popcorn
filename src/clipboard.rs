use anyhow::{Context, Result, bail};
use std::mem::size_of;
use std::ptr::null_mut;
use windows::Win32::Foundation::{HANDLE, HGLOBAL, HWND};
use windows::Win32::System::DataExchange::{
	CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::Memory::{
	GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
	INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput, VIRTUAL_KEY,
};

const CF_UNICODETEXT: u32 = 13;
const VK_CONTROL: VIRTUAL_KEY = VIRTUAL_KEY(0x11);
const VK_V: VIRTUAL_KEY = VIRTUAL_KEY(0x56);

pub fn get_text() -> Result<Option<String>> {
	unsafe {
		OpenClipboard(Option::from(HWND(null_mut()))).context("Failed to open clipboard")?;

		let result = (|| -> Result<Option<String>> {
			let handle = match GetClipboardData(CF_UNICODETEXT) {
				Ok(handle) => handle,
				Err(_) => return Ok(None),
			};

			if handle.0.is_null() {
				return Ok(None);
			}

			let memory = HGLOBAL(handle.0);
			let size = GlobalSize(memory);

			if size < size_of::<u16>() {
				return Ok(None);
			}

			let pointer = GlobalLock(memory);
			if pointer.is_null() {
				bail!("Failed to lock clipboard memory");
			}

			let units = size / size_of::<u16>();
			let utf16 = std::slice::from_raw_parts(pointer.cast::<u16>(), units);

			let length = utf16.iter().position(|&unit| unit == 0).unwrap_or(units);

			let text =
				String::from_utf16(&utf16[..length]).context("Failed to decode clipboard text");

			let _ = GlobalUnlock(memory);

			text.map(Some)
		})();

		let _ = CloseClipboard();
		result
	}
}

pub fn set_text(text: &str) -> Result<()> {
	let mut utf16: Vec<u16> = text.encode_utf16().collect();
	utf16.push(0);

	let byte_len = utf16.len() * size_of::<u16>();

	unsafe {
		OpenClipboard(Option::from(HWND(null_mut()))).context("Failed to open clipboard")?;

		let result = (|| -> Result<()> {
			EmptyClipboard().context("Failed to empty clipboard")?;

			let memory = GlobalAlloc(GMEM_MOVEABLE, byte_len)
				.context("Failed to allocate clipboard memory")?;

			let pointer = GlobalLock(memory);

			if pointer.is_null() {
				bail!("Failed to lock clipboard memory");
			}

			std::ptr::copy_nonoverlapping(utf16.as_ptr(), pointer.cast::<u16>(), utf16.len());

			let _ = GlobalUnlock(memory);

			SetClipboardData(CF_UNICODETEXT, Option::from(HANDLE(memory.0)))
				.context("Failed to set clipboard data")?;

			Ok(())
		})();

		let _ = CloseClipboard();
		result
	}
}

pub fn clear() -> Result<()> {
	unsafe {
		OpenClipboard(Option::from(HWND(null_mut()))).context("Failed to open clipboard")?;

		let result = EmptyClipboard().context("Failed to empty clipboard");

		let _ = CloseClipboard();
		result
	}
}

pub fn paste() -> Result<()> {
	let inputs = [
		keyboard_input(VK_CONTROL, false),
		keyboard_input(VK_V, false),
		keyboard_input(VK_V, true),
		keyboard_input(VK_CONTROL, true),
	];

	let sent = unsafe { SendInput(&inputs, size_of::<INPUT>() as i32) };

	if sent != inputs.len() as u32 {
		bail!("Failed to send Ctrl+V input");
	}

	Ok(())
}

fn keyboard_input(key: VIRTUAL_KEY, key_up: bool) -> INPUT {
	INPUT {
		r#type: INPUT_KEYBOARD,
		Anonymous: INPUT_0 {
			ki: KEYBDINPUT {
				wVk: key,
				wScan: 0,
				dwFlags: if key_up {
					KEYEVENTF_KEYUP
				} else {
					Default::default()
				},
				time: 0,
				dwExtraInfo: 0,
			},
		},
	}
}
