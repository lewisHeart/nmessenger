mod animations;
mod catalog;
mod fluent;
pub(crate) use animations::{EMOJI_ANIMATIONS, EMOJI_UNICODE};
pub(crate) use catalog::{AVATAR_CATALOG, EMOJI_CATALOG};
pub(crate) use fluent::ANIMATIONS as FLUENT_ANIMATIONS;

pub(crate) fn bytes(name: &str) -> Option<&'static [u8]> {
    let animated = FLUENT_ANIMATIONS
        .iter()
        .find(|(preview, _)| *preview == name);
    fluent::bytes(animated.map_or(name, |(_, path)| *path)).or_else(|| catalog::bytes(name))
}

pub(crate) fn avatar_bytes(name: &str) -> Option<&'static [u8]> {
    AVATAR_CATALOG
        .iter()
        .any(|(_, _, original)| *original == name)
        .then(|| bytes(name))
        .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_icons_resolve_and_decode() {
        for status in ["disponivel", "ausente", "ocupado", "offline"] {
            let icon = bytes(&format!("status/{status}_icone.svg")).unwrap();
            assert!(crate::ui::nmessenger::decode_image_bytes(icon).is_some());
        }
    }

    #[test]
    fn group_status_icons_use_the_original_distinct_group_images() {
        for status in ["disponivel", "ausente", "ocupado", "offline"] {
            let group = bytes(&format!("status/{status}_grupo.svg")).unwrap();
            let individual = bytes(&format!("status/{status}_icone.svg")).unwrap();
            assert_ne!(group, individual, "ícone de grupo substituído para {status}");
            assert!(crate::ui::nmessenger::decode_image_bytes(group).is_some());
        }
    }

    #[test]
    fn fluent_resources_are_local_animated_pngs_and_override_compatible_previews() {
        assert!(FLUENT_ANIMATIONS.len() >= 60);
        for &(preview, animated) in FLUENT_ANIMATIONS {
            let data = bytes(animated).unwrap();
            assert!(
                data.starts_with(b"\x89PNG\r\n\x1a\n"),
                "invalid PNG: {animated}"
            );
            let header = data
                .windows(4)
                .position(|bytes| bytes == b"acTL")
                .expect("not an APNG");
            let frames = u32::from_be_bytes(data[header + 4..header + 8].try_into().unwrap());
            assert!(frames > 1, "static resource: {animated}");
            assert_eq!(bytes(preview).unwrap().as_ptr(), data.as_ptr());
        }
    }
}
