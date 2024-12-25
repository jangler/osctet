use macroquad::input::*;

pub struct TextEditState {
    pub id: String,
    pub text: String,
    pub anchor: usize, // beginning of selection
    pub cursor: usize, // end of selection
}

impl TextEditState {
    pub fn new(id: String, text: String) -> Self {
        Self {
            id,
            anchor: 0,
            cursor: text.chars().count(),
            text,
        }
    }

    /// Handles text editing input. `mouse_i` is the character index of the
    /// mouse cursor, if the mouse cursor is over the text area.
    pub fn handle_input(&mut self, mouse_i: Option<usize>, clipboard: &mut Option<String>,
        max_width: usize
    ) {
        if let Some(i) = mouse_i {
            let i = i.min(self.text.chars().count());
            if is_mouse_button_pressed(MouseButton::Left) {
                self.set_cursor(i);
            } else if is_mouse_button_down(MouseButton::Left) {
                self.cursor = i;
            }
        }

        while let Some(c) = get_char_pressed() {
            if !c.is_ascii_control() {
                self.insert(&c.to_string(), max_width);
            }
        }

        for key in get_keys_pressed() {
            if is_key_down(KeyCode::LeftControl) || is_key_down(KeyCode::RightControl) {
                match key {
                    KeyCode::X => {
                        *clipboard = Some(self.selected_text().to_owned());
                        self.delete(0);
                    }
                    KeyCode::C => *clipboard = Some(self.selected_text().to_owned()),
                    KeyCode::V => if let Some(s) = clipboard {
                        self.insert(&s, max_width)
                    }
                    _ => (),
                }
            } else {
                match key {
                    KeyCode::Backspace => self.delete(-1),
                    KeyCode::Delete => self.delete(1),
                    KeyCode::Home => self.set_cursor(0),
                    KeyCode::End => self.set_cursor(self.text.chars().count()),
                    KeyCode::Left => if self.cursor > 0 {
                        self.set_cursor(self.cursor - 1);
                    } else {
                        self.set_cursor(0);
                    }
                    KeyCode::Right => if self.cursor < self.text.chars().count() {
                        self.set_cursor(self.cursor + 1);
                    } else {
                        self.set_cursor(self.cursor);
                    }
                    _ => (),
                }
            }
        }

        self.anchor = self.anchor.min(self.text.chars().count());
    }

    /// Sets the mouse cursor to the given position, updating anchor as needed.
    /// Does not check bounds.
    fn set_cursor(&mut self, pos: usize) {
        self.cursor = pos;
        if !(is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift)) {
            self.anchor = self.cursor;
        }
    }

    /// Insert text into the string at the cursor position.
    fn insert(&mut self, s: &str, max_width: usize) {
        if self.cursor != self.anchor {
            self.delete(0);
        }
        let s = {
            let n = self.text.chars().count();
            if n + s.chars().count() > max_width {
                &s.chars().take(max_width - n).collect::<String>()
            } else {
                s
            }
        };
        self.text.insert_str(self.cursor, s);
        self.cursor += s.chars().count();
        self.anchor = self.cursor;
    }

    /// Delete selected text. `offset` determines which character(s) are
    /// deleted when there is no selection.
    fn delete(&mut self, offset: isize) {
        if self.cursor == self.anchor {
            self.cursor = ((self.cursor as isize + offset).max(0) as usize)
                .min(self.text.chars().count());
        }

        let start = self.cursor.min(self.anchor);
        let end = self.cursor.max(self.anchor);

        self.text = self.text.chars()
            .enumerate()
            .filter_map(|(i, c)| {
                if i < start || i >= end {
                    Some(c)
                } else {
                    None
                }
            }).collect();

        self.cursor = start;
        self.anchor = start;
    }

    /// Returns the selected text.
    fn selected_text(&self) -> &str {
        let start = self.cursor.min(self.anchor);
        let end = self.cursor.max(self.anchor);

        if let Some((start, _)) = self.text.char_indices().nth(start) {
            if let Some((end, _)) = self.text.char_indices().nth(end) {
                &self.text[start..end]
            } else {
                &self.text[start..]
            }
        } else {
            ""
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_selected_text() {
        let mut state = TextEditState::new(String::from(""), String::from("hello"));
        assert_eq!(state.selected_text(), "hello");
        state.cursor = 0;
        assert_eq!(state.selected_text(), "");
        state.cursor = 1;
        assert_eq!(state.selected_text(), "h");
        state.cursor = 5;
        assert_eq!(state.selected_text(), "hello");
        state.anchor = 1;
        assert_eq!(state.selected_text(), "ello");
    }
}