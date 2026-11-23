#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! DasIDM desktop shell. The download core keeps running in the tray; the WebView window
//! is destroyed whenever it is closed so an idle DasIDM holds almost no memory.

use das_core::{AddRequest, ItemView, Manager, ProbeInfo, Settings};
use std::sync::Arc;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::window::{Effect, EffectsBuilder};
use tauri::{AppHandle, Emitter, Manager as _, RunEvent, State, Theme, WebviewUrl, WebviewWindowBuilder, WindowEvent};

struct App {
    core: Manager,
}

type Res<T> = Result<T, String>;

// Commands that may start downloads are async so they run on the tokio runtime.

#[tauri::command]
fn list_downloads(app: State<'_, App>) -> Vec<ItemView> {
    app.core.list()
}

#[tauri::command]
async fn probe_url(app: State<'_, App>, url: String) -> Res<ProbeInfo> {
    app.core.probe(url, vec![]).await
}

#[tauri::command]
async fn add_download(app: State<'_, App>, req: AddRequest) -> Res<ItemView> {
    app.core.add(req)
}

#[tauri::command]
async fn pause(app: State<'_, App>, ids: Vec<String>) -> Res<()> {
    app.core.pause(&ids);
    Ok(())
}

#[tauri::command]
async fn resume(app: State<'_, App>, ids: Vec<String>) -> Res<()> {
    app.core.resume(&ids);
    Ok(())
}

#[tauri::command]
async fn restart(app: State<'_, App>, ids: Vec<String>) -> Res<()> {
    app.core.restart(&ids);
    Ok(())
}

#[tauri::command]
async fn remove(app: State<'_, App>, ids: Vec<String>, delete_files: bool) -> Res<()> {
    app.core.remove(&ids, delete_files);
    Ok(())
}

#[tauri::command]
async fn pause_all(app: State<'_, App>) -> Res<()> {
    app.core.pause_all();
    Ok(())
}

#[tauri::command]
async fn resume_all(app: State<'_, App>) -> Res<()> {
    app.core.resume_all();
    Ok(())
}

#[tauri::command]
fn speed_history(app: State<'_, App>) -> Vec<u64> {
    app.core.speed_history()
}

#[tauri::command]
fn get_settings(app: State<'_, App>) -> Settings {
    app.core.settings()
}

#[tauri::command]
async fn set_settings(app: State<'_, App>, settings: Settings) -> Res<()> {
    app.core.set_settings(settings)
}

/// Open a finished file with its default program.
#[tauri::command]
fn open_file(app: State<'_, App>, id: String) -> Res<()> {
    let item = app.core.get(&id).ok_or("Download not found")?;
    let path = item.file_path.ok_or("The file is not finished yet")?;
    std::process::Command::new("explorer.exe").arg(path).spawn().map_err(|e| e.to_string())?;
    Ok(())
}

/// Show the file (or its folder) in Explorer.
#[tauri::command]
fn open_folder(app: State<'_, App>, id: String) -> Res<()> {
    use std::os::windows::process::CommandExt;
    let item = app.core.get(&id).ok_or("Download not found")?;
    let mut cmd = std::process::Command::new("explorer.exe");
    match item.file_path.filter(|p| std::path::Path::new(p).exists()) {
        Some(p) => cmd.raw_arg(format!("/select,\"{p}\"")),
        None => cmd.arg(&item.save_dir),
    };
    cmd.spawn().map_err(|e| e.to_string())?;
    Ok(())
}

fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }
    let built = WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
        .title("DasIDM")
        .inner_size(1280.0, 820.0)
        .min_inner_size(920.0, 580.0)
        .decorations(false)
        .transparent(true)
        .theme(Some(Theme::Dark))
        .effects(EffectsBuilder::new().effect(Effect::Mica).build())
        // Fewer helper processes and background services in WebView2. The glass is drawn by
        // the Windows compositor (Mica), so the page renders in software: measured ~111 MB
        // private with the window open instead of ~200 MB with GPU rendering.
        .additional_browser_args(
            "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection,Translate,AutofillServerCommunication,msEdgeShopping \
             --disable-background-networking --renderer-process-limit=1 \
             --disable-gpu --in-process-gpu --disable-gpu-compositing",
        )
        .build();
    if built.is_ok() {
        app.state::<App>().core.set_visible(true);
    }
}

