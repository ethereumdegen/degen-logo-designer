use std::ops::Range;
use std::rc::Rc;

use gpui::{
    App, Bounds, ClipboardItem, Context, CursorStyle, ElementId, ElementInputHandler, Entity,
    EntityInputHandler, FocusHandle, Focusable, GlobalElementId, KeyBinding, LayoutId, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, PaintQuad, Pixels, Point, ShapedLine,
    SharedString, Style, TextRun, UTF16Selection, Window, actions, div, fill, hsla,
    point, prelude::*, px, relative, rgba, size,
};
use unicode_segmentation::*;

actions!(
    text_input,
    [
        Backspace,
        Delete,
        Left,
        Right,
        Up,
        Down,
        SelectLeft,
        SelectRight,
        SelectAll,
        Home,
        End,
        Enter,
        ShowCharacterPalette,
        Paste,
        Cut,
        Copy,
    ]
);

pub fn key_bindings() -> Vec<KeyBinding> {
    vec![
        KeyBinding::new("backspace", Backspace, Some("TextInput")),
        KeyBinding::new("delete", Delete, Some("TextInput")),
        KeyBinding::new("left", Left, Some("TextInput")),
        KeyBinding::new("right", Right, Some("TextInput")),
        KeyBinding::new("up", Up, Some("TextInput")),
        KeyBinding::new("down", Down, Some("TextInput")),
        KeyBinding::new("shift-left", SelectLeft, Some("TextInput")),
        KeyBinding::new("shift-right", SelectRight, Some("TextInput")),
        KeyBinding::new("cmd-a", SelectAll, Some("TextInput")),
        KeyBinding::new("ctrl-a", SelectAll, Some("TextInput")),
        KeyBinding::new("cmd-v", Paste, Some("TextInput")),
        KeyBinding::new("ctrl-v", Paste, Some("TextInput")),
        KeyBinding::new("cmd-c", Copy, Some("TextInput")),
        KeyBinding::new("ctrl-c", Copy, Some("TextInput")),
        KeyBinding::new("cmd-x", Cut, Some("TextInput")),
        KeyBinding::new("ctrl-x", Cut, Some("TextInput")),
        KeyBinding::new("home", Home, Some("TextInput")),
        KeyBinding::new("end", End, Some("TextInput")),
        KeyBinding::new("enter", Enter, Some("TextInput")),
        KeyBinding::new("ctrl-cmd-space", ShowCharacterPalette, Some("TextInput")),
    ]
}

pub struct TextInput {
    focus_handle: FocusHandle,
    content: SharedString,
    pub placeholder: SharedString,
    selected_range: Range<usize>,
    selection_reversed: bool,
    marked_range: Option<Range<usize>>,
    last_layout: Option<Vec<ShapedLine>>,
    last_bounds: Option<Bounds<Pixels>>,
    is_selecting: bool,
    /// Called on Enter with the current content. TextInput clears itself.
    pub on_enter: Option<Rc<dyn Fn(String, &mut Window, &mut App)>>,
}

