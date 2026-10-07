use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum Platform {
    Windows,
    Macos,
    Linux,
}

impl Platform {
    pub const fn current() -> Self {
        #[cfg(target_os = "windows")]
        {
            Self::Windows
        }
        #[cfg(target_os = "macos")]
        {
            Self::Macos
        }
        #[cfg(target_os = "linux")]
        {
            Self::Linux
        }
        #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
        {
            Self::Linux
        }
    }

    pub fn renderer_options(self) -> Vec<RendererOption> {
        let values: &[(&str, &str)] = match self {
            Self::Windows => &[
                ("Auto", "自動"),
                ("D3D11", "Direct3D 11"),
                ("D3D12", "Direct3D 12"),
            ],
            Self::Macos => &[("Auto", "自動"), ("GL", "OpenGL"), ("OSX", "macOS")],
            Self::Linux => &[
                ("Auto", "自動"),
                ("GL", "OpenGL"),
                ("Xv", "X11 Video"),
                ("Wayland", "Wayland"),
            ],
        };

        values
            .iter()
            .map(|(value, label)| RendererOption {
                value: (*value).to_owned(),
                label: (*label).to_owned(),
            })
            .collect()
    }

    pub fn sanitize_renderer(self, renderer: &str) -> String {
        if self
            .renderer_options()
            .iter()
            .any(|option| option.value == renderer)
        {
            renderer.to_owned()
        } else {
            "Auto".to_owned()
        }
    }

    pub fn video_sink(self, renderer: &str) -> Option<&'static str> {
        match (self, renderer) {
            (Self::Windows, "D3D11") => Some("d3d11videosink"),
            (Self::Windows, "D3D12") => Some("d3d12videosink"),
            (Self::Macos, "GL") => Some("glimagesink"),
            (Self::Macos, "OSX") => Some("osxvideosink"),
            (Self::Linux, "GL") => Some("glimagesink"),
            (Self::Linux, "Xv") => Some("xvimagesink"),
            (Self::Linux, "Wayland") => Some("waylandsink"),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
pub struct RendererOption {
    pub value: String,
    pub label: String,
}

#[cfg(test)]
mod tests {
    use super::Platform;

    #[test]
    fn renderer_values_match_platform_support() {
        assert_eq!(
            Platform::Windows.video_sink("D3D11"),
            Some("d3d11videosink")
        );
        assert_eq!(Platform::Macos.video_sink("OSX"), Some("osxvideosink"));
        assert_eq!(Platform::Linux.video_sink("Wayland"), Some("waylandsink"));
        assert_eq!(Platform::Linux.video_sink("D3D11"), None);
    }

    #[test]
    fn unsupported_renderer_sanitizes_to_auto() {
        assert_eq!(Platform::Macos.sanitize_renderer("D3D11"), "Auto");
        assert_eq!(Platform::Windows.sanitize_renderer("D3D11"), "D3D11");
    }
}
