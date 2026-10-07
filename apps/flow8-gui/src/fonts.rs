use std::path::{Path, PathBuf};

use eframe::egui;

#[derive(Clone, Copy)]
struct SystemFontCandidate {
    family: &'static str,
    file_name: &'static str,
    collection_index: u32,
}

pub(super) fn configure_fonts(context: &egui::Context) {
    let Some((candidate, path, bytes)) = load_system_cjk_font() else {
        tracing::warn!(target: "flow8_gui",
            "no supported system CJK font was found; Simplified Chinese glyphs may be missing"
        );
        return;
    };

    let mut fonts = egui::FontDefinitions::default();
    let mut font_data = egui::FontData::from_owned(bytes);
    font_data.index = candidate.collection_index;
    fonts
        .font_data
        .insert("system-cjk".into(), font_data.into());

    // Keep egui's current Latin fonts first. The system CJK face is appended
    // only as a missing-glyph fallback, which keeps English metrics stable.
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        let names = fonts.families.entry(family).or_default();
        if !names.iter().any(|name| name == "system-cjk") {
            names.push("system-cjk".into());
        }
    }
    context.set_fonts(fonts);
    tracing::info!(target: "flow8_gui",
        family = candidate.family,
        path = %path.display(),
        "registered system CJK font fallback"
    );
}

fn load_system_cjk_font() -> Option<(SystemFontCandidate, PathBuf, Vec<u8>)> {
    let candidates = cjk_font_candidates();
    for (root, recursive) in cjk_font_roots() {
        for candidate in &candidates {
            let exact = root.join(candidate.file_name);
            if let Ok(bytes) = std::fs::read(&exact) {
                return Some((*candidate, exact, bytes));
            }
        }
        if recursive
            && let Some((candidate, path)) = find_font_by_file_name(&root, &candidates, 4)
            && let Ok(bytes) = std::fs::read(&path)
        {
            return Some((candidate, path, bytes));
        }
    }
    None
}

fn cjk_font_candidates() -> Vec<SystemFontCandidate> {
    vec![
        SystemFontCandidate {
            family: "Microsoft YaHei UI",
            file_name: "msyh.ttc",
            collection_index: 0,
        },
        SystemFontCandidate {
            family: "Microsoft YaHei",
            file_name: "msyh.ttf",
            collection_index: 0,
        },
        SystemFontCandidate {
            family: "DengXian",
            file_name: "Deng.ttf",
            collection_index: 0,
        },
        SystemFontCandidate {
            family: "SimSun",
            file_name: "simsun.ttc",
            collection_index: 0,
        },
        SystemFontCandidate {
            family: "Noto Sans CJK SC",
            file_name: "NotoSansCJKsc-Regular.otf",
            collection_index: 0,
        },
        SystemFontCandidate {
            family: "Noto Sans CJK SC",
            file_name: "NotoSansCJK-Regular.ttc",
            collection_index: 2,
        },
        SystemFontCandidate {
            family: "Noto Sans SC",
            file_name: "NotoSansSC-Regular.ttf",
            collection_index: 0,
        },
        SystemFontCandidate {
            family: "Source Han Sans SC",
            file_name: "SourceHanSansSC-Regular.otf",
            collection_index: 0,
        },
        SystemFontCandidate {
            family: "WenQuanYi Micro Hei",
            file_name: "wqy-microhei.ttc",
            collection_index: 0,
        },
    ]
}

fn cjk_font_roots() -> Vec<(PathBuf, bool)> {
    let mut roots = Vec::new();
    #[cfg(target_os = "windows")]
    {
        if let Some(windows_dir) = std::env::var_os("WINDIR") {
            roots.push((PathBuf::from(windows_dir).join("Fonts"), false));
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        for root in [
            "/usr/share/fonts",
            "/usr/local/share/fonts",
            "/usr/share/fonts/opentype/noto",
            "/usr/share/fonts/truetype/wqy",
        ] {
            roots.push((PathBuf::from(root), true));
        }
        if let Some(data_home) = std::env::var_os("XDG_DATA_HOME") {
            roots.push((PathBuf::from(data_home).join("fonts"), true));
        }
    }
    roots
}

fn find_font_by_file_name(
    root: &Path,
    candidates: &[SystemFontCandidate],
    remaining_depth: usize,
) -> Option<(SystemFontCandidate, PathBuf)> {
    if remaining_depth == 0 {
        return None;
    }
    let entries = std::fs::read_dir(root).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_font_by_file_name(&path, candidates, remaining_depth - 1) {
                return Some(found);
            }
            continue;
        }
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if let Some(candidate) = candidates
            .iter()
            .find(|candidate| file_name.eq_ignore_ascii_case(candidate.file_name))
        {
            return Some((*candidate, path));
        }
    }
    None
}
