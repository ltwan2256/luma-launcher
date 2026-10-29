//! Opt-in, local LUMA group skin packs. No authentication or protocol changes.
//! Every participant imports the same pack before starting a LUMA-enabled client.
use std::{collections::HashMap, path::Path, sync::OnceLock};

pub(super) struct CustomSkin {
    pub rgba: Vec<u8>,
    pub slim: bool,
}

fn valid_name(name: &str) -> bool {
    (3..=16).contains(&name.len())
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

fn read_pack(root: &Path) -> HashMap<String, CustomSkin> {
    let mut skins = HashMap::new();
    let Ok(entries) = std::fs::read_dir(root) else {
        return skins;
    };
    for entry in entries.flatten().take(256) {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("png") {
            continue;
        }
        let Some(name) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if !valid_name(name)
            || !entry.metadata().is_ok_and(|m| m.is_file() && m.len() <= 1024 * 1024)
        {
            continue;
        }
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        if bytes.len() < 24
            || &bytes[..8] != b"\x89PNG\r\n\x1a\n"
            || &bytes[12..16] != b"IHDR"
        {
            continue;
        }
        let width = u32::from_be_bytes(bytes[16..20].try_into().expect("checked header"));
        let height = u32::from_be_bytes(bytes[20..24].try_into().expect("checked header"));
        if width != 64 || ![32, 64].contains(&height) {
            continue;
        }
        let Ok((width, height, rgba)) = horizon_client::hud::decode_bytes(&bytes) else {
            continue;
        };
        let Some(rgba) = horizon_client::skins::process_legacy_skin(width, height, &rgba) else {
            continue;
        };
        let slim = std::fs::read(path.with_extension("json"))
            .ok()
            .filter(|bytes| bytes.len() <= 1024)
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
            .and_then(|json| json.get("slim").and_then(|v| v.as_bool()))
            .unwrap_or(false);
        skins.insert(name.to_ascii_lowercase(), CustomSkin { rgba, slim });
    }
    skins
}

pub(super) fn skin(name: &str) -> Option<&'static CustomSkin> {
    static PACK: OnceLock<HashMap<String, CustomSkin>> = OnceLock::new();
    if !valid_name(name) {
        return None;
    }
    PACK.get_or_init(|| {
        std::env::var_os("LUMA_SKINS_DIR")
            .filter(|value| !value.is_empty())
            .map(|root| read_pack(Path::new(&root)))
            .unwrap_or_default()
    })
    .get(&name.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn player_names_never_address_paths() {
        assert!(valid_name("Test_Player"));
        for name in ["ab", "../Player", "a/b", "a\\b", "Name With Space", "abcdefghijklmnopq"] {
            assert!(!valid_name(name));
        }
    }

    #[test]
    fn reads_png_and_model_and_ignores_invalid_files() {
        let root = std::env::temp_dir().join(format!(
            "luma-skins-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        std::fs::create_dir(&root).expect("scratch folder");
        {
            let file = std::fs::File::create(root.join("test_player.png")).expect("png file");
            let mut encoder = png::Encoder::new(file, 64, 64);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().expect("png header");
            writer.write_image_data(&vec![255; 64 * 64 * 4]).expect("png data");
        }
        std::fs::write(root.join("test_player.json"), br#"{"slim":true}"#)
            .expect("model metadata");
        std::fs::write(root.join("invalid.png"), b"not a PNG").expect("invalid image");
        let pack = read_pack(&root);
        assert_eq!(pack.len(), 1);
        assert!(pack["test_player"].slim);
        assert_eq!(pack["test_player"].rgba.len(), 64 * 64 * 4);
        std::fs::remove_dir_all(&root).expect("scratch cleanup");
    }
}
