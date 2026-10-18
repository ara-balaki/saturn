# Vendored GPUI

`gpui-pre`, `gpui-pre-apple` and `gpui-pre-macos` 0.3.9 from crates.io (Apache-2.0, see each
crate's `LICENSE-APACHE`), with four patches: a **backdrop blur** primitive, so a panel can show
what's under it frosted; drop shadows that stay outside their element; translucent layers that
stack instead of adding up, so a see-through window stays see-through; and the window's macOS blur
kept as the whole sidebar material (fill, clamp, tint, saturation) instead of a bare blur. The root
`Cargo.toml` points Saturn at these copies with `[patch.crates-io]`; the workspace excludes them,
so Saturn's lints and formatter leave them alone. `gpui-pre`'s examples were left out.

The idea and the shape of the API come from the
[`gpui_rn` fork](https://github.com/rngpui/zed-react-native-gpui) (Apache-2.0); the code here is
written against this version of GPUI. Every change is marked "Saturn's patch":

- `gpui-pre/src/scene.rs`: the `BackdropBlur` primitive, its list in `Scene`, and
  `PrimitiveBatch::BackdropBlurs`. The same file is copied to `gpui-pre-apple/vendor/gpui/src/`,
  where the build script reads it to generate the shader header.
- `gpui-pre/src/window.rs`: `Window::paint_backdrop_blur`.
- `gpui-pre-apple/build.rs`: exports `BackdropBlur` and `BackdropBlurInputIndex` to the header.
- `gpui-pre-apple/src/metal_renderer.rs`: the layer's texture stays readable; a blur batch copies
  what was drawn so far into a mipmapped texture and draws the blurs from it; and the pipelines
  blend alpha "over" (`OneMinusSourceAlpha`) instead of adding it.
- `gpui-pre-apple/src/shaders.metal`: `backdrop_blur_vertex` and `backdrop_blur_fragment`; and
  `shadow_fragment` clips a drop shadow to outside its element, as CSS does, so shadows don't
  tint translucent panels.

- `gpui-pre-macos/src/window.rs`: `blurred_view_update_layer` no longer strips the material's
  layers (`remove_layer_background` is left in place, unused); it turns the material's fill down
  to `MATERIAL_FILL` (0.6), keeping its tint and saturation.

To move to a newer GPUI: copy the new crates here, then re-apply the changes above.
