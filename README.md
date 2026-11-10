# Neo AirPlay
[![Banner](/image/banner.png)](https://github.com/neo-airplay/neo-airplay)

Neo AirPlay 可在 Windows、macOS 與 Linux 電腦上接收 iPhone、iPad 畫面，也能顯示 Android 手機畫面。兩種畫面可以同時使用。

## 安裝檔下載連結
- [下載連結](https://github.com/Benknightdark/neo-airplay/releases)

## 常用指令

第一次執行時會先準備所需工具，可能需要網路或電腦管理員協助。

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
