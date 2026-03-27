use std::rc::Rc;

use gpui::*;
use gpui::prelude::FluentBuilder;

use crate::state::{AppState, GenerationStatus, LogEntry, ImageRecord, STYLES};
use crate::text_input::TextInput;
use crate::theme::Theme;

#[derive(Clone, PartialEq)]
pub enum Page {
    Design,
    Settings,
    Logs,
    History,
}

pub struct Workspace {
    pub state: Entity<AppState>,
    pub page: Page,
    pub prompt_input: Entity<TextInput>,
    pub evolve_input: Entity<TextInput>,
    pub api_key_input: Entity<TextInput>,

    pub show_style_dropdown: bool,

    pub on_generate: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
    pub on_evolve: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
    pub on_save_key: Option<Rc<dyn Fn(&mut Window, &mut App)>>,

    pub export_status: Option<String>,
}

impl Workspace {
    pub fn new(
        state: Entity<AppState>,
        prompt_input: Entity<TextInput>,
        evolve_input: Entity<TextInput>,
        api_key_input: Entity<TextInput>,
        _cx: &mut Context<Self>,
    ) -> Self {
        Self {
            state,
            page: Page::Design,
            prompt_input,
            evolve_input,
            api_key_input,
            show_style_dropdown: false,
            on_generate: None,
            on_evolve: None,
            on_save_key: None,
            export_status: None,
        }
    }

    fn export_file(src: &std::path::Path, name_hint: &str, ext: &str) -> Result<std::path::PathBuf, String> {
        let desktop = dirs_next::desktop_dir()
            .or_else(dirs_next::download_dir)
            .or_else(dirs_next::home_dir)
            .ok_or_else(|| "Could not find Desktop/Downloads directory".to_string())?;

        let base_name = format!("{}.{}", name_hint, ext);
        let mut dest = desktop.join(&base_name);

        // Avoid overwriting existing files
        let mut counter = 1u32;
        while dest.exists() {
            dest = desktop.join(format!("{}_{}.{}", name_hint, counter, ext));
            counter += 1;
        }

        std::fs::copy(src, &dest).map_err(|e| format!("Failed to export: {}", e))?;
        Ok(dest)
    }

    fn open_in_file_manager(path: &std::path::Path) {
        let dir = if path.is_dir() { path } else { path.parent().unwrap_or(path) };
        #[cfg(target_os = "linux")]
        { let _ = std::process::Command::new("xdg-open").arg(dir).spawn(); }
        #[cfg(target_os = "macos")]
        { let _ = std::process::Command::new("open").arg(dir).spawn(); }
        #[cfg(target_os = "windows")]
        { let _ = std::process::Command::new("explorer").arg(dir).spawn(); }
    }

