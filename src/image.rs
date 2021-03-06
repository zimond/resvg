// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use crate::render::prelude::*;
use log::warn;
use std::io::prelude::*;

pub fn draw(image: &usvg::Image, canvas: &mut tiny_skia::Canvas) -> Rect {
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
    canvas: &mut tiny_skia::Canvas,
) {
    match kind {
        usvg::ImageKind::JPEG(ref data) => match read_jpeg(data) {
            Some(image) => {
                draw_raster(&image, view_box, rendering_mode, canvas);
            }
            None => warn!("Failed to load an embedded image."),
        },
        usvg::ImageKind::PNG(ref data) => match read_png(data) {
            Some(image) => {
                draw_raster(&image, view_box, rendering_mode, canvas);
            }
            None => warn!("Failed to load an embedded image."),
        },
        usvg::ImageKind::SVG(ref subtree, ref opts) => {
            if let Some(tree) = load_sub_svg(subtree, opts) {
                draw_svg(&tree, view_box, canvas);
            }
        }
        usvg::ImageKind::RAW(ref data) => match read_raw(data) {
            Some(image) => {
                draw_raster(&image, view_box, rendering_mode, canvas);
            }
            None => warn!("Failed to load an embedded raw image."),
        },
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
    canvas: &mut tiny_skia::Canvas,
) -> Option<()> {
    let (w, h) = img.size.dimensions();
    let mut pixmap = tiny_skia::Pixmap::new(w, h)?;
    image_to_pixmap(&img, pixmap.data_mut());

    let mut filter = tiny_skia::FilterQuality::Bicubic;
    if rendering_mode == usvg::ImageRendering::OptimizeSpeed {
        filter = tiny_skia::FilterQuality::Nearest;
    }

    if view_box.aspect.slice {
        let r = view_box.rect;
        let rect = tiny_skia::Rect::from_xywh(
            r.x() as f32,
            r.y() as f32,
            r.width() as f32,
            r.height() as f32,
        )?;
        canvas.set_clip_rect(rect, true);
    }

    let r = image_rect(&view_box, img.size);
    let rect = tiny_skia::Rect::from_xywh(
        r.x() as f32,
        r.y() as f32,
        r.width() as f32,
        r.height() as f32,
    )?;

    let ts = tiny_skia::Transform::from_row(
        rect.width() as f32 / pixmap.width() as f32,
        0.0,
        0.0,
        rect.height() as f32 / pixmap.height() as f32,
        r.x() as f32,
        r.y() as f32,
    )?;

    let pattern = tiny_skia::Pattern::new(&pixmap, tiny_skia::SpreadMode::Pad, filter, 1.0, ts);
    let mut paint = tiny_skia::Paint::default();
    paint.shader = pattern;

    canvas.fill_rect(rect, &paint);

    canvas.reset_clip();

    Some(())
}

fn image_to_pixmap(image: &Image, pixmap: &mut [u8]) {
    use rgb::FromSlice;

    let mut i = 0;
    match &image.data {
        ImageData::RGB(data) => {
            for p in data.as_rgb() {
                pixmap[i + 0] = p.r;
                pixmap[i + 1] = p.g;
                pixmap[i + 2] = p.b;
                pixmap[i + 3] = 255;

                i += tiny_skia::BYTES_PER_PIXEL;
            }
        }
        ImageData::RGBA(data) => {
            for p in data.as_rgba() {
                pixmap[i + 0] = p.r;
                pixmap[i + 1] = p.g;
                pixmap[i + 2] = p.b;
                pixmap[i + 3] = p.a;

                i += tiny_skia::BYTES_PER_PIXEL;
            }

            svgfilters::multiply_alpha(pixmap.as_rgba_mut());
        }
    }
}

fn draw_svg(
    tree: &usvg::Tree,
    view_box: usvg::ViewBox,
    canvas: &mut tiny_skia::Canvas,
) -> Option<()> {
    let img_size = tree.svg_node().size.to_screen_size();
    let (ts, clip) = usvg::utils::view_box_to_transform_with_clip(&view_box, img_size);

    let mut sub_canvas = canvas.clone();
    sub_canvas.apply_transform(&ts.to_native());
    sub_canvas.pixmap.fill(tiny_skia::Color::TRANSPARENT);
    render_to_canvas(&tree, img_size, &mut sub_canvas);

    if let Some(clip) = clip {
        let rr = tiny_skia::Rect::from_xywh(
            clip.x() as f32,
            clip.y() as f32,
            clip.width() as f32,
            clip.height() as f32,
        )?;
        canvas.set_clip_rect(rr, false);
    }

    let ts = canvas.get_transform();
    canvas.reset_transform();
    canvas.draw_pixmap(0, 0, &sub_canvas.pixmap, &tiny_skia::PixmapPaint::default());
    canvas.reset_clip();
    canvas.set_transform(ts);

    Some(())
}

struct Image {
    data: ImageData,
    size: ScreenSize,
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

fn read_raw(data: &[u8]) -> Option<Image> {
    let mut r = data.clone();
    let mut width_vec = [0u8; 4];
    let mut height_vec = [0u8; 4];
    r.read_exact(&mut width_vec).ok()?;
    r.read_exact(&mut height_vec).ok()?;
    let width: u32 = u32::from_be_bytes(width_vec);
    let height: u32 = u32::from_be_bytes(height_vec);

    let size = ScreenSize::new(width, height)?;

    let mut rgba_vec = vec![0; (width * height * 4) as usize];
    r.read_exact(&mut rgba_vec).ok()?;
    let data = ImageData::RGBA(rgba_vec);
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
