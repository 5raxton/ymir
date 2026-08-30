use std::ptr;

use anyhow::{ensure, Context as _};
use smithay::backend::allocator::dmabuf::Dmabuf;
use smithay::backend::allocator::{Buffer, Fourcc};
use smithay::backend::renderer::damage::OutputDamageTracker;
use smithay::backend::renderer::element::utils::{Relocate, RelocateRenderElement};
use smithay::backend::renderer::element::{Element, Kind, RenderElement, RenderElementStates};
use smithay::backend::renderer::gles::{
    GlesError, GlesMapping, GlesRenderer, GlesTarget, GlesTexture,
};
use smithay::backend::renderer::sync::SyncPoint;
use smithay::backend::renderer::{
    Bind, Color32F, ExportMem, Frame, Offscreen, Renderer, Texture as _,
};
use smithay::reexports::wayland_server::protocol::wl_buffer::WlBuffer;
use smithay::reexports::wayland_server::protocol::wl_shm;
use smithay::utils::user_data::UserDataMap;
use smithay::utils::{Logical, Physical, Point, Rectangle, Scale, Size, Transform};
use smithay::wayland::shm;
use solid_color::{SolidColorBuffer, SolidColorRenderElement};
use ymir_config::BlockOutFrom;

use self::primary_gpu_texture::PrimaryGpuTextureRenderElement;
use self::texture::{TextureBuffer, TextureRenderElement};
use crate::render_helpers::renderer::AsGlesRenderer;
use crate::render_helpers::xray::Xray;

pub mod background_effect;
pub mod blur;
pub mod border;
pub mod clipped_surface;
pub mod damage;
pub mod debug;
pub mod effect_buffer;
pub mod framebuffer_effect;
pub mod gradient_fade_texture;
pub mod memory;
pub mod offscreen;
pub mod primary_gpu_texture;
pub mod render_elements;
pub mod renderer;
pub mod resize;
pub mod resources;
pub mod shader_element;
pub mod shaders;
pub mod shadow;
pub mod snapshot;
pub mod solid_color;
pub mod surface;
pub mod texture;
pub mod xray;

/// A rendering context.
///
/// Bundles together things needed by most rendering code.
pub struct RenderCtx<'a, R> {
    pub renderer: &'a mut R,
    pub target: RenderTarget,
    pub xray: Option<&'a Xray>,
}

impl<'a, R> RenderCtx<'a, R> {
    /// Reborrows this context with a smaller lifetime.
    #[inline]
    pub fn r<'b>(&'b mut self) -> RenderCtx<'b, R> {
        RenderCtx {
            renderer: self.renderer,
            target: self.target,
            xray: self.xray,
        }
    }
}

impl<'a, R: AsGlesRenderer> RenderCtx<'a, R> {
    pub fn as_gles<'b>(&'b mut self) -> RenderCtx<'b, GlesRenderer> {
        RenderCtx {
            renderer: self.renderer.as_gles_renderer(),
            target: self.target,
            xray: self.xray,
        }
    }
}

/// What we're rendering for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderTarget {
    /// Rendering to display on screen.
    Output = 0,
    /// Rendering for a screencast.
    Screencast,
    /// Rendering for any other screen capture.
    ScreenCapture,
}

/// Buffer with location, src and dst.
#[derive(Debug)]
pub struct BakedBuffer<B> {
    pub buffer: B,
    pub location: Point<f64, Logical>,
    pub src: Option<Rectangle<f64, Logical>>,
    pub dst: Option<Size<i32, Logical>>,
}

pub trait ToRenderElement {
    type RenderElement;

    fn to_render_element(
        &self,
        location: Point<f64, Logical>,
        scale: Scale<f64>,
        alpha: f32,
        kind: Kind,
    ) -> Self::RenderElement;
}

impl RenderTarget {
    pub const COUNT: usize = 3;

    pub fn should_block_out(self, block_out_from: Option<BlockOutFrom>) -> bool {
        match block_out_from {
            None => false,
            Some(BlockOutFrom::Screencast) => self == RenderTarget::Screencast,
            Some(BlockOutFrom::ScreenCapture) => self != RenderTarget::Output,
        }
    }
}

