# Neo AirPlay
[![Banner](/image/banner.png)](https://github.com/neo-airplay/neo-airplay)

Neo AirPlay 可在 Windows、macOS 與 Linux 電腦上接收 iPhone、iPad 畫面，也能顯示 Android 手機畫面。兩種畫面可以同時使用。

## 安裝檔下載連結
- [下載連結](https://github.com/Benknightdark/neo-airplay/releases)

## 常用指令

第一次執行時會先準備所需工具，可能需要網路或電腦管理員協助。

桌面程式的「工具管理」頁可以升級投影工具與平台相依套件，或清除 Neo AirPlay 專用工具後重新安裝。重裝會先停止投影服務，並保留接收器設定、診斷記錄與系統套件。Arch Linux 的 pacman 更新可能同時更新整個系統。

### macOS 與 Linux

試跑程式：

```sh
bash desktop/scripts/launch.sh dev
```

建立正式程式：

```sh
bash desktop/scripts/launch.sh release
```

製作安裝檔：

```sh
bash desktop/scripts/launch.sh bundle
```

### Windows

試跑程式：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\desktop\scripts\launch.ps1 dev
```

建立正式程式：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\desktop\scripts\launch.ps1 release
```

製作安裝檔：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\desktop\scripts\launch.ps1 bundle
```

## 執行測試

分別安裝應用程式與測試專案的套件：

```sh
(cd desktop && bun install --frozen-lockfile --silent)
(cd desktop-test && bun install --frozen-lockfile --silent)
```

執行完整型別、單元測試、Rust 檢查與前端建置：

```sh
bash desktop/scripts/verify.sh
```

Windows 請執行：

```powershell
pwsh desktop/scripts/verify.ps1
```

執行前端與後端單元測試：

```sh
cd desktop-test && bun run --silent verify
```

建置並執行桌面端對端測試：

```sh
cd desktop-test && bun run --silent test:e2e:ci
```
