// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use crate::render::prelude::*;
use log::warn;

pub fn draw(image: &usvg::Image, canvas: &mut skia::Canvas) -> Rect {
    if image.visibility != usvg::Visibility::Visible {
        return image.view_box.rect;
    }

    draw_kind(&image.kind, image.view_box, image.rendering_mode, canvas);
    image.view_box.rect
}

pub fn draw_kind(
    kind: &usvg::ImageKind,
    view_box: usvg::ViewBox,
    rendering_mode: usvg::ImageRendering,
    canvas: &mut skia::Canvas,
) {
    match kind {
        usvg::ImageKind::JPEG(ref data) => match read_jpeg(data) {
            Some(image) => draw_raster(&image, view_box, rendering_mode, canvas),
            None => warn!("Failed to load an embedded image."),
        },
        usvg::ImageKind::PNG(ref data) => match read_png(data) {
            Some(image) => draw_raster(&image, view_box, rendering_mode, canvas),
            None => warn!("Failed to load an embedded image."),
        },
        usvg::ImageKind::SVG(ref subtree, ref opts) => {
            if let Some(tree) = load_sub_svg(subtree, opts) {
                draw_svg(&tree, view_box, canvas);
            }
        }
    }
}

/// Tries to load the `ImageData` content as an SVG image.
///
/// Unlike `Tree::from_*` methods, this one will also remove all `image` elements
/// from the loaded SVG, as required by the spec.
pub fn load_sub_svg(data: &[u8], opt: &usvg::Options) -> Option<usvg::Tree> {
    let sub_opt = usvg::Options {
        path: None,
        dpi: opt.dpi,
        font_family: opt.font_family.clone(),
        font_size: opt.font_size,
        languages: opt.languages.clone(),
        shape_rendering: opt.shape_rendering,
        text_rendering: opt.text_rendering,
        image_rendering: opt.image_rendering,
        keep_named_groups: false,
        #[cfg(feature = "text")]
        fontdb: opt.fontdb.clone(),
    };

    let tree = match usvg::Tree::from_data(data, &sub_opt) {
        Ok(tree) => tree,
        Err(_) => {
            warn!("Failed to load subsvg image.");
            return None;
        }
    };

    sanitize_sub_svg(&tree);
    Some(tree)
}

fn sanitize_sub_svg(tree: &usvg::Tree) {
    // Remove all Image nodes.
    //
    // The referenced SVG image cannot have any 'image' elements by itself.
    // Not only recursive. Any. Don't know why.

    // TODO: implement drain or something to the rctree.
    let mut changed = true;
    while changed {
        changed = false;

        for mut node in tree.root().descendants() {
            let mut rm = false;
            // TODO: feImage?
            if let usvg::NodeKind::Image(_) = *node.borrow() {
                rm = true;
            };

            if rm {
                node.detach();
                changed = true;
                break;
            }
        }
    }
}

fn draw_raster(
    img: &Image,
    view_box: usvg::ViewBox,
    rendering_mode: usvg::ImageRendering,
    canvas: &mut skia::Canvas,
) {
    let image = {
        let (w, h) = img.size.dimensions();
        let mut image = try_opt_warn_or!(
            skia::Surface::new_rgba(w, h),
            (),
            "Failed to create a {}x{} surface.",
            w,
            h
        );

        image_to_surface(&img, &mut image.data_mut());
        image
    };

    let mut filter = skia::FilterQuality::Low;
    if rendering_mode == usvg::ImageRendering::OptimizeSpeed {
        filter = skia::FilterQuality::None;
    }

    canvas.save();

    if view_box.aspect.slice {
        let r = view_box.rect;
        canvas.set_clip_rect(
            r.x() as f32,
            r.y() as f32,
            r.width() as f32,
            r.height() as f32,
        );
    }

    let r = image_rect(&view_box, img.size);
    canvas.draw_surface_rect(
        &image,
        r.x() as f32,
        r.y() as f32,
        r.width() as f32,
        r.height() as f32,
        filter,
    );

    // Revert.
    canvas.restore();
}