    fn build_export_buttons(
        &self,
        sel: &ImageRecord,
        images_path: &std::path::Path,
        cx: &mut Context<Self>,
    ) -> Div {
        let sel_path = images_path.join(&sel.filename);
        let prompt_slug: String = sel.prompt.chars()
            .filter(|c| c.is_alphanumeric() || *c == ' ')
            .take(30)
            .collect::<String>()
            .trim()
            .replace(' ', "-")
            .to_lowercase();
        let name_hint = if prompt_slug.is_empty() { sel.id.clone() } else { prompt_slug };

        let mut row = div()
            .flex()
            .flex_row()
            .gap(px(6.0));

        // Export SVG button (only for SVG files)
        if sel.file_type == "svg" {
            let svg_src = sel_path.clone();
            let hint = name_hint.clone();
            row = row.child(
                div()
                    .id("export-svg-btn")
                    .px(px(10.0))
                    .py(px(5.0))
                    .rounded(px(4.0))
                    .bg(Theme::button_bg())
                    .hover(|s| s.bg(Theme::button_hover()))
                    .cursor_pointer()
                    .child(div().text_xs().text_color(Theme::text_primary()).child("Export SVG"))
                    .on_click(cx.listener(move |this, _ev, _window, _cx| {
                        match Self::export_file(&svg_src, &hint, "svg") {
                            Ok(dest) => this.export_status = Some(format!("Saved to {}", dest.display())),
                            Err(e) => this.export_status = Some(e),
                        }
                    })),
            );
        }

        // Export PNG button
        let png_src = if sel.file_type == "svg" {
            sel_path.with_extension("png")
        } else {
            sel_path.clone()
        };
        if png_src.exists() {
            let hint = name_hint.clone();
            row = row.child(
                div()
                    .id("export-png-btn")
                    .px(px(10.0))
                    .py(px(5.0))
                    .rounded(px(4.0))
                    .bg(Theme::button_bg())
                    .hover(|s| s.bg(Theme::button_hover()))
                    .cursor_pointer()
                    .child(div().text_xs().text_color(Theme::text_primary()).child("Export PNG"))
                    .on_click(cx.listener(move |this, _ev, _window, _cx| {
                        match Self::export_file(&png_src, &hint, "png") {
                            Ok(dest) => this.export_status = Some(format!("Saved to {}", dest.display())),
                            Err(e) => this.export_status = Some(e),
                        }
                    })),
            );
        }

        // Open folder button
        let folder = images_path.to_path_buf();
        row = row.child(
            div()
                .id("open-folder-btn")
                .px(px(10.0))
                .py(px(5.0))
                .rounded(px(4.0))
                .bg(Theme::button_bg())
                .hover(|s| s.bg(Theme::button_hover()))
                .cursor_pointer()
                .child(div().text_xs().text_color(Theme::text_primary()).child("Open Folder"))
                .on_click(move |_ev, _window, _cx| {
                    Self::open_in_file_manager(&folder);
                }),
        );

        row
    }
}

impl Render for Workspace {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let sessions = state.sessions.clone();
        let active_session_id = state.active_session_id.clone();
        let has_key = state.fal_key.is_some();
        let page = self.page.clone();
        let state_entity = self.state.clone();

        // ========== SIDEBAR ==========
        let mut session_items: Vec<AnyElement> = Vec::new();
        for session in &sessions {
            let sid = session.id.clone();
            let is_active = active_session_id.as_deref() == Some(&session.id);
            let name = session.name.clone();
            let state_clone = state_entity.clone();

            session_items.push(
                div()
                    .id(SharedString::from(format!("sess-{}", sid)))
                    .w_full()
                    .px(px(8.0))
                    .py(px(6.0))
                    .rounded(px(4.0))
                    .bg(if is_active { Theme::selection_bg() } else { gpui::transparent_black() })
                    .hover(|s| s.bg(Theme::button_hover()))
                    .cursor_pointer()
                    .child(
                        div()
                            .text_sm()
                            .text_color(if is_active { Theme::text_primary() } else { Theme::text_secondary() })
                            .overflow_hidden()
                            .child(name),
                    )
                    .on_click(move |_ev, _window, cx| {
                        state_clone.update(cx, |s, _cx| {
                            s.select_session(&sid);
                        });
                    })
                    .into_any_element(),
            );
        }

        let state_new = state_entity.clone();
        let new_btn = div()
            .id("new-session-btn")
            .w_full()
            .px(px(8.0))
            .py(px(6.0))
            .rounded(px(4.0))
            .bg(Theme::button_primary())
            .hover(|s| s.bg(Theme::button_primary_hover()))
            .cursor_pointer()
            .child(div().text_sm().text_color(Theme::text_primary()).child("+ New Session"))
            .on_click(move |_ev, _window, cx| {
                state_new.update(cx, |s, _cx| {
                    let session = s.create_session("Untitled Logo");
                    s.select_session(&session.id);
                });
            });

        let logs_btn = div()
            .id("logs-btn")
            .w_full()
            .px(px(8.0))
            .py(px(6.0))
            .rounded(px(4.0))
            .bg(if page == Page::Logs { Theme::selection_bg() } else { gpui::transparent_black() })
            .hover(|s| s.bg(Theme::button_hover()))
            .cursor_pointer()
            .child(div().text_sm().text_color(Theme::text_muted()).child("API Logs"))
            .on_click(cx.listener(|this, _ev: &ClickEvent, _window, _cx| {
                this.page = Page::Logs;
            }));

