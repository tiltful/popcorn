use anyhow::{Context, Result, bail};
use std::mem::size_of;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput, VIRTUAL_KEY,
};
use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

const CF_UNICODETEXT: u32 = 13;
const VK_CONTROL: VIRTUAL_KEY = VIRTUAL_KEY(0x11);
const VK_V: VIRTUAL_KEY = VIRTUAL_KEY(0x56);

/// Places UTF-16 text into the system clipboard.
pub fn set_text(text: &str) -> Result<()> {
	let mut utf16: Vec<u16> = text.encode_utf16().collect();
	utf16.push(0);

	let byte_len = utf16.len() * size_of::<u16>();

	unsafe {
		OpenClipboard(Option::from(GetForegroundWindow())).context("Failed to open clipboard")?;

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

			// The system owns the memory after SetClipboardData succeeds.
			Ok(())
		})();

		let _ = CloseClipboard();
		result
	}
}

/// Sends Ctrl+V to the foreground application.
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