impl TextInput {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            content: "".into(),
            placeholder: "Type here...".into(),
            selected_range: 0..0,
            selection_reversed: false,
            marked_range: None,
            last_layout: None,
            last_bounds: None,
            is_selecting: false,
            on_enter: None,
        }
    }

    pub fn content(&self) -> &str {
        &self.content
    }

    pub fn set_content(&mut self, text: String, cx: &mut Context<Self>) {
        self.content = text.into();
        self.selected_range = 0..0;
        self.marked_range = None;
        cx.notify();
    }

    // Line helpers
    fn lines(&self) -> Vec<&str> {
        if self.content.is_empty() {
            vec![""]
        } else {
            self.content.split('\n').collect()
        }
    }

    fn line_and_col_for_offset(&self, offset: usize) -> (usize, usize) {
        let mut line = 0;
        let mut col = 0;
        for (i, ch) in self.content.char_indices() {
            if i >= offset {
                break;
            }
            if ch == '\n' {
                line += 1;
                col = 0;
            } else {
                col += ch.len_utf8();
            }
        }
        if offset > 0 && offset <= self.content.len() {
            // recalc col for exact offset
            let lines = self.lines();
            let mut pos = 0;
            for (i, l) in lines.iter().enumerate() {
                let line_end = pos + l.len();
                if offset <= line_end || i == lines.len() - 1 {
                    return (i, offset - pos);
                }
                pos = line_end + 1; // +1 for '\n'
            }
        }
        (line, col)
    }

    fn offset_for_line_col(&self, target_line: usize, target_col: usize) -> usize {
        let lines = self.lines();
        let line = target_line.min(lines.len().saturating_sub(1));
        let mut offset = 0;
        for i in 0..line {
            offset += lines[i].len() + 1; // +1 for '\n'
        }
        offset + target_col.min(lines[line].len())
    }

    fn line_start_offset(&self, offset: usize) -> usize {
        let (line, _) = self.line_and_col_for_offset(offset);
        self.offset_for_line_col(line, 0)
    }

    fn line_end_offset(&self, offset: usize) -> usize {
        let (line, _) = self.line_and_col_for_offset(offset);
        let lines = self.lines();
        let l = line.min(lines.len().saturating_sub(1));
        self.offset_for_line_col(l, lines[l].len())
    }

    fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.previous_boundary(self.cursor_offset()), cx);
        } else {
            self.move_to(self.selected_range.start, cx)
        }
    }

    fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.next_boundary(self.selected_range.end), cx);
        } else {
            self.move_to(self.selected_range.end, cx)
        }
    }

    fn up(&mut self, _: &Up, _: &mut Window, cx: &mut Context<Self>) {
        let (line, col) = self.line_and_col_for_offset(self.cursor_offset());
        if line > 0 {
            self.move_to(self.offset_for_line_col(line - 1, col), cx);
        } else {
            self.move_to(0, cx);
        }
    }

    fn down(&mut self, _: &Down, _: &mut Window, cx: &mut Context<Self>) {
        let (line, col) = self.line_and_col_for_offset(self.cursor_offset());
        let num_lines = self.lines().len();
        if line < num_lines - 1 {
            self.move_to(self.offset_for_line_col(line + 1, col), cx);
        } else {
            self.move_to(self.content.len(), cx);
        }
    }

    fn enter(&mut self, _: &Enter, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(cb) = self.on_enter.clone() {
            let content = self.content.to_string();
            self.content = "".into();
            self.selected_range = 0..0;
            self.marked_range = None;
            cx.notify();
            cb(content, window, cx);
        } else {
            self.replace_text_in_range(None, "\n", window, cx);
        }
    }

    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.previous_boundary(self.cursor_offset()), cx);
    }

    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.next_boundary(self.cursor_offset()), cx);
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, cx);
        self.select_to(self.content.len(), cx)
    }

    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        let start = self.line_start_offset(self.cursor_offset());
        self.move_to(start, cx);
    }

    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        let end = self.line_end_offset(self.cursor_offset());
        self.move_to(end, cx);
    }

    fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.select_to(self.previous_boundary(self.cursor_offset()), cx)
        }
        self.replace_text_in_range(None, "", window, cx)
    }

    fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.select_to(self.next_boundary(self.cursor_offset()), cx)
        }
        self.replace_text_in_range(None, "", window, cx)
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.is_selecting = true;
        if event.modifiers.shift {
            self.select_to(self.index_for_mouse_position(event.position), cx);
        } else {
            self.move_to(self.index_for_mouse_position(event.position), cx)
        }
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _window: &mut Window, _: &mut Context<Self>) {
        self.is_selecting = false;
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.is_selecting {
            self.select_to(self.index_for_mouse_position(event.position), cx);
        }
    }

    fn show_character_palette(
        &mut self,
        _: &ShowCharacterPalette,
        window: &mut Window,
        _: &mut Context<Self>,
    ) {
        window.show_character_palette();
    }

    fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.replace_text_in_range(None, &text, window, cx);
        }
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected_range.clone()].to_string(),
            ));
        }
    }

    fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected_range.clone()].to_string(),
            ));
            self.replace_text_in_range(None, "", window, cx)
        }
    }

    fn move_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.selected_range = offset..offset;
        cx.notify()
    }

    fn cursor_offset(&self) -> usize {
        if self.selection_reversed {
            self.selected_range.start
        } else {
            self.selected_range.end
        }
    }

    fn index_for_mouse_position(&self, position: Point<Pixels>) -> usize {
        if self.content.is_empty() {
            return 0;
        }

        let (Some(bounds), Some(lines)) = (self.last_bounds.as_ref(), self.last_layout.as_ref())
        else {
            return 0;
        };

        if position.y < bounds.top() {
            return 0;
        }
        if position.y > bounds.bottom() {
            return self.content.len();
        }

        // Find which line was clicked
        let line_height = if lines.is_empty() {
            px(20.0)
        } else {
            px(20.0)
        };
        let relative_y = position.y - bounds.top();
        let line_idx = (relative_y / line_height).floor() as usize;
        let text_lines = self.lines();
        let line_idx = line_idx.min(text_lines.len().saturating_sub(1));

        // Calculate offset to start of this line
        let mut offset = 0;
        for i in 0..line_idx {
            offset += text_lines[i].len() + 1;
        }

        // Find position within line
        if line_idx < lines.len() {
            let local_idx = lines[line_idx].closest_index_for_x(position.x - bounds.left());
            offset + local_idx
        } else {
            offset + text_lines[line_idx].len()
        }
    }

    fn select_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        if self.selection_reversed {
            self.selected_range.start = offset
        } else {
            self.selected_range.end = offset
        };
        if self.selected_range.end < self.selected_range.start {
            self.selection_reversed = !self.selection_reversed;
            self.selected_range = self.selected_range.end..self.selected_range.start;
        }
        cx.notify()
    }

    fn offset_from_utf16(&self, offset: usize) -> usize {
        let mut utf8_offset = 0;
        let mut utf16_count = 0;
        for ch in self.content.chars() {
            if utf16_count >= offset {
                break;
            }
            utf16_count += ch.len_utf16();
            utf8_offset += ch.len_utf8();
        }
        utf8_offset
    }

    fn offset_to_utf16(&self, offset: usize) -> usize {
        let mut utf16_offset = 0;
        let mut utf8_count = 0;
        for ch in self.content.chars() {
            if utf8_count >= offset {
                break;
            }
            utf8_count += ch.len_utf8();
            utf16_offset += ch.len_utf16();
        }
        utf16_offset
    }

    fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_to_utf16(range.start)..self.offset_to_utf16(range.end)
    }

    fn range_from_utf16(&self, range_utf16: &Range<usize>) -> Range<usize> {
        self.offset_from_utf16(range_utf16.start)..self.offset_from_utf16(range_utf16.end)
    }

    fn previous_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .rev()
            .find_map(|(idx, _)| (idx < offset).then_some(idx))
            .unwrap_or(0)
    }

    fn next_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .find_map(|(idx, _)| (idx > offset).then_some(idx))
            .unwrap_or(self.content.len())
    }
}

