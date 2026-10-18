//! The app's assets: the few Lucide icons Saturn uses beyond gpui-kit's default set (ISC, see
//! `assets/icons/LICENSE-LUCIDE`), falling back to gpui-kit's bundled assets for the rest.

use std::borrow::Cow;

use gpui_kit::{AssetSource, Result, SharedString};

const ICONS: &[(&str, &[u8])] = &[
    ("icons/panel-top.svg", include_bytes!("../assets/icons/panel-top.svg")),
    ("icons/columns-2.svg", include_bytes!("../assets/icons/columns-2.svg")),
    ("icons/rows-2.svg", include_bytes!("../assets/icons/rows-2.svg")),
    ("icons/x.svg", include_bytes!("../assets/icons/x.svg")),
    ("icons/git-compare.svg", include_bytes!("../assets/icons/git-compare.svg")),
];

pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if let Some((_, bytes)) = ICONS.iter().find(|(name, _)| *name == path) {
            return Ok(Some(Cow::Borrowed(bytes)));
        }
        gpui_kit::assets::Assets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut names = gpui_kit::assets::Assets.list(path)?;
        names
            .extend(ICONS.iter().filter(|(name, _)| name.starts_with(path)).map(|(name, _)| SharedString::from(*name)));
        Ok(names)
    }
}