impl ToRenderElement for BakedBuffer<TextureBuffer<GlesTexture>> {
    type RenderElement = PrimaryGpuTextureRenderElement;

    fn to_render_element(
        &self,
        location: Point<f64, Logical>,
        _scale: Scale<f64>,
        alpha: f32,
        kind: Kind,
    ) -> Self::RenderElement {
        let elem = TextureRenderElement::from_texture_buffer(
            self.buffer.clone(),
            location + self.location,
            alpha,
            self.src,
            self.dst.map(|dst| dst.to_f64()),
            kind,
        );
        PrimaryGpuTextureRenderElement(elem)
    }
}

impl ToRenderElement for BakedBuffer<SolidColorBuffer> {
    type RenderElement = SolidColorRenderElement;

    fn to_render_element(
        &self,
        location: Point<f64, Logical>,
        _scale: Scale<f64>,
        alpha: f32,
        kind: Kind,
    ) -> Self::RenderElement {
        SolidColorRenderElement::from_buffer(&self.buffer, location + self.location, alpha, kind)
    }
}

pub fn encompassing_geo(
    scale: Scale<f64>,
    elements: impl Iterator<Item = impl Element>,
) -> Rectangle<i32, Physical> {
    elements
        .map(|ele| ele.geometry(scale))
        .reduce(|a, b| a.merge(b))
        .unwrap_or_default()
}

pub fn create_texture(
    renderer: &mut GlesRenderer,
    size: Size<i32, Physical>,
    fourcc: Fourcc,
) -> Result<GlesTexture, GlesError> {
    let buffer_size = size.to_logical(1).to_buffer(1, Transform::Normal);
    renderer.create_buffer(fourcc, buffer_size)
}

pub fn copy_framebuffer(
    renderer: &mut GlesRenderer,
    target: &GlesTarget,
    fourcc: Fourcc,
) -> Result<GlesMapping, GlesError> {
    renderer.copy_framebuffer(target, Rectangle::from_size(target.size()), fourcc)
}

pub fn render_to_encompassing_texture(
    renderer: &mut GlesRenderer,
    scale: Scale<f64>,
    transform: Transform,
    fourcc: Fourcc,
    elements: &[impl RenderElement<GlesRenderer>],
) -> anyhow::Result<(GlesTexture, SyncPoint, Rectangle<i32, Physical>)> {
    let geo = encompassing_geo(scale, elements.iter());
    let elements = elements.iter().rev().map(|ele| {
        RelocateRenderElement::from_element(ele, geo.loc.upscale(-1), Relocate::Relative)
    });

    let (texture, sync_point) =
        render_to_texture(renderer, geo.size, scale, transform, fourcc, elements)?;

    Ok((texture, sync_point, geo))
}

pub fn render_to_texture(
    renderer: &mut GlesRenderer,
    size: Size<i32, Physical>,
    scale: Scale<f64>,
    transform: Transform,
    fourcc: Fourcc,
    elements: impl Iterator<Item = impl RenderElement<GlesRenderer>>,
) -> anyhow::Result<(GlesTexture, SyncPoint)> {
    let _span = tracy_client::span!();

    let mut texture = create_texture(renderer, size, fourcc).context("error creating texture")?;

    let sync_point = {
        let mut target = renderer
            .bind(&mut texture)
            .context("error binding texture")?;

        render_elements(renderer, &mut target, size, scale, transform, elements)?
    };

    Ok((texture, sync_point))
}

pub fn render_and_download(
    renderer: &mut GlesRenderer,
    size: Size<i32, Physical>,
    scale: Scale<f64>,
    transform: Transform,
    fourcc: Fourcc,
    elements: impl Iterator<Item = impl RenderElement<GlesRenderer>>,
) -> anyhow::Result<GlesMapping> {
    let _span = tracy_client::span!();

    let mut texture = create_texture(renderer, size, fourcc).context("error creating texture")?;
    let mut target = renderer
        .bind(&mut texture)
        .context("error binding texture")?;

    let _sync = render_elements(renderer, &mut target, size, scale, transform, elements)
        .context("error rendering")?;

    copy_framebuffer(renderer, &target, fourcc).context("error copying framebuffer")
}