impl EntityInputHandler for TextInput {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.range_from_utf16(&range_utf16);
        actual_range.replace(self.range_to_utf16(&range));
        Some(self.content[range].to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.range_to_utf16(&self.selected_range),
            reversed: self.selection_reversed,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.marked_range
            .as_ref()
            .map(|range| self.range_to_utf16(range))
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        self.marked_range = None;
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());

        self.content =
            (self.content[0..range.start].to_owned() + new_text + &self.content[range.end..])
                .into();
        self.selected_range = range.start + new_text.len()..range.start + new_text.len();
        self.marked_range.take();
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());

        self.content =
            (self.content[0..range.start].to_owned() + new_text + &self.content[range.end..])
                .into();
        if !new_text.is_empty() {
            self.marked_range = Some(range.start..range.start + new_text.len());
        } else {
            self.marked_range = None;
        }
        self.selected_range = new_selected_range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .map(|new_range| new_range.start + range.start..new_range.end + range.end)
            .unwrap_or_else(|| range.start + new_text.len()..range.start + new_text.len());

        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let last_layout = self.last_layout.as_ref()?;
        if last_layout.is_empty() {
            return None;
        }
        let range = self.range_from_utf16(&range_utf16);
        // Use first line for bounds approximation
        Some(Bounds::from_corners(
            point(
                bounds.left() + last_layout[0].x_for_index(range.start),
                bounds.top(),
            ),
            point(
                bounds.left() + last_layout[0].x_for_index(range.end),
                bounds.bottom(),
            ),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: gpui::Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        let bounds = self.last_bounds?;
        let _line_point = bounds.localize(&point)?;
        let last_layout = self.last_layout.as_ref()?;
        if last_layout.is_empty() {
            return None;
        }
        let utf8_index = last_layout[0].index_for_x(point.x - bounds.origin.x)?;
        Some(self.offset_to_utf16(utf8_index))
    }
}

pub struct TextElement {
    input: Entity<TextInput>,
}

pub struct MultilinePrepaintState {
    lines: Vec<ShapedLine>,
    cursor: Option<PaintQuad>,
    selection: Option<PaintQuad>,
}

impl IntoElement for TextElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for TextElement {
    type RequestLayoutState = ();
    type PrepaintState = MultilinePrepaintState;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let input = self.input.read(cx);
        let num_lines = if input.content.is_empty() {
            1
        } else {
            input.content.split('\n').count()
        };
        let line_h = window.line_height();
        let total_h = line_h * num_lines;

        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = total_h.into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let input = self.input.read(cx);
        let content = input.content.clone();
        let selected_range = input.selected_range.clone();
        let cursor = input.cursor_offset();
        let style = window.text_style();

        let text_lines: Vec<String> = if content.is_empty() {
            vec![String::new()]
        } else {
            content.split('\n').map(|s| s.to_string()).collect()
        };

        let is_placeholder = content.is_empty();
        let display_text = if is_placeholder {
            input.placeholder.clone()
        } else {
            content.clone()
        };
        let text_color = if is_placeholder {
            hsla(0., 0., 0.4, 1.0)
        } else {
            style.color
        };

        let font_size = style.font_size.to_pixels(window.rem_size());