        let history_btn = div()
            .id("history-btn")
            .w_full()
            .px(px(8.0))
            .py(px(6.0))
            .rounded(px(4.0))
            .bg(if page == Page::History { Theme::selection_bg() } else { gpui::transparent_black() })
            .hover(|s| s.bg(Theme::button_hover()))
            .cursor_pointer()
            .child(div().text_sm().text_color(Theme::text_muted()).child("Session History"))
            .on_click(cx.listener(|this, _ev: &ClickEvent, _window, _cx| {
                this.page = Page::History;
            }));

        let settings_btn = div()
            .id("settings-btn")
            .w_full()
            .px(px(8.0))
            .py(px(6.0))
            .rounded(px(4.0))
            .bg(if page == Page::Settings { Theme::selection_bg() } else { gpui::transparent_black() })
            .hover(|s| s.bg(Theme::button_hover()))
            .cursor_pointer()
            .child(div().text_sm().text_color(Theme::text_muted()).child("Settings"))
            .on_click(cx.listener(|this, _ev: &ClickEvent, _window, _cx| {
                this.page = Page::Settings;
            }));

        let sidebar = div()
            .flex()
            .flex_col()
            .w(px(220.0))
            .h_full()
            .bg(Theme::bg_panel())
            .border_r_1()
            .border_color(Theme::border())
            .child(
                div()
                    .px(px(12.0))
                    .py(px(12.0))
                    .border_b_1()
                    .border_color(Theme::border())
                    .child(div().text_base().text_color(Theme::text_primary()).child("Logo Designer")),
            )
            .child(div().p(px(8.0)).child(new_btn))
            .child(
                div()
                    .id("session-list-scroll")
                    .flex_grow()
                    .overflow_y_scroll()
                    .p(px(4.0))
                    .children(session_items),
            )
            .child(
                div()
                    .border_t_1()
                    .border_color(Theme::border())
                    .p(px(8.0))
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .child(history_btn)
                    .child(logs_btn)
                    .child(settings_btn),
            );

        // ========== MAIN CONTENT ==========
        // Drop the immutable borrow on state before calling methods that need &mut cx
        let session_images = state.session_images.clone();
        let selected_image_id = state.selected_image_id.clone();
        let selected_image = state.selected_image().cloned();
        let images_path_clone = state.images_path.clone();
        let active_session_name = state.active_session().map(|s| s.name.clone());
        let style_idx = state.selected_style_idx;
        let gen_status = state.status.clone();
        let has_active = state.active_session_id.is_some();
        let active_sid = state.active_session_id.clone();
        let _ = state; // done reading state

        let logs = self.state.read(cx).logs.clone();

        let image_counts = {
            let conn = self.state.read(cx).db.lock().unwrap();
            crate::db::get_session_image_counts(&conn).unwrap_or_default()
        };

        let content: AnyElement = match self.page {
            Page::Design => self.render_design_page(
                has_active,
                active_sid,
                active_session_name,
                style_idx,
                gen_status,
                session_images,
                selected_image_id,
                selected_image,
                images_path_clone,
                cx,
            ),
            Page::Settings => self.render_settings_page(has_key, cx),
            Page::Logs => self.render_logs_page(logs, cx),
            Page::History => self.render_history_page(sessions, image_counts, cx),
        };

        div()
            .flex()
            .flex_row()
            .size_full()
            .bg(Theme::bg_primary())
            .text_color(Theme::text_primary())
            .child(sidebar)
            .child(content)
    }
}

impl Workspace {
    fn render_design_page(
        &self,
        has_active: bool,
        active_sid: Option<String>,
        active_session_name: Option<String>,
        style_idx: usize,
        gen_status: GenerationStatus,
        images: Vec<crate::state::ImageRecord>,
        selected_id: Option<String>,
        selected_image: Option<crate::state::ImageRecord>,
        images_path: std::path::PathBuf,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if !has_active {
            return div()
                .flex()
                .flex_col()
                .flex_grow()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_xl()
                        .text_color(Theme::text_muted())
                        .child("Select or create a session to start designing"),
                )
                .into_any_element();
        }