pub fn render_to_vec(
    renderer: &mut GlesRenderer,
    size: Size<i32, Physical>,
    scale: Scale<f64>,
    transform: Transform,
    fourcc: Fourcc,
    elements: impl Iterator<Item = impl RenderElement<GlesRenderer>>,
) -> anyhow::Result<Vec<u8>> {
    let _span = tracy_client::span!();

    let mapping = render_and_download(renderer, size, scale, transform, fourcc, elements)
        .context("error rendering")?;
    let copy = renderer
        .map_texture(&mapping)
        .context("error mapping texture")?;
    Ok(copy.to_vec())
}

pub fn render_to_dmabuf(
    renderer: &mut GlesRenderer,
    damage_tracker: &mut OutputDamageTracker,
    mut dmabuf: Dmabuf,
    elements: &[impl RenderElement<GlesRenderer>],
    states: RenderElementStates,
) -> anyhow::Result<SyncPoint> {
    let _span = tracy_client::span!();
    let (size, _scale, _transform) = damage_tracker.mode().try_into().unwrap();
    ensure!(
        dmabuf.width() == size.w as u32 && dmabuf.height() == size.h as u32,
        "invalid buffer size"
    );

    let mut target = renderer.bind(&mut dmabuf).context("error binding dmabuf")?;
    let res = damage_tracker
        .render_output_with_states(
            renderer,
            &mut target,
            0,
            elements,
            Color32F::TRANSPARENT,
            states,
        )
        .context("error rendering to dmabuf")?;
    Ok(res.sync)
}

pub fn render_to_shm(
    renderer: &mut GlesRenderer,
    damage_tracker: &mut OutputDamageTracker,
    buffer: &WlBuffer,
    elements: &[impl RenderElement<GlesRenderer>],
    states: RenderElementStates,
) -> anyhow::Result<()> {
    let _span = tracy_client::span!();
    shm::with_buffer_contents_mut(buffer, |shm_buffer, shm_len, buffer_data| {
        let (size, _scale, _transform) = damage_tracker.mode().try_into().unwrap();
        let fourcc = Fourcc::Xrgb8888;

        ensure!(
            // The buffer prefers pixels in little endian ...
            buffer_data.format == wl_shm::Format::Xrgb8888
                && buffer_data.width == size.w
                && buffer_data.height == size.h
                && buffer_data.stride == size.w * 4
                && shm_len == buffer_data.stride as usize * buffer_data.height as usize,
            "invalid buffer format or size"
        );

        let mut texture =
            create_texture(renderer, size, fourcc).context("error creating texture")?;
        let mut target = renderer
            .bind(&mut texture)
            .context("error binding texture")?;

        let _res = damage_tracker
            .render_output_with_states(
                renderer,
                &mut target,
                0,
                elements,
                Color32F::TRANSPARENT,
                states,
            )
            .context("error rendering")?;

        let mapping =
            copy_framebuffer(renderer, &target, fourcc).context("error copying framebuffer")?;
        let bytes = renderer
            .map_texture(&mapping)
            .context("error mapping texture")?;

        unsafe {
            let _span = tracy_client::span!("copy_nonoverlapping");
            ptr::copy_nonoverlapping(bytes.as_ptr(), shm_buffer.cast(), shm_len);
        }

        Ok(())
    })
    .context("expected shm buffer, but didn't get one")?
}

pub fn clear_dmabuf(renderer: &mut GlesRenderer, mut dmabuf: Dmabuf) -> anyhow::Result<SyncPoint> {
    let size = dmabuf.size();
    let size = size.to_logical(1, Transform::Normal).to_physical(1);
    let mut target = renderer.bind(&mut dmabuf).context("error binding dmabuf")?;
    let mut frame = renderer
        .render(&mut target, size, Transform::Normal)
        .context("error starting frame")?;
    frame
        .clear(Color32F::TRANSPARENT, &[Rectangle::from_size(size)])
        .context("error clearing")?;
    frame.finish().context("error finishing frame")
}

