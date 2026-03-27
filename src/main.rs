use std::path::PathBuf;
use std::rc::Rc;

use gpui::*;

mod db;
mod fal;
mod state;
mod text_input;
mod theme;
mod workspace;

use state::{AppState, GenerationStatus, ImageRecord, LogEntry};
use text_input::TextInput;
use workspace::{Page, Workspace};

fn data_dir() -> PathBuf {
    if let Some(proj_dirs) = directories_next::ProjectDirs::from("", "", "logo-designer") {
        proj_dirs.data_dir().to_path_buf()
    } else {
        PathBuf::from("./data")
    }
}

fn main() {
    Application::new().run(move |cx: &mut App| {
        cx.bind_keys(text_input::key_bindings());

        let data = data_dir();
        let db_path = data.join("logo-designer.db");
        let images_path = data.join("images");

        let state = cx.new(|_cx| {
            AppState::new(
                db_path.to_str().unwrap_or("logo-designer.db"),
                images_path,
            )
        });

        let prompt_input = cx.new(|cx| {
            let mut input = TextInput::new(cx);
            input.placeholder = "Describe your logo... e.g. 'Minimalist mountain logo for a hiking app called Trailblaze'".into();
            input
        });

        let evolve_input = cx.new(|cx| {
            let mut input = TextInput::new(cx);
            input.placeholder = "Describe changes... e.g. 'Make it gold, add more detail'".into();
            input
        });

        let api_key_input = cx.new(|cx| {
            let mut input = TextInput::new(cx);
            input.placeholder = "Paste your FAL API key here...".into();
            input
        });

        // Check if key is set, if not go to settings
        let needs_key = state.read(cx).fal_key.is_none();

        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: Point {
                        x: px(100.0),
                        y: px(100.0),
                    },
                    size: Size {
                        width: px(1200.0),
                        height: px(800.0),
                    },
                })),
                titlebar: Some(TitlebarOptions {
                    title: Some("Logo Designer".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |_window, cx| {
                let workspace = cx.new(|cx| {
                    let mut ws = Workspace::new(
                        state.clone(),
                        prompt_input.clone(),
                        evolve_input.clone(),
                        api_key_input.clone(),
                        cx,
                    );
                    if needs_key {
                        ws.page = Page::Settings;
                    }
                    ws
                });

                // Wire up generate callback
                let gen_state = state.clone();
                let gen_prompt = prompt_input.clone();

                workspace.update(cx, |ws, _cx| {
                    ws.on_generate = Some(Rc::new(move |_window: &mut Window, cx: &mut App| {
                        let prompt_text = gen_prompt.read(cx).content().to_string();
                        if prompt_text.trim().is_empty() {
                            return;
                        }

                        let fal_key = gen_state.read(cx).fal_key.clone();
                        let Some(fal_key) = fal_key else {
                            gen_state.update(cx, |s, _cx| {
                                s.status = GenerationStatus::Error("No API key set".to_string());
                            });
                            return;
                        };

                        let session_id = gen_state.read(cx).active_session_id.clone();
                        let Some(session_id) = session_id else { return };

                        let style = gen_state.read(cx).style_value().to_string();
                        let images_path = gen_state.read(cx).images_path.clone();

                        gen_state.update(cx, |s, _cx| {
                            s.status = GenerationStatus::Generating;
                        });

                        let state_done = gen_state.clone();
                        let prompt_clone = prompt_text.clone();

                        cx.spawn(async move |cx: &mut AsyncApp| {
                            let result = cx
                                .background_executor()
                                .spawn(async move {
                                    let start = std::time::Instant::now();
                                    let endpoint = "fal-ai/recraft/v4/pro/text-to-vector".to_string();
                                    let gen_result = fal::generate_logo(&fal_key, &prompt_clone, Some(&style));
                                    let duration_ms = start.elapsed().as_millis() as u64;

                                    let log = LogEntry {
                                        id: uuid::Uuid::new_v4().to_string(),
                                        timestamp: chrono::Utc::now().to_rfc3339(),
                                        endpoint: endpoint.clone(),
                                        prompt: prompt_clone.clone(),
                                        status: if gen_result.is_ok() { "success".to_string() } else { "error".to_string() },
                                        detail: match &gen_result {
                                            Ok(res) => format!("url={}, content_type={}", res.url, res.content_type),
                                            Err(e) => e.clone(),
                                        },
                                        duration_ms,
                                    };

                                    match gen_result {
                                        Ok(res) => {
                                            let image_id = uuid::Uuid::new_v4().to_string();
                                            let (filename, file_type) = if res.content_type.contains("svg") {
                                                (format!("{}.svg", image_id), "svg".to_string())
                                            } else {
                                                (format!("{}.png", image_id), "png".to_string())
                                            };

                                            let save_path = images_path.join(&filename);
                                            fal::download_image(&res.url, &save_path)?;

                                            Ok((Some(ImageRecord {
                                                id: image_id,
                                                session_id,
                                                prompt: prompt_text,
                                                model: "recraft-v4-text-to-vector".to_string(),
                                                parent_image_id: None,
                                                filename,
                                                file_type,
                                                created_at: chrono::Utc::now().to_rfc3339(),
                                            }), log))
                                        }
                                        Err(e) => Ok((None, log)),
                                    }
                                })
                                .await;

                            let _ = cx.update(|cx| {
                                match result {
                                    Ok((Some(img), log)) => {
                                        state_done.update(cx, |s, _cx| {
                                            s.add_log(log);
                                            s.add_image(img);
                                            s.status = GenerationStatus::Idle;
                                        });
                                    }
                                    Ok((None, log)) => {
                                        let err = log.detail.clone();
                                        state_done.update(cx, |s, _cx| {
                                            s.add_log(log);
                                            s.status = GenerationStatus::Error(err);
                                        });
                                    }
                                    Err(e) => {
                                        state_done.update(cx, |s, _cx| {
                                            s.status = GenerationStatus::Error(e);
                                        });
                                    }
                                }
                            });
                        })
                        .detach();
                    }));
                });

                // Wire up evolve callback
                let ev_state = state.clone();
                let ev_evolve_input = evolve_input.clone();

                workspace.update(cx, |ws, _cx| {
                    ws.on_evolve = Some(Rc::new(move |_window: &mut Window, cx: &mut App| {
                        let evolve_text = ev_evolve_input.read(cx).content().to_string();
                        if evolve_text.trim().is_empty() {
                            return;
                        }

                        let fal_key = ev_state.read(cx).fal_key.clone();
                        let Some(fal_key) = fal_key else {
                            ev_state.update(cx, |s, _cx| {
                                s.status = GenerationStatus::Error("No API key set".to_string());
                            });
                            return;
                        };

                        let selected = ev_state.read(cx).selected_image().cloned();
                        let Some(selected) = selected else { return };

                        let session_id = ev_state.read(cx).active_session_id.clone();
                        let Some(session_id) = session_id else { return };

                        let images_path = ev_state.read(cx).images_path.clone();
                        let parent_path = images_path.join(&selected.filename);

                        ev_state.update(cx, |s, _cx| {
                            s.status = GenerationStatus::Generating;
                        });

                        let state_done = ev_state.clone();
                        let parent_id = selected.id.clone();
                        let prompt_text = evolve_text.clone();
                        let evolve_input_clear = ev_evolve_input.clone();

                        cx.spawn(async move |cx: &mut AsyncApp| {
                            let result = cx
                                .background_executor()
                                .spawn(async move {
                                    // Read and base64 encode parent image
                                    let image_bytes = std::fs::read(&parent_path)
                                        .map_err(|e| format!("Failed to read image: {}", e))?;

                                    let mime = if selected.file_type == "svg" {
                                        "image/svg+xml"
                                    } else {
                                        "image/png"
                                    };
                                    let b64 = base64::Engine::encode(
                                        &base64::engine::general_purpose::STANDARD,
                                        &image_bytes,
                                    );
                                    let data_uri = format!("data:{};base64,{}", mime, b64);

                                    let res = fal::evolve_logo(&fal_key, &prompt_text, &data_uri)?;

                                    let image_id = uuid::Uuid::new_v4().to_string();
                                    let filename = format!("{}.png", image_id);
                                    let save_path = images_path.join(&filename);
                                    fal::download_image(&res.url, &save_path)?;

                                    Ok::<ImageRecord, String>(ImageRecord {
                                        id: image_id,
                                        session_id,
                                        prompt: evolve_text,
                                        model: "flux-kontext-pro".to_string(),
                                        parent_image_id: Some(parent_id),
                                        filename,
                                        file_type: "png".to_string(),
                                        created_at: chrono::Utc::now().to_rfc3339(),
                                    })
                                })
                                .await;

                            let _ = cx.update(|cx| {
                                match result {
                                    Ok(img) => {
                                        state_done.update(cx, |s, _cx| {
                                            s.add_image(img);
                                            s.status = GenerationStatus::Idle;
                                        });
                                        evolve_input_clear.update(cx, |input, cx| {
                                            input.set_content(String::new(), cx);
                                        });
                                    }
                                    Err(e) => {
                                        state_done.update(cx, |s, _cx| {
                                            s.status = GenerationStatus::Error(e);
                                        });
                                    }
                                }
                            });
                        })
                        .detach();
                    }));
                });

                // Wire up save key callback
                let key_state = state.clone();
                let key_input = api_key_input.clone();
                let key_workspace = workspace.clone();

                workspace.update(cx, |ws, _cx| {
                    ws.on_save_key = Some(Rc::new(move |_window: &mut Window, cx: &mut App| {
                        let key_text = key_input.read(cx).content().to_string();
                        if key_text.trim().is_empty() {
                            return;
                        }
                        key_state.update(cx, |s, _cx| {
                            s.save_fal_key(key_text.trim());
                        });
                        key_input.update(cx, |input, cx| {
                            input.set_content(String::new(), cx);
                        });
                        key_workspace.update(cx, |ws, _cx| {
                            ws.page = Page::Design;
                        });
                    }));
                });

                workspace
            },
        )
        .unwrap();
    });
}