        let session_name = active_session_name.unwrap_or_default();
        let style_label = STYLES[style_idx].1;
        let is_generating = gen_status == GenerationStatus::Generating;
        let status_text = match &gen_status {
            GenerationStatus::Idle => String::new(),
            GenerationStatus::Generating => "Generating...".to_string(),
            GenerationStatus::Error(e) => format!("Error: {}", e),
        };
        let has_error = matches!(gen_status, GenerationStatus::Error(_));
        let state_entity = self.state.clone();

        // --- Header ---
        let state_del = state_entity.clone();
        let del_id = active_sid.clone().unwrap_or_default();
        let header = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .px(px(16.0))
            .py(px(10.0))
            .border_b_1()
            .border_color(Theme::border())
            .child(div().text_base().text_color(Theme::text_primary()).child(session_name))
            .child(
                div()
                    .id("delete-session-btn")
                    .px(px(10.0))
                    .py(px(4.0))
                    .rounded(px(4.0))
                    .bg(Theme::button_red())
                    .hover(|s| s.bg(Theme::button_red_hover()))
                    .cursor_pointer()
                    .child(div().text_xs().text_color(Theme::text_primary()).child("Delete"))
                    .on_click(move |_ev, _window, cx| {
                        state_del.update(cx, |s, _cx| { s.delete_session(&del_id); });
                    }),
            );

        // --- Generate bar ---
        let show_dropdown = self.show_style_dropdown;

        let style_toggle = div()
            .id("style-dropdown-toggle")
            .px(px(10.0))
            .py(px(6.0))
            .rounded(px(4.0))
            .bg(Theme::button_bg())
            .hover(|s| s.bg(Theme::button_hover()))
            .cursor_pointer()
            .child(div().text_xs().text_color(Theme::purple()).child(format!("Style: {} {}", style_label, if show_dropdown { "▲" } else { "▼" })))
            .on_click(cx.listener(|this, _ev: &ClickEvent, _window, _cx| {
                this.show_style_dropdown = !this.show_style_dropdown;
            }));

        let style_picker = if show_dropdown {
            let mut items: Vec<AnyElement> = Vec::new();
            for (i, (_value, label)) in STYLES.iter().enumerate() {
                let is_selected = i == style_idx;
                let state_pick = state_entity.clone();
                items.push(
                    div()
                        .id(SharedString::from(format!("style-{}", i)))
                        .w_full()
                        .px(px(10.0))
                        .py(px(5.0))
                        .bg(if is_selected { Theme::selection_bg() } else { Theme::bg_panel() })
                        .hover(|s| s.bg(Theme::button_hover()))
                        .cursor_pointer()
                        .child(
                            div()
                                .text_xs()
                                .text_color(if is_selected { Theme::purple() } else { Theme::text_secondary() })
                                .child(*label),
                        )
                        .on_click(cx.listener(move |this, _ev: &ClickEvent, _window, cx| {
                            state_pick.update(cx, |s, _cx| {
                                s.selected_style_idx = i;
                            });
                            this.show_style_dropdown = false;
                        }))
                        .into_any_element(),
                );
            }

            Some(
                div()
                    .absolute()
                    .top(px(30.0))
                    .left(px(0.0))
                    .w(px(180.0))
                    .bg(Theme::bg_panel())
                    .border_1()
                    .border_color(Theme::border())
                    .rounded(px(4.0))
                    .py(px(2.0))
                    .children(items),
            )
        } else {
            None
        };

        let style_container = div()
            .relative()
            .child(style_toggle)
            .children(style_picker);

        let on_gen = self.on_generate.clone();
        let generate_btn = div()
            .id("generate-btn")
            .px(px(14.0))
            .py(px(6.0))
            .rounded(px(4.0))
            .bg(if is_generating { Theme::button_bg() } else { Theme::button_primary() })
            .when(!is_generating, |d| d.hover(|s| s.bg(Theme::button_primary_hover())))
            .cursor_pointer()
            .child(
                div().text_sm().text_color(Theme::text_primary()).child(
                    if is_generating { "Generating..." } else { "Generate Logo" },
                ),
            )
            .on_click(move |_ev, window, cx| {
                if let Some(cb) = &on_gen { cb(window, cx); }
            });

