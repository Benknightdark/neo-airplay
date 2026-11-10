use std::{env, thread, time::Duration};

fn main() {
    let name = env::current_exe()
        .ok()
        .and_then(|path| {
            path.file_stem()
                .map(|stem| stem.to_string_lossy().to_ascii_lowercase())
        })
        .unwrap_or_default();

    if name == "adb" {
        let profile_directory = env::var_os("NEO_AIRPLAY_E2E_PROFILE")
            .map(std::path::PathBuf::from)
            .or_else(|| {
                env::current_exe().ok().and_then(|path| {
                    path.parent()
                        .and_then(|engine| engine.parent())
                        .map(|profile| profile.to_path_buf())
                })
            });
        if let Some(profile_directory) = profile_directory {
            let block_path = profile_directory.join("adb.block");
            if block_path.exists() {
                std::fs::write(profile_directory.join("adb.started"), b"waiting")
                    .expect("無法建立 ADB 等待測試訊號。");
                while block_path.exists() {
                    thread::sleep(Duration::from_millis(20));
                }
            }
        }
        println!("List of devices attached");
        println!("neo-e2e-device device product:neo model:Test_Pixel");
        return;
    }

    loop {
        thread::sleep(Duration::from_secs(1));
    }
}