        // Shape each line
        let shaped_lines: Vec<ShapedLine> = if is_placeholder {
            let run = TextRun {
                len: display_text.len(),
                font: style.font(),
                color: text_color,
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            vec![window
                .text_system()
                .shape_line(display_text.clone(), font_size, &[run], None)]
        } else {
            text_lines
                .iter()
                .map(|line_text| {
                    let text: SharedString = if line_text.is_empty() {
                        " ".into()
                    } else {
                        line_text.clone().into()
                    };
                    let run = TextRun {
                        len: text.len(),
                        font: style.font(),
                        color: text_color,
                        background_color: None,
                        underline: None,
                        strikethrough: None,
                    };
                    window
                        .text_system()
                        .shape_line(text, font_size, &[run], None)
                })
                .collect()
        };

        // Compute cursor position
        let line_height = window.line_height();
        let (cursor_line, cursor_col) = input.line_and_col_for_offset(cursor);

        let cursor_quad = if selected_range.is_empty() && !is_placeholder {
            let line_idx = cursor_line.min(shaped_lines.len().saturating_sub(1));
            let x = if cursor_col == 0 {
                px(0.0)
            } else {
                let empty = String::new();
                let actual_line_text = text_lines.get(cursor_line).unwrap_or(&empty);
                let col = cursor_col.min(actual_line_text.len());
                shaped_lines[line_idx].x_for_index(col)
            };
            Some(fill(
                Bounds::new(
                    point(bounds.left() + x, bounds.top() + line_height * cursor_line),
                    size(px(2.), line_height),
                ),
                gpui::blue(),
            ))
        } else {
            None
        };

        let selection_quad = if !selected_range.is_empty() && !is_placeholder {
            // Simple: highlight on the first selected line only
            let (start_line, start_col) =
                input.line_and_col_for_offset(selected_range.start);
            let (end_line, end_col) =
                input.line_and_col_for_offset(selected_range.end);

            if start_line == end_line {
                let line_idx = start_line.min(shaped_lines.len().saturating_sub(1));
                let x1 = shaped_lines[line_idx].x_for_index(start_col);
                let x2 = shaped_lines[line_idx].x_for_index(end_col);
                Some(fill(
                    Bounds::from_corners(
                        point(
                            bounds.left() + x1,
                            bounds.top() + line_height * start_line,
                        ),
                        point(
                            bounds.left() + x2,
                            bounds.top() + line_height * (start_line + 1),
                        ),
                    ),
                    rgba(0x3311ff30),
                ))
            } else {
                // Multi-line: just highlight first line selection to end
                let line_idx = start_line.min(shaped_lines.len().saturating_sub(1));
                let x1 = shaped_lines[line_idx].x_for_index(start_col);
                let empty2 = String::new();
                let actual_text = text_lines.get(start_line).unwrap_or(&empty2);
                let x2 = shaped_lines[line_idx].x_for_index(actual_text.len());
                Some(fill(
                    Bounds::from_corners(
                        point(
                            bounds.left() + x1,
                            bounds.top() + line_height * start_line,
                        ),
                        point(
                            bounds.left() + x2,
                            bounds.top() + line_height * (end_line + 1),
                        ),
                    ),
                    rgba(0x3311ff30),
                ))
            }
        } else {
            None
        };

        MultilinePrepaintState {
            lines: shaped_lines,
            cursor: cursor_quad,
            selection: selection_quad,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus_handle = self.input.read(cx).focus_handle.clone();
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.input.clone()),
            cx,
        );

        if let Some(selection) = prepaint.selection.take() {
            window.paint_quad(selection)
        }

        let line_height = window.line_height();
        let lines: Vec<ShapedLine> = std::mem::take(&mut prepaint.lines);

        for (i, line) in lines.iter().enumerate() {
            let origin = point(bounds.left(), bounds.top() + line_height * i);
            line.paint(origin, line_height, window, cx).unwrap();
        }

        if focus_handle.is_focused(window) {
            if let Some(cursor) = prepaint.cursor.take() {
                window.paint_quad(cursor);
            }
        }

        self.input.update(cx, |input, _cx| {
            input.last_layout = Some(lines);
            input.last_bounds = Some(bounds);
        });
    }
}

impl Render for TextInput {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .key_context("TextInput")
            .track_focus(&self.focus_handle(cx))
            .cursor(CursorStyle::IBeam)
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::up))
            .on_action(cx.listener(Self::down))
            .on_action(cx.listener(Self::enter))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::show_character_palette))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::copy))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .line_height(px(20.))
            .text_size(px(14.))
            .child(
                div()
                    .id("text-input-scroll")
                    .w_full()
                    .flex_grow()
                    .p(px(4.))
                    .overflow_y_scroll()
                    .child(TextElement {
                        input: cx.entity(),
                    }),
            )
    }
}

impl Focusable for TextInput {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}