fn render_elements(
    renderer: &mut GlesRenderer,
    target: &mut GlesTarget,
    size: Size<i32, Physical>,
    scale: Scale<f64>,
    transform: Transform,
    elements: impl Iterator<Item = impl RenderElement<GlesRenderer>>,
) -> anyhow::Result<SyncPoint> {
    let transform = transform.invert();
    let output_rect = Rectangle::from_size(transform.transform_size(size));

    let mut frame = renderer
        .render(target, size, transform)
        .context("error starting frame")?;

    frame
        .clear(Color32F::TRANSPARENT, &[output_rect])
        .context("error clearing")?;

    for element in elements {
        let src = element.src();
        let dst = element.geometry(scale);

        if let Some(mut damage) = output_rect.intersection(dst) {
            damage.loc -= dst.loc;

            let cache = UserDataMap::new();
            if element.is_framebuffer_effect() {
                element
                    .capture_framebuffer(&mut frame, src, dst, &cache)
                    .context("error in capture_framebuffer()")?;
            }
            element
                .draw(&mut frame, src, dst, &[damage], &[], Some(&cache))
                .context("error drawing element")?;
        }
    }

    frame.finish().context("error finishing frame")
}

#[cfg(test)]
mod shader_color_tests {
    //! CPU-side mirror of the color math in `shaders/border.frag` (R1/R4/R5).
    //!
    //! The shader can't be executed from a Rust test, so these tests re-implement the
    //! exact same conversion pipeline to validate the invariants the fixes rely on:
    //! mixing in-gamut sRGB colors can leave the sRGB gamut in Oklab/Oklch, and the
    //! resulting linear RGB must be finite and gamut-mapped back into range instead of
    //! producing NaN (negative LMS) or a hue-distorting per-channel clip.

    fn srgb_to_linear(c: [f64; 3]) -> [f64; 3] {
        c.map(|v| {
            if v <= 0.04045 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        })
    }

    fn linear_to_srgb(c: [f64; 3]) -> [f64; 3] {
        c.map(|v| {
            if v <= 0.0031308 {
                v * 12.92
            } else {
                1.055 * v.max(0.0).powf(1.0 / 2.4) - 0.055
            }
        })
    }

    fn linear_to_oklab(color: [f64; 3]) -> [f64; 3] {
        let [l1, l2, l3] = [
            0.4122214708 * color[0] + 0.5363325363 * color[1] + 0.0514459929 * color[2],
            0.2119034982 * color[0] + 0.6806995451 * color[1] + 0.1073969566 * color[2],
            0.0883024619 * color[0] + 0.2817188376 * color[1] + 0.6299787005 * color[2],
        ];
        let [m1, m2, m3] = [l1.cbrt(), l2.cbrt(), l3.cbrt()];
        [
            0.2104542553 * m1 + 0.7936177850 * m2 - 0.0040720468 * m3,
            1.9779984951 * m1 - 2.4285922050 * m2 + 0.4505937099 * m3,
            0.0259040371 * m1 + 0.7827717662 * m2 - 0.8086757660 * m3,
        ]
    }

    fn oklab_to_linear(color: [f64; 3]) -> [f64; 3] {
        let [l1, l2, l3] = [
            color[0] + 0.3963377774 * color[1] + 0.2158037573 * color[2],
            color[0] - 0.1055613458 * color[1] - 0.0638541728 * color[2],
            color[0] - 0.0894841775 * color[1] - 1.2914855480 * color[2],
        ];
        // Mirror the `max(lms, 0)` guard in the shader: negative LMS must not reach `pow`.
        let [m1, m2, m3] = [
            l1.max(0.0).powf(3.0),
            l2.max(0.0).powf(3.0),
            l3.max(0.0).powf(3.0),
        ];
        [
            4.0767416621 * m1 - 3.3077115913 * m2 + 0.2309699292 * m3,
            -1.2684380046 * m1 + 2.6097574011 * m2 - 0.3413193965 * m3,
            -0.0041960863 * m1 - 0.7034186147 * m2 + 1.7076147010 * m3,
        ]
    }