fn image_to_surface(image: &Image, surface: &mut [u8]) {
    // Surface is always ARGB.
    const SURFACE_CHANNELS: usize = 4;

    use rgb::FromSlice;

    let mut i = 0;
    match &image.data {
        ImageData::RGB(data) => {
            for p in data.as_rgb() {
                surface[i + 0] = p.r;
                surface[i + 1] = p.g;
                surface[i + 2] = p.b;
                surface[i + 3] = 255;

                i += SURFACE_CHANNELS;
            }
        }
        ImageData::RGBA(data) => {
            for p in data.as_rgba() {
                surface[i + 0] = p.r;
                surface[i + 1] = p.g;
                surface[i + 2] = p.b;
                surface[i + 3] = p.a;

                i += SURFACE_CHANNELS;
            }
        }
    }
}

fn draw_svg(tree: &usvg::Tree, view_box: usvg::ViewBox, canvas: &mut skia::Canvas) {
    let img_size = tree.svg_node().size.to_screen_size();
    let (ts, clip) = usvg::utils::view_box_to_transform_with_clip(&view_box, img_size);

    canvas.save();

    if let Some(clip) = clip {
        canvas.set_clip_rect(
            clip.x() as f32,
            clip.y() as f32,
            clip.width() as f32,
            clip.height() as f32,
        );
    }

    canvas.concat(ts.to_native());
    render_to_canvas(&tree, img_size, canvas);

    canvas.restore();
}

/// A raster image data.
struct Image {
    pub data: ImageData,
    pub size: ScreenSize,
}

/// A raster image data kind.
enum ImageData {
    RGB(Vec<u8>),
    RGBA(Vec<u8>),
}

fn read_png(data: &[u8]) -> Option<Image> {
    let decoder = png::Decoder::new(data);
    let (info, mut reader) = decoder.read_info().ok()?;

    let size = ScreenSize::new(info.width, info.height)?;

    let mut img_data = vec![0; info.buffer_size()];
    reader.next_frame(&mut img_data).ok()?;

    let data = match info.color_type {
        png::ColorType::RGB => ImageData::RGB(img_data),
        png::ColorType::RGBA => ImageData::RGBA(img_data),
        png::ColorType::Grayscale => {
            let mut rgb_data = Vec::with_capacity(img_data.len() * 3);
            for gray in img_data {
                rgb_data.push(gray);
                rgb_data.push(gray);
                rgb_data.push(gray);
            }

            ImageData::RGB(rgb_data)
        }
        png::ColorType::GrayscaleAlpha => {
            let mut rgba_data = Vec::with_capacity(img_data.len() * 2);
            for slice in img_data.chunks(2) {
                let gray = slice[0];
                let alpha = slice[1];
                rgba_data.push(gray);
                rgba_data.push(gray);
                rgba_data.push(gray);
                rgba_data.push(alpha);
            }

            ImageData::RGBA(rgba_data)
        }
        png::ColorType::Indexed => {
            warn!("Indexed PNG is not supported.");
            return None;
        }
    };

    Some(Image { data, size })
}

fn read_jpeg(data: &[u8]) -> Option<Image> {
    let mut decoder = jpeg_decoder::Decoder::new(data);
    let img_data = decoder.decode().ok()?;
    let info = decoder.info()?;

    let size = ScreenSize::new(info.width as u32, info.height as u32)?;

    let data = match info.pixel_format {
        jpeg_decoder::PixelFormat::RGB24 => ImageData::RGB(img_data),
        jpeg_decoder::PixelFormat::L8 => {
            let mut rgb_data = Vec::with_capacity(img_data.len() * 3);
            for gray in img_data {
                rgb_data.push(gray);
                rgb_data.push(gray);
                rgb_data.push(gray);
            }

            ImageData::RGB(rgb_data)
        }
        _ => return None,
    };

    Some(Image { data, size })
}

/// Calculates an image rect depending on the provided view box.
fn image_rect(view_box: &usvg::ViewBox, img_size: ScreenSize) -> Rect {
    let new_size = img_size.fit_view_box(view_box);
    let (x, y) = usvg::utils::aligned_pos(
        view_box.aspect.align,
        view_box.rect.x(),
        view_box.rect.y(),
        view_box.rect.width() - new_size.width() as f64,
        view_box.rect.height() - new_size.height() as f64,
    );

    new_size.to_size().to_rect(x, y)
}