/// Links passed on the command line (`dasidm.exe <url>…`) are added as downloads.
fn add_urls(app: &AppHandle, args: &[String]) {
    let urls: Vec<String> = args.iter().filter(|a| a.starts_with("http://") || a.starts_with("https://")).cloned().collect();
    if urls.is_empty() {
        return;
    }
    let core = app.state::<App>().core.clone();
    tauri::async_runtime::spawn(async move {
        for url in urls {
            let _ = core.add(AddRequest {
                url,
                file_name: None,
                save_dir: None,
                connections: None,
                headers: vec![],
                start: true,
                allow_page: false,
            });
        }
    });
}

fn quit(app: &AppHandle) {
    let core = app.state::<App>().core.clone();
    tauri::async_runtime::block_on(core.shutdown());
    app.exit(0);
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let start_hidden = args.iter().any(|a| a == "--background");
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            // A second launch hands its links to the running DasIDM.
            add_urls(app, &args);
            if !args.iter().any(|a| a == "--background") {
                show_main(app);
            }
        }))
        .invoke_handler(tauri::generate_handler![
            list_downloads,
            speed_history,
            probe_url,
            add_download,
            pause,
            resume,
            restart,
            remove,
            pause_all,
            resume_all,
            get_settings,
            set_settings,
            open_file,
            open_folder
        ])
        .setup(move |app| {
            let handle = app.handle().clone();
            // DASIDM_DATA_DIR runs a separate, portable profile (tests, USB sticks).
            let data_dir = match std::env::var_os("DASIDM_DATA_DIR") {
                Some(d) => std::path::PathBuf::from(d),
                None => app.path().app_data_dir()?,
            };
            let emitter = handle.clone();
            let core = Manager::open(&data_dir, Arc::new(move |e| {
                let _ = emitter.emit("core", e);
            }))?;
            if let Some(dir) = std::env::var_os("DASIDM_DOWNLOAD_DIR") {
                let mut s = core.settings();
                s.download_dir = dir.to_string_lossy().into_owned();
                let c = core.clone();
                tauri::async_runtime::block_on(async move { c.set_settings(s) })?;
            }
            tauri::async_runtime::spawn(core.clone().run());
            app.manage(App { core });
            add_urls(&handle, &args);

            let open = MenuItem::with_id(app, "open", "Open DasIDM", true, None::<&str>)?;
            let pause = MenuItem::with_id(app, "pause_all", "Pause all", true, None::<&str>)?;
            let resume = MenuItem::with_id(app, "resume_all", "Resume all", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quit DasIDM", true, None::<&str>)?;
            let sep = PredefinedMenuItem::separator(app)?;
            let menu = Menu::with_items(app, &[&open, &pause, &resume, &sep, &quit_item])?;
            TrayIconBuilder::with_id("tray")
                .icon(app.default_window_icon().cloned().ok_or("missing icon")?)
                .tooltip("DasIDM")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, ev| match ev.id().as_ref() {
                    "open" => show_main(app),
                    "pause_all" => app.state::<App>().core.pause_all(),
                    "resume_all" => {
                        let core = app.state::<App>().core.clone();
                        tauri::async_runtime::spawn(async move { core.resume_all() });
                    }
                    "quit" => quit(app),
                    _ => {}
                })
                .on_tray_icon_event(|tray, ev| {
                    if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = ev {
                        show_main(tray.app_handle());
                    }
                })
                .build(app)?;

            if !start_hidden {
                show_main(&handle);
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::Destroyed = event {
                if window.label() == "main" {
                    window.app_handle().state::<App>().core.set_visible(false);
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("failed to start DasIDM")
        .run(|app, event| {
            if let RunEvent::ExitRequested { code: None, api, .. } = event {
                // Last window closed: keep running in the tray unless the user turned that off.
                if app.state::<App>().core.settings().close_to_tray {
                    api.prevent_exit();
                } else {
                    let core = app.state::<App>().core.clone();
                    tauri::async_runtime::block_on(core.shutdown());
                }
            }
        });
}
