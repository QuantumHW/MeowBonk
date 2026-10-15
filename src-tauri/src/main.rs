#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod platform;
mod settings;
mod target;

use serde::{Deserialize, Serialize};
use std::sync::{
    atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
    mpsc, Arc, Mutex,
};
use std::thread::{self, JoinHandle};
use tauri::{
    menu::{ContextMenu, Menu, MenuItem},
    Emitter, Manager,
};

struct Worker {
    stop: Arc<AtomicBool>,
    input: JoinHandle<()>,
    dispatch: JoinHandle<()>,
}
impl Worker {
    fn stop(self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = self.input.join();
        let _ = self.dispatch.join();
    }
}
#[derive(Clone, Copy)]
struct Screen {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    scale: f64,
}
#[derive(Clone, Copy, Deserialize)]
struct PetBounds {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    flipped: bool,
}
struct AppState {
    worker: Mutex<Option<Worker>>,
    preferences: Mutex<settings::Store>,
    status: platform::Status,
    screen: Screen,
    pet: Arc<Mutex<Option<PetBounds>>>,
    exiting: Arc<AtomicBool>,
    overlay_id: AtomicU32,
    generation: Arc<AtomicU64>,
    collision: platform::Status,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Snapshot {
    listening: bool,
    status: String,
    permission: bool,
    input_permission: bool,
    accessibility_permission: bool,
    screen_permission: bool,
    collision: String,
    platform: &'static str,
    version: &'static str,
    diagnostics: Option<[u64; 6]>,
}
#[derive(Clone, Serialize)]
struct Bonk {
    x: f64,
    y: f64,
    target: Option<target::Target>,
}

fn require_settings(window: &tauri::WebviewWindow) -> Result<(), String> {
    if window.label() == "settings" {
        Ok(())
    } else {
        Err("此操作仅允许设置窗口调用".into())
    }
}
#[tauri::command]
fn get_status(
    window: tauri::WebviewWindow,
    state: tauri::State<AppState>,
) -> Result<Snapshot, String> {
    require_settings(&window)?;
    let status = state.status.lock().map_err(|_| "无法读取监听状态")?.clone();
    Ok(Snapshot {
        listening: status == "监听中",
        status,
        permission: platform::permission(),
        input_permission: platform::input_permission(),
        accessibility_permission: platform::accessibility_permission(),
        screen_permission: target::screen_permission(false),
        collision: state
            .collision
            .lock()
            .map_err(|_| "无法读取碰撞状态")?
            .clone(),
        platform: std::env::consts::OS,
        version: env!("CARGO_PKG_VERSION"),
        diagnostics: platform::diagnostics(),
    })
}
fn pause(app: &tauri::AppHandle) {
    let state = app.state::<AppState>();
    state.generation.fetch_add(1, Ordering::Relaxed);
    if let Ok(mut slot) = state.worker.lock() {
        if let Some(worker) = slot.take() {
            worker.stop();
        }
    }
    platform::report(&state.status, "已暂停");
    let _ = app.emit_to("overlay", "pause", ());
}
fn start(app: &tauri::AppHandle, request_permissions: bool) -> Result<(), String> {
    let state = app.state::<AppState>();
    if !platform::permission() && (!request_permissions || !platform::request_permission()) {
        platform::report(
            &state.status,
            "请在系统设置 → 隐私与安全性中允许 MeowBonk 的输入监控与辅助功能，然后重新启用",
        );
        return Ok(());
    }
    let mut slot = state.worker.lock().map_err(|_| "无法读取监听任务")?;
    if slot.as_ref().is_some_and(|w| !w.input.is_finished()) {
        return Ok(());
    }
    if let Some(old) = slot.take() {
        old.stop();
    }
    let stop = Arc::new(AtomicBool::new(false));
    let (sender, receiver) = mpsc::sync_channel(1);
    let native_stop = stop.clone();
    let status = state.status.clone();
    platform::report(&status, "正在启动监听");
    let factor = if cfg!(target_os = "macos") {
        state.screen.scale
    } else {
        1.0
    };
    let region = platform::InputRegion {
        left: state.screen.x / factor,
        top: state.screen.y / factor,
        width: state.screen.width * state.screen.scale / factor,
        height: state.screen.height * state.screen.scale / factor,
    };
    let input = thread::spawn(move || platform::run(sender, native_stop, status, region));
    let dispatch_stop = stop.clone();
    let handle = app.clone();
    let screen = state.screen;
    let overlay_id = state.overlay_id.load(Ordering::Relaxed);
    let collision = state.collision.clone();
    let generation = state.generation.clone();
    let dispatch = thread::spawn(move || {
        let mut last = std::time::Instant::now() - std::time::Duration::from_secs(1);
        while !dispatch_stop.load(Ordering::Relaxed) {
            match receiver.recv_timeout(std::time::Duration::from_millis(100)) {
                Ok(trigger) => {
                    if !trigger.fresh() {
                        continue;
                    }
                    let point = trigger.point;
                    if dispatch_stop.load(Ordering::Relaxed) {
                        break;
                    }
                    if last.elapsed().as_millis() < 150 {
                        continue;
                    }
                    if let Some((x, y)) = platform::local_point(
                        point,
                        (screen.x, screen.y),
                        screen.scale,
                        cfg!(target_os = "macos"),
                        (screen.width, screen.height),
                    ) {
                        last = std::time::Instant::now();
                        let ticket = generation.fetch_add(1, Ordering::Relaxed) + 1;
                        let target = target::capture(
                            x,
                            y,
                            screen.width,
                            screen.height,
                            screen.scale,
                            overlay_id,
                        );
                        if dispatch_stop.load(Ordering::Relaxed) {
                            break;
                        }
                        if !trigger.fresh() {
                            continue;
                        }
                        if generation.load(Ordering::Relaxed) != ticket {
                            continue;
                        }
                        platform::report(&collision, target.status());
                        if target.surface.is_none() {
                            continue;
                        }
                        if handle
                            .emit_to(
                                "overlay",
                                "bonk",
                                Bonk {
                                    x,
                                    y,
                                    target: Some(target),
                                },
                            )
                            .is_ok()
                        {
                            platform::count(5);
                        }
                    } else {
                        platform::count(4);
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
        }
    });
    *slot = Some(Worker {
        stop,
        input,
        dispatch,
    });
    Ok(())
}
#[tauri::command]
async fn set_listening(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    enabled: bool,
) -> Result<(), String> {
    require_settings(&window)?;
    apply_enabled(&app, enabled)
}
fn apply_enabled(app: &tauri::AppHandle, enabled: bool) -> Result<(), String> {
    // Pausing takes effect even if the disk is unavailable. Exit cleanup calls pause directly
    // and must never overwrite the user's saved enabled preference.
    if !enabled {
        pause(app);
    }
    app.state::<AppState>()
        .preferences
        .lock()
        .map_err(|_| "无法更新设置")?
        .set_enabled(enabled)?;
    if enabled {
        start(app, true)
    } else {
        Ok(())
    }
}
#[derive(Deserialize)]
struct DemoPoint {
    x: f64,
    y: f64,
}
#[tauri::command]
async fn demo(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    point: DemoPoint,
) -> Result<(), String> {
    require_settings(&window)?;
    let state = app.state::<AppState>();
    let size = window.inner_size().map_err(|e| e.to_string())?;
    let scale = window.scale_factor().map_err(|e| e.to_string())?;
    if !point.x.is_finite()
        || !point.y.is_finite()
        || point.x < 0.
        || point.y < 0.
        || point.x > size.width as f64 / scale
        || point.y > size.height as f64 / scale
    {
        return Err("演示位置无效".into());
    }
    let origin = window.inner_position().map_err(|e| e.to_string())?;
    let screen = state.screen;
    let x = (origin.x as f64 - screen.x) / screen.scale + point.x;
    let y = (origin.y as f64 - screen.y) / screen.scale + point.y;
    let excluded = state.overlay_id.load(Ordering::Relaxed);
    let ticket = state.generation.fetch_add(1, Ordering::Relaxed) + 1;
    let target = tauri::async_runtime::spawn_blocking(move || {
        target::capture(x, y, screen.width, screen.height, screen.scale, excluded)
    })
    .await
    .map_err(|e| e.to_string())?;
    if state.generation.load(Ordering::Relaxed) != ticket {
        return Ok(());
    }
    platform::report(&state.collision, target.status());
    if target.surface.is_none() {
        return Err(target.status());
    }
    app.emit_to(
        "overlay",
        "bonk",
        Bonk {
            x,
            y,
            target: Some(target),
        },
    )
    .map_err(|e| e.to_string())
}
#[tauri::command]
fn report_visual_failure(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
) -> Result<(), String> {
    if window.label() != "overlay" {
        return Err("仅角色窗口可报告画面状态".into());
    }
    platform::report(
        &app.state::<AppState>().collision,
        "局部画面无法加载，本次未播放撞击；请重试",
    );
    Ok(())
}
#[tauri::command]
fn enable_screen_capture(window: tauri::WebviewWindow) -> Result<bool, String> {
    require_settings(&window)?;
    Ok(target::screen_permission(true))
}
#[tauri::command]
fn quit(window: tauri::WebviewWindow, app: tauri::AppHandle) -> Result<(), String> {
    require_settings(&window)?;
    app.exit(0);
    Ok(())
}
#[tauri::command]
fn pet_ready(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
) -> Result<(), String> {
    if window.label() != "overlay" {
        return Err("仅角色窗口可调用".into());
    }
    window.show().map_err(|e| e.to_string())?;
    #[cfg(target_os = "macos")]
    state.overlay_id.store(
        target::window_id(window.ns_window().map_err(|e| e.to_string())?),
        Ordering::Relaxed,
    );
    let enabled = state
        .preferences
        .lock()
        .map_err(|_| "无法读取设置")?
        .preferences
        .listening_enabled;
    if enabled {
        start(&app, false)?;
    }
    Ok(())
}
#[tauri::command]
fn set_pet_bounds(
    window: tauri::WebviewWindow,
    state: tauri::State<AppState>,
    bounds: Option<PetBounds>,
) -> Result<(), String> {
    if window.label() != "overlay" {
        return Err("仅角色窗口可调用".into());
    }
    if let Some(b) = bounds {
        if ![b.x, b.y, b.width, b.height].iter().all(|n| n.is_finite())
            || b.width < 20.0
            || b.width > 600.0
            || b.height < 20.0
            || b.height > 600.0
            || b.x < 0.0
            || b.y < 0.0
            || b.x + b.width > state.screen.width + 1.0
            || b.y + b.height > state.screen.height + 1.0
        {
            return Err("角色范围无效".into());
        }
    }
    *state.pet.lock().map_err(|_| "无法更新角色范围")? = bounds;
    Ok(())
}
fn show_settings(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}
#[tauri::command]
fn pet_menu(window: tauri::WebviewWindow, app: tauri::AppHandle) -> Result<(), String> {
    if window.label() != "overlay" {
        return Err("仅角色窗口可调用".into());
    }
    let listening = app
        .state::<AppState>()
        .status
        .lock()
        .map_err(|_| "无法读取状态")?
        .as_str()
        == "监听中";
    let settings = MenuItem::with_id(&app, "settings", "设置与使用说明", true, None::<&str>)
        .map_err(|e| e.to_string())?;
    let toggle = MenuItem::with_id(
        &app,
        if listening { "pause" } else { "start" },
        if listening {
            "暂停召唤"
        } else {
            "启用召唤"
        },
        true,
        None::<&str>,
    )
    .map_err(|e| e.to_string())?;
    let about = MenuItem::with_id(&app, "about", "关于 MeowBonk", true, None::<&str>)
        .map_err(|e| e.to_string())?;
    let exit =
        MenuItem::with_id(&app, "quit", "退出", true, None::<&str>).map_err(|e| e.to_string())?;
    let menu =
        Menu::with_items(&app, &[&settings, &toggle, &about, &exit]).map_err(|e| e.to_string())?;
    menu.popup(window.as_ref().window().clone())
        .map_err(|e| e.to_string())
}

fn main() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            show_settings(app)
        }))
        .invoke_handler(tauri::generate_handler![
            get_status,
            enable_screen_capture,
            report_visual_failure,
            set_listening,
            demo,
            quit,
            pet_ready,
            set_pet_bounds,
            pet_menu
        ])
        .on_menu_event(|app, event| match event.id().as_ref() {
            "settings" | "about" => show_settings(app),
            "pause" | "start" => {
                if let Err(error) = apply_enabled(app, event.id().as_ref() == "start") {
                    platform::report(&app.state::<AppState>().status, error);
                }
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                if window.label() == "settings" {
                    let _ = window.hide();
                }
            }
        })
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            let monitor = app.primary_monitor()?.ok_or("找不到主显示器")?;
            let scale = monitor.scale_factor();
            let position = monitor.position();
            let size = monitor.size();
            let screen = Screen {
                x: position.x as f64,
                y: position.y as f64,
                width: size.width as f64 / scale,
                height: size.height as f64 / scale,
                scale,
            };
            let pet = Arc::new(Mutex::new(None));
            let exiting = Arc::new(AtomicBool::new(false));
            let settings_path = app.path().app_config_dir()?.join("settings.json");
            let (preferences, initial_status) = match settings::load(&settings_path) {
                Ok(p) => (
                    p,
                    if p.listening_enabled {
                        "正在恢复召唤".into()
                    } else {
                        "已暂停".into()
                    },
                ),
                Err(error) => (settings::Preferences::default(), error),
            };
            app.manage(AppState {
                worker: Mutex::new(None),
                preferences: Mutex::new(settings::Store {
                    path: settings_path,
                    preferences,
                }),
                status: Arc::new(Mutex::new(initial_status)),
                screen,
                pet: pet.clone(),
                exiting: exiting.clone(),
                overlay_id: AtomicU32::new(0),
                generation: Arc::new(AtomicU64::new(0)),
                collision: Arc::new(Mutex::new("等待选择目标".into())),
            });
            let overlay = tauri::WebviewWindowBuilder::new(
                app,
                "overlay",
                tauri::WebviewUrl::App("index.html?overlay".into()),
            )
            .title("MeowBonk 小猫")
            .position(screen.x / scale, screen.y / scale)
            .inner_size(screen.width, screen.height)
            .transparent(true)
            .decorations(false)
            .closable(false)
            .minimizable(false)
            .maximizable(false)
            .resizable(false)
            .shadow(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .focusable(false)
            .focused(false)
            .visible(false)
            .build()?;
            overlay.set_ignore_cursor_events(true)?;
            #[cfg(target_os = "macos")]
            app.state::<AppState>()
                .overlay_id
                .store(target::window_id(overlay.ns_window()?), Ordering::Relaxed);
            let image =
                tauri::image::Image::from_bytes(include_bytes!("../../public/cat-standing.png"))?;
            let rgba = image.rgba().to_vec();
            let iw = image.width() as usize;
            let ih = image.height() as usize;
            let handle = app.handle().clone();
            // 仅静止角色的不透明像素接收点击，透明区域和动画期间全部穿透。
            thread::spawn(move || {
                let mut last_hit = false;
                while !exiting.load(Ordering::Relaxed) {
                    let bounds = pet.lock().ok().and_then(|v| *v);
                    let hit = platform::cursor()
                        .and_then(|p| {
                            platform::local_point(
                                p,
                                (screen.x, screen.y),
                                scale,
                                cfg!(target_os = "macos"),
                                (screen.width, screen.height),
                            )
                        })
                        .zip(bounds)
                        .is_some_and(|((x, y), b)| {
                            let u = (x - b.x) / b.width;
                            let v = (y - b.y) / b.height;
                            if !(0.0..1.0).contains(&u) || !(0.0..1.0).contains(&v) {
                                return false;
                            }
                            let px = ((if b.flipped { 1.0 - u } else { u }) * iw as f64)
                                .floor()
                                .min((iw - 1) as f64) as usize;
                            let py = (v * ih as f64).floor().min((ih - 1) as f64) as usize;
                            rgba[(py * iw + px) * 4 + 3] > 80
                        });
                    if hit != last_hit {
                        last_hit = hit;
                        let app = handle.clone();
                        let _ = handle.run_on_main_thread(move || {
                            if let Some(w) = app.get_webview_window("overlay") {
                                let _ = w.set_ignore_cursor_events(!hit);
                            }
                        });
                    }
                    thread::sleep(std::time::Duration::from_millis(33));
                }
            });
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("MeowBonk 启动失败");
    app.run(|app, event| {
        if matches!(event, tauri::RunEvent::Exit) {
            app.state::<AppState>()
                .exiting
                .store(true, Ordering::Relaxed);
            pause(app);
        }
    });
}