    fn lab_to_lch(color: [f64; 3]) -> [f64; 3] {
        let c = (color[1] * color[1] + color[2] * color[2]).sqrt();
        let mut h = color[2].atan2(color[1]).to_degrees();
        if h <= 0.0 {
            h += 360.0;
        }
        [color[0], c, h]
    }

    fn lch_to_lab(color: [f64; 3]) -> [f64; 3] {
        let h = color[2].to_radians();
        [color[0], color[1] * h.cos(), color[1] * h.sin()]
    }

    fn in_gamut(c: [f64; 3]) -> bool {
        c.iter().all(|&v| (0.0..=1.0).contains(&v))
    }

    fn reduce_gamut(linear: [f64; 3]) -> [f64; 3] {
        if in_gamut(linear) {
            return linear;
        }
        let lch = lab_to_lch(linear_to_oklab(linear));
        let mut lo = 0.0;
        let mut hi = lch[1];
        for _ in 0..12 {
            let mid = (lo + hi) * 0.5;
            let rgb = oklab_to_linear(lch_to_lab([lch[0], mid, lch[2]]));
            if in_gamut(rgb) {
                lo = mid;
            } else {
                hi = mid;
            };
        }
        oklab_to_linear(lch_to_lab([lch[0], lo, lch[2]]))
    }

    /// Mixed color between two sRGB endpoints, mirroring the shader's oklab/oklch path.
    fn mix_colors(a: [f64; 3], b: [f64; 3], ratio: f64, oklch: bool) -> [f64; 4] {
        let to_signal = |c: [f64; 3]| {
            if oklch {
                lab_to_lch(linear_to_oklab(c))
            } else {
                linear_to_oklab(c)
            }
        };
        let mix_oklab = |a: [f64; 3], b: [f64; 3]| {
            a.iter()
                .zip(b)
                .map(|(&x, y)| x + (y - x) * ratio)
                .collect::<Vec<_>>()
                .try_into()
                .unwrap()
        };

        let (m_a, m_b) = (to_signal(srgb_to_linear(a)), to_signal(srgb_to_linear(b)));
        let mixed = mix_oklab(m_a, m_b);
        let linear = if oklch {
            oklab_to_linear(lch_to_lab(mixed))
        } else {
            oklab_to_linear(mixed)
        };
        let reduced = reduce_gamut(linear);
        [reduced[0], reduced[1], reduced[2], 1.0]
    }

    #[test]
    fn srgb_eotf_roundtrips() {
        for i in 0..=100 {
            let v = i as f64 / 100.0;
            let linear = srgb_to_linear([v; 3]);
            let back = linear_to_srgb(linear);
            for (got, expected) in back.iter().zip([v; 3]) {
                assert!(
                    (got - expected).abs() < 1e-6,
                    "roundtrip failed at {v}: {got} vs {expected}"
                );
            }
        }
    }

    #[test]
    fn oklab_mix_out_of_gamut_stays_finite_and_in_gamut() {
        // Mixing vivid red and blue along the Oklab chord leaves the sRGB gamut for
        // intermediate ratios; every sample must be finite and end up in-gamut, with no
        // NaN from negative LMS (R1/R5) and no per-channel clip (R5).
        let red = [1.0, 0.0, 0.0];
        let blue = [0.0, 0.0, 1.0];
        for oklch in [false, true] {
            for i in 0..=20 {
                let ratio = i as f64 / 20.0;
                let mixed = mix_colors(red, blue, ratio, oklch);
                assert!(
                    mixed.iter().all(|v| v.is_finite()),
                    "oklch={oklch} ratio={ratio}: non-finite output {mixed:?}"
                );
                for &v in &mixed[..3] {
                    assert!(
                        (0.0..=1.0).contains(&v),
                        "oklch={oklch} ratio={ratio}: out-of-gamut output {mixed:?}"
                    );
                }
            }
        }
    }
}