        let prompt_bar = div()
            .flex()
            .flex_col()
            .px(px(16.0))
            .py(px(10.0))
            .gap(px(6.0))
            .border_b_1()
            .border_color(Theme::border())
            .child(div().text_xs().text_color(Theme::text_muted()).child("Describe your logo (Enter to submit)"))
            .child(
                div()
                    .w_full()
                    .h(px(60.0))
                    .bg(Theme::bg_tertiary())
                    .rounded(px(4.0))
                    .border_1()
                    .border_color(Theme::border())
                    .p(px(6.0))
                    .text_color(Theme::text_primary())
                    .child(self.prompt_input.clone()),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(8.0))
                    .items_center()
                    .child(style_container)
                    .child(generate_btn),
            );

        // --- Status ---
        let status_bar: Option<Div> = if !status_text.is_empty() {
            Some(
                div()
                    .px(px(16.0))
                    .py(px(6.0))
                    .child(
                        div()
                            .text_xs()
                            .text_color(if has_error { Theme::red() } else { Theme::yellow() })
                            .child(status_text),
                    ),
            )
        } else {
            None
        };

        // --- Image grid ---
        let mut grid_items: Vec<AnyElement> = Vec::new();
        for image_rec in &images {
            let img_id = image_rec.id.clone();
            let is_selected = selected_id.as_deref() == Some(&image_rec.id);
            let img_path = images_path.join(&image_rec.filename);
            let state_sel = state_entity.clone();
            let is_evolved = image_rec.parent_image_id.is_some();

            let mut card = div()
                .id(SharedString::from(format!("img-{}", img_id)))
                .w(px(140.0))
                .h(px(140.0))
                .rounded(px(6.0))
                .border_2()
                .border_color(if is_selected { Theme::purple() } else { Theme::border() })
                .bg(Theme::image_bg())
                .overflow_hidden()
                .cursor_pointer()
                .hover(|s| s.border_color(Theme::text_muted()))
                .on_click(move |_ev, _window, cx| {
                    state_sel.update(cx, |s, _cx| {
                        s.selected_image_id = Some(img_id.clone());
                    });
                });

            // For SVG files, look for the rendered PNG preview
            let display_path = if image_rec.file_type == "svg" {
                let png_preview = img_path.with_extension("png");
                if png_preview.exists() { png_preview } else { img_path.clone() }
            } else {
                img_path.clone()
            };

            if display_path.exists() && (image_rec.file_type == "png" || display_path.extension().is_some_and(|e| e == "png")) {
                card = card.child(
                    img(display_path)
                        .w(px(130.0))
                        .h(px(130.0))
                        .object_fit(ObjectFit::Contain),
                );
            } else {
                card = card.child(
                    div()
                        .w_full()
                        .h_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            div()
                                .text_xs()
                                .text_color(Theme::text_muted())
                                .child(if is_evolved { "Evolved" } else { "SVG" }),
                        ),
                );
            }

            grid_items.push(card.into_any_element());
        }

        if images.is_empty() && !is_generating {
            grid_items.push(
                div()
                    .text_sm()
                    .text_color(Theme::text_muted())
                    .py(px(40.0))
                    .child("Generate your first logo above")
                    .into_any_element(),
            );
        }

        if is_generating {
            grid_items.push(
                div()
                    .w(px(140.0))
                    .h(px(140.0))
                    .rounded(px(6.0))
                    .border_2()
                    .border_color(Theme::border())
                    .bg(Theme::bg_tertiary())
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(div().text_xs().text_color(Theme::yellow()).child("Creating..."))
                    .into_any_element(),
            );
        }

        let image_grid = div()
            .id("image-grid-scroll")
            .flex()
            .flex_row()
            .flex_wrap()
            .gap(px(10.0))
            .flex_grow()
            .overflow_y_scroll()
            .p(px(16.0))
            .children(grid_items);

        // --- Preview panel ---
        let preview: Option<AnyElement> = if let Some(sel) = selected_image {
            let sel_path = images_path.join(&sel.filename);
            let on_ev = self.on_evolve.clone();

            let mut img_container = div()
                .w_full()
                .h(px(240.0))
                .bg(Theme::image_bg())
                .rounded(px(6.0))
                .flex()
                .items_center()
                .justify_center()
                .overflow_hidden();

            // For SVG files, use the rendered PNG preview
            let sel_display_path = if sel.file_type == "svg" {
                let png_preview = sel_path.with_extension("png");
                if png_preview.exists() { png_preview } else { sel_path.clone() }
            } else {
                sel_path.clone()
            };

            if sel_display_path.exists() && (sel.file_type == "png" || sel_display_path.extension().is_some_and(|e| e == "png")) {
                img_container = img_container.child(
                    img(sel_display_path)
                        .w(px(220.0))
                        .h(px(220.0))
                        .object_fit(ObjectFit::Contain),
                );
            } else {
                img_container = img_container.child(
                    div().text_sm().text_color(Theme::text_muted()).child("SVG preview not available"),
                );
            }

            let evolve_label = if is_generating { "Evolving..." } else { "Evolve" };

            let preview_div = div()
                .w(px(320.0))
                .flex_shrink_0()
                .flex()
                .flex_col()
                .gap(px(10.0))
                .p(px(16.0))
                .border_l_1()
                .border_color(Theme::border())
                .bg(Theme::bg_panel())
                .child(img_container)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(2.0))
                        .child(div().text_xs().text_color(Theme::text_secondary()).child(sel.prompt.clone()))
                        .child(
                            div().text_xs().text_color(Theme::text_muted()).child(format!(
                                "{} | {}{}",
                                sel.model,
                                sel.file_type.to_uppercase(),
                                if sel.parent_image_id.is_some() { " | Evolved" } else { "" }
                            )),
                        ),
                )
                // Export buttons
                .child(
                    div()
                        .border_t_1()
                        .border_color(Theme::border())
                        .pt(px(8.0))
                        .flex()
                        .flex_col()
                        .gap(px(4.0))
                        .child(div().text_xs().text_color(Theme::text_muted()).child("Export"))
                        .child(self.build_export_buttons(&sel, &images_path, cx))
                        .children(self.export_status.as_ref().map(|s| {
                            div().text_xs().text_color(Theme::green()).child(s.clone())
                        })),
                )
                .child(
                    div()
                        .border_t_1()
                        .border_color(Theme::border())
                        .pt(px(8.0))
                        .child(div().text_xs().text_color(Theme::text_muted()).mb(px(4.0)).child("Evolve this logo"))
                        .child(
                            div()
                                .w_full()
                                .h(px(50.0))
                                .bg(Theme::bg_tertiary())
                                .rounded(px(4.0))
                                .border_1()
                                .border_color(Theme::border())
                                .p(px(6.0))
                                .text_color(Theme::text_primary())
                                .child(self.evolve_input.clone()),
                        ),
                )
                .child(
                    div()
                        .id("evolve-btn")
                        .px(px(14.0))
                        .py(px(6.0))
                        .rounded(px(4.0))
                        .bg(if is_generating { Theme::button_bg() } else { Theme::button_green() })
                        .when(!is_generating, |d| d.hover(|s| s.bg(Theme::button_green_hover())))
                        .cursor_pointer()
                        .child(div().text_sm().text_color(Theme::text_primary()).child(evolve_label))
                        .on_click(move |_ev, window, cx| {
                            if let Some(cb) = &on_ev { cb(window, cx); }
                        }),
                );

            Some(preview_div.into_any_element())
        } else {
            None
        };

        let content = div()
            .flex()
            .flex_row()
            .flex_grow()
            .overflow_hidden()
            .child(image_grid)
            .children(preview);

        div()
            .flex()
            .flex_col()
            .flex_grow()
            .child(header)
            .child(prompt_bar)
            .children(status_bar)
            .child(content)
            .into_any_element()
    }

    fn render_settings_page(
        &self,
        has_key: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let on_save = self.on_save_key.clone();

        div()
            .flex()
            .flex_col()
            .flex_grow()
            .p(px(32.0))
            .gap(px(16.0))
            .child(div().text_xl().text_color(Theme::text_primary()).child("Settings"))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .p(px(16.0))
                    .bg(Theme::bg_panel())
                    .rounded(px(6.0))
                    .border_1()
                    .border_color(Theme::border())
                    .child(div().text_sm().text_color(Theme::text_primary()).child("FAL AI API Key"))
                    .child(div().text_xs().text_color(Theme::text_muted()).child("Get your key from fal.ai/dashboard/keys"))
                    .when(has_key, |d| {
                        d.child(
                            div().text_xs().text_color(Theme::green()).child("Key is configured. Enter a new one to replace it."),
                        )
                    })
                    .child(
                        div()
                            .w_full()
                            .h(px(32.0))
                            .bg(Theme::bg_tertiary())
                            .rounded(px(4.0))
                            .border_1()
                            .border_color(Theme::border())
                            .p(px(6.0))
                            .text_color(Theme::text_primary())
                            .child(self.api_key_input.clone()),
                    )
                    .child(
                        div()
                            .id("save-key-btn")
                            .w(px(100.0))
                            .px(px(14.0))
                            .py(px(6.0))
                            .rounded(px(4.0))
                            .bg(Theme::button_primary())
                            .hover(|s| s.bg(Theme::button_primary_hover()))
                            .cursor_pointer()
                            .child(div().text_sm().text_color(Theme::text_primary()).child("Save Key"))
                            .on_click(move |_ev, window, cx| {
                                if let Some(cb) = &on_save { cb(window, cx); }
                            }),
                    ),
            )
            .child(
                div()
                    .id("back-to-design-btn")
                    .w(px(140.0))
                    .px(px(14.0))
                    .py(px(6.0))
                    .rounded(px(4.0))
                    .bg(Theme::button_bg())
                    .hover(|s| s.bg(Theme::button_hover()))
                    .cursor_pointer()
                    .child(div().text_sm().text_color(Theme::text_secondary()).child("Back to Design"))
                    .on_click(cx.listener(|this, _ev: &ClickEvent, _window, _cx| {
                        this.page = Page::Design;
                    })),
            )
            .into_any_element()
    }

    fn render_logs_page(
        &self,
        logs: Vec<LogEntry>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut log_rows: Vec<AnyElement> = Vec::new();

        if logs.is_empty() {
            log_rows.push(
                div()
                    .py(px(20.0))
                    .text_sm()
                    .text_color(Theme::text_muted())
                    .child("No API logs yet. Generate a logo to see requests here.")
                    .into_any_element(),
            );
        }

        for log in &logs {
            let is_error = log.status == "error";
            let status_color = if is_error { Theme::red() } else { Theme::green() };

            // Truncate detail for display
            let detail_display = if log.detail.len() > 200 {
                format!("{}...", &log.detail[..200])
            } else {
                log.detail.clone()
            };

            let row = div()
                .w_full()
                .p(px(10.0))
                .mb(px(6.0))
                .bg(Theme::bg_panel())
                .rounded(px(4.0))
                .border_1()
                .border_color(Theme::border())
                .flex()
                .flex_col()
                .gap(px(4.0))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .justify_between()
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .gap(px(8.0))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(status_color)
                                        .child(log.status.to_uppercase()),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(Theme::text_secondary())
                                        .child(log.endpoint.clone()),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .gap(px(8.0))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(Theme::text_muted())
                                        .child(format!("{}ms", log.duration_ms)),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(Theme::text_muted())
                                        .child(log.timestamp.clone()),
                                ),
                        ),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(Theme::text_secondary())
                        .child(format!("Prompt: {}", log.prompt)),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(if is_error { Theme::red() } else { Theme::text_muted() })
                        .child(detail_display),
                );

            log_rows.push(row.into_any_element());
        }

        div()
            .flex()
            .flex_col()
            .flex_grow()
            .child(
                div()
                    .px(px(32.0))
                    .pt(px(32.0))
                    .pb(px(16.0))
                    .flex()
                    .flex_row()
                    .justify_between()
                    .items_center()
                    .child(div().text_xl().text_color(Theme::text_primary()).child("API Logs"))
                    .child(
                        div()
                            .id("logs-back-btn")
                            .px(px(14.0))
                            .py(px(6.0))
                            .rounded(px(4.0))
                            .bg(Theme::button_bg())
                            .hover(|s| s.bg(Theme::button_hover()))
                            .cursor_pointer()
                            .child(div().text_sm().text_color(Theme::text_secondary()).child("Back to Design"))
                            .on_click(cx.listener(|this, _ev: &ClickEvent, _window, _cx| {
                                this.page = Page::Design;
                            })),
                    ),
            )
            .child(
                div()
                    .id("logs-scroll")
                    .flex_grow()
                    .overflow_y_scroll()
                    .px(px(32.0))
                    .pb(px(32.0))
                    .children(log_rows),
            )
            .into_any_element()
    }

    fn render_history_page(
        &self,
        sessions: Vec<crate::state::Session>,
        image_counts: std::collections::HashMap<String, usize>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let state_entity = self.state.clone();
        let mut rows: Vec<AnyElement> = Vec::new();

        if sessions.is_empty() {
            rows.push(
                div()
                    .py(px(20.0))
                    .text_sm()
                    .text_color(Theme::text_muted())
                    .child("No sessions yet. Create one to get started.")
                    .into_any_element(),
            );
        }

        for session in &sessions {
            let sid = session.id.clone();
            let count = image_counts.get(&session.id).copied().unwrap_or(0);
            let state_click = state_entity.clone();

            let created = &session.created_at;
            let updated = &session.updated_at;

            // Format dates: show just date + time portion
            let created_short = if created.len() >= 16 { &created[..16] } else { created };
            let updated_short = if updated.len() >= 16 { &updated[..16] } else { updated };

            let row = div()
                .id(SharedString::from(format!("hist-{}", sid)))
                .w_full()
                .p(px(12.0))
                .mb(px(6.0))
                .bg(Theme::bg_panel())
                .rounded(px(6.0))
                .border_1()
                .border_color(Theme::border())
                .hover(|s| s.bg(Theme::button_hover()))
                .cursor_pointer()
                .flex()
                .flex_row()
                .justify_between()
                .items_center()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(4.0))
                        .child(
                            div()
                                .text_sm()
                                .text_color(Theme::text_primary())
                                .child(session.name.clone()),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .gap(px(12.0))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(Theme::text_muted())
                                        .child(format!("Created: {}", created_short)),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(Theme::text_muted())
                                        .child(format!("Updated: {}", updated_short)),
                                ),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .gap(px(12.0))
                        .items_center()
                        .child(
                            div()
                                .text_sm()
                                .text_color(Theme::purple())
                                .child(format!("{} image{}", count, if count == 1 { "" } else { "s" })),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(Theme::text_secondary())
                                .child("Open →"),
                        ),
                )
                .on_click(cx.listener(move |this, _ev: &ClickEvent, _window, cx| {
                    state_click.update(cx, |s, _cx| {
                        s.select_session(&sid);
                    });
                    this.page = Page::Design;
                }));

            rows.push(row.into_any_element());
        }

        div()
            .flex()
            .flex_col()
            .flex_grow()
            .child(
                div()
                    .px(px(32.0))
                    .pt(px(32.0))
                    .pb(px(16.0))
                    .flex()
                    .flex_row()
                    .justify_between()
                    .items_center()
                    .child(div().text_xl().text_color(Theme::text_primary()).child("Session History"))
                    .child(
                        div()
                            .id("history-back-btn")
                            .px(px(14.0))
                            .py(px(6.0))
                            .rounded(px(4.0))
                            .bg(Theme::button_bg())
                            .hover(|s| s.bg(Theme::button_hover()))
                            .cursor_pointer()
                            .child(div().text_sm().text_color(Theme::text_secondary()).child("Back to Design"))
                            .on_click(cx.listener(|this, _ev: &ClickEvent, _window, _cx| {
                                this.page = Page::Design;
                            })),
                    ),
            )
            .child(
                div()
                    .id("history-scroll")
                    .flex_grow()
                    .overflow_y_scroll()
                    .px(px(32.0))
                    .pb(px(32.0))
                    .children(rows),
            )
            .into_any_element()
    }
}
