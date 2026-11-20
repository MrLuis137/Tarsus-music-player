mod audio;

use audio::AudioHandle;
use tauri::{path::BaseDirectory, AppHandle, Manager, State};

#[tauri::command]
fn play_test_file(app: AppHandle, audio: State<AudioHandle>) -> Result<(), String> {
    let path = app
        .path()
        .resolve("test.flac", BaseDirectory::Resource)
        .map_err(|e| e.to_string())?;
    audio.play(path)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Workaround Wayland + NVIDIA: WebKitGTK falla con
    // "Error 71 dispatching to Wayland display" usando DMABUF.
    #[cfg(target_os = "linux")]
    {
        if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
            #[allow(unused_unsafe)]
            unsafe {
                std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
            }
        }
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            app.manage(AudioHandle::spawn(app.handle().clone()));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![play_test_file])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}