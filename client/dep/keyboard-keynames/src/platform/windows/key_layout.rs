use std::convert::TryInto;
use crate::errors::KeyLayoutError;

/// The KeyLayout struct
pub struct KeyLayout {}

impl KeyLayout {
    
    /// Construct a KeyLayout from a winit Window
    pub fn new_from_window(_window: &winit::window::Window) -> Result<KeyLayout, KeyLayoutError> {
        Ok(KeyLayout {})
    }

    /// Convert a scancode to a String
    #[allow(unsafe_code)]
    pub fn get_key_as_string(&self, scancode: u32) -> String {
        // Try withMapVirtualKeyW first (single character)

        // MapVirtualKeyW is safe as on failure, it returns 0 on failure
        let win_vk = unsafe {
            winapi::um::winuser::MapVirtualKeyW(scancode, winapi::um::winuser::MAPVK_VSC_TO_VK_EX)
        };

        // MapVirtualKeyW is safe as on failure, it returns 0 on failure
        let char_key = unsafe {
            winapi::um::winuser::MapVirtualKeyW(win_vk, winapi::um::winuser::MAPVK_VK_TO_CHAR)
        };

        // return only if there is a visible character
        if char_key != 0 {
            let mut output = String::from_utf16_lossy(&[char_key as u16]);

            output = output.replace(|c: char| c.is_control(), "");

            if !output.trim().is_empty() {
                return output;
            }
        }

        // Fallback to GetKeyNameTextW (longer key name)

        // Convert the scancode
        let mut l_param: i32 = (scancode.clone()).try_into().unwrap();
        l_param <<= 16;

        // Check if 0xE0 escape sequence is present and set extended key flag
        if (scancode & 0x0000FF00) == 0xE000 {
            l_param |= 0b01 << 24;
        }

        // Buffer to get the utf-16 encoded key name
        let mut utf_key: [u16; 32] = [0; 32];

        // Call is safe: utf_key is not borrowed, and GetKeyNameTextW returns 0 if it fails
        let output_size =
            unsafe { winapi::um::winuser::GetKeyNameTextW(l_param, utf_key.as_mut_ptr(), 32) };

        // Truncate the array to the size of the key name
        let (utf_key, _) = utf_key.split_at(output_size.try_into().unwrap());

        String::from_utf16_lossy(utf_key)
    }

    /// Construct a KeyLayout
    pub fn new() -> Result<KeyLayout, KeyLayoutError> {
        Ok(KeyLayout {})
    }
}
