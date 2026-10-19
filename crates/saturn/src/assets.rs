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
    ("icons/locate.svg", include_bytes!("../assets/icons/locate.svg")),
    ("icons/locate-fixed.svg", include_bytes!("../assets/icons/locate-fixed.svg")),
    ("icons/square-dot.svg", include_bytes!("../assets/icons/square-dot.svg")),
    ("icons/square-plus.svg", include_bytes!("../assets/icons/square-plus.svg")),
    ("icons/square-minus.svg", include_bytes!("../assets/icons/square-minus.svg")),
    ("icons/square-arrow-right.svg", include_bytes!("../assets/icons/square-arrow-right.svg")),
    ("icons/square-x.svg", include_bytes!("../assets/icons/square-x.svg")),
    ("icons/git-branch.svg", include_bytes!("../assets/icons/git-branch.svg")),
    ("icons/git-pull-request.svg", include_bytes!("../assets/icons/git-pull-request.svg")),
    ("icons/git-pull-request-draft.svg", include_bytes!("../assets/icons/git-pull-request-draft.svg")),
    ("icons/git-pull-request-closed.svg", include_bytes!("../assets/icons/git-pull-request-closed.svg")),
    ("icons/git-merge.svg", include_bytes!("../assets/icons/git-merge.svg")),
    ("icons/bot.svg", include_bytes!("../assets/icons/bot.svg")),
    ("icons/chart-no-axes-column.svg", include_bytes!("../assets/icons/chart-no-axes-column.svg")),
    ("icons/database.svg", include_bytes!("../assets/icons/database.svg")),
    ("icons/key-round.svg", include_bytes!("../assets/icons/key-round.svg")),
    ("icons/link-2.svg", include_bytes!("../assets/icons/link-2.svg")),
    ("icons/zoom-in.svg", include_bytes!("../assets/icons/zoom-in.svg")),
    ("icons/zoom-out.svg", include_bytes!("../assets/icons/zoom-out.svg")),
    ("icons/scan.svg", include_bytes!("../assets/icons/scan.svg")),
    ("icons/maximize-2.svg", include_bytes!("../assets/icons/maximize-2.svg")),
    ("icons/minimize-2.svg", include_bytes!("../assets/icons/minimize-2.svg")),
    ("icons/waypoints.svg", include_bytes!("../assets/icons/waypoints.svg")),
    ("icons/file-code.svg", include_bytes!("../assets/icons/file-code.svg")),
    ("icons/arrow-left.svg", include_bytes!("../assets/icons/arrow-left.svg")),
    ("icons/search.svg", include_bytes!("../assets/icons/search.svg")),
    ("icons/refresh-cw.svg", include_bytes!("../assets/icons/refresh-cw.svg")),
    ("icons/external-link.svg", include_bytes!("../assets/icons/external-link.svg")),
    ("icons/panel-right.svg", include_bytes!("../assets/icons/panel-right.svg")),
    ("icons/circle-x.svg", include_bytes!("../assets/icons/circle-x.svg")),
    ("icons/triangle-alert.svg", include_bytes!("../assets/icons/triangle-alert.svg")),
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
