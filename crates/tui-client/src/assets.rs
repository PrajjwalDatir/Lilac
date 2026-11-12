//! Rasterizes the MIT-licensed `assets/pieces/spatial/*.svg` vector pieces
//! via `resvg` at native/2x HiDPI pixel size for PNG-capable terminals
//! (`KITTY PNG`, `ITERM2 PNG`, `SIXEL PNG`).

use std::collections::{HashMap, HashSet};

use chess_core::{PieceKind, Side};
use image::{DynamicImage, Rgba, RgbaImage};
use resvg::{tiny_skia, usvg};

use crate::game::LocalGameApp;

const SVG_WK: &[u8] = include_bytes!("../../../assets/pieces/spatial/wK.svg");
const SVG_WQ: &[u8] = include_bytes!("../../../assets/pieces/spatial/wQ.svg");
const SVG_WR: &[u8] = include_bytes!("../../../assets/pieces/spatial/wR.svg");
const SVG_WB: &[u8] = include_bytes!("../../../assets/pieces/spatial/wB.svg");
const SVG_WN: &[u8] = include_bytes!("../../../assets/pieces/spatial/wN.svg");
const SVG_WP: &[u8] = include_bytes!("../../../assets/pieces/spatial/wP.svg");

const SVG_BK: &[u8] = include_bytes!("../../../assets/pieces/spatial/bK.svg");
const SVG_BQ: &[u8] = include_bytes!("../../../assets/pieces/spatial/bQ.svg");
const SVG_BR: &[u8] = include_bytes!("../../../assets/pieces/spatial/bR.svg");
const SVG_BB: &[u8] = include_bytes!("../../../assets/pieces/spatial/bB.svg");
const SVG_BN: &[u8] = include_bytes!("../../../assets/pieces/spatial/bN.svg");
const SVG_BP: &[u8] = include_bytes!("../../../assets/pieces/spatial/bP.svg");

// Monochrome + Lilac square palette matching the Web UI
const COL_LIGHT: [u8; 3] = [184, 184, 192];
const COL_DARK: [u8; 3] = [72, 72, 82];
const COL_LAST_LIGHT: [u8; 3] = [166, 153, 190];
const COL_LAST_DARK: [u8; 3] = [91, 77, 116];
const COL_HOVER_LIGHT: [u8; 3] = [174, 158, 206];
const COL_HOVER_DARK: [u8; 3] = [94, 80, 126];
const COL_SELECTED: [u8; 3] = [147, 51, 234];
const COL_CURSOR: [u8; 3] = [192, 132, 252];
const COL_LILAC: [u8; 3] = [192, 132, 252];
const COL_WHITE: [u8; 3] = [250, 250, 250];
const COL_DOT: [u8; 3] = [9, 9, 11];

/// Premultiplied RGBA pixmap rasterized from a single piece SVG.
struct PieceBitmap {
    data: Vec<u8>,
}

/// Cached SVG piece atlas sized directly to the terminal's square pixels.
pub struct SvgPieceAtlas {
    tile_w: u32,
    tile_h: u32,
    pieces: HashMap<(Side, PieceKind), PieceBitmap>,
}

impl SvgPieceAtlas {
    pub fn empty() -> Self {
        Self {
            tile_w: 0,
            tile_h: 0,
            pieces: HashMap::with_capacity(12),
        }
    }

    /// Re-rasterizes the 12 SVG pieces only when target square dimensions change.
    pub fn ensure_tile_size(&mut self, tile_w: u32, tile_h: u32) -> anyhow::Result<()> {
        let tile_w = tile_w.max(8);
        let tile_h = tile_h.max(8);
        if self.tile_w == tile_w && self.tile_h == tile_h && !self.pieces.is_empty() {
            return Ok(());
        }

        let entries: [((Side, PieceKind), &[u8]); 12] = [
            ((Side::White, PieceKind::King), SVG_WK),
            ((Side::White, PieceKind::Queen), SVG_WQ),
            ((Side::White, PieceKind::Rook), SVG_WR),
            ((Side::White, PieceKind::Bishop), SVG_WB),
            ((Side::White, PieceKind::Knight), SVG_WN),
            ((Side::White, PieceKind::Pawn), SVG_WP),
            ((Side::Black, PieceKind::King), SVG_BK),
            ((Side::Black, PieceKind::Queen), SVG_BQ),
            ((Side::Black, PieceKind::Rook), SVG_BR),
            ((Side::Black, PieceKind::Bishop), SVG_BB),
            ((Side::Black, PieceKind::Knight), SVG_BN),
            ((Side::Black, PieceKind::Pawn), SVG_BP),
        ];

        let opt = usvg::Options::default();
        self.pieces.clear();
        let min_dim = tile_w.min(tile_h) as f32;

        for (key, bytes) in entries {
            let tree = usvg::Tree::from_data(bytes, &opt)
                .map_err(|e| anyhow::anyhow!("failed to parse piece SVG: {e}"))?;
            let mut pixmap = tiny_skia::Pixmap::new(tile_w, tile_h)
                .ok_or_else(|| anyhow::anyhow!("failed to allocate pixmap"))?;

            let sz = tree.size();
            let scale = (min_dim * 0.88) / sz.width().max(sz.height());
            let bbox = tree.root().abs_stroke_bounding_box();
            let cx = bbox.map_or(sz.width() * 0.5, |b| b.x() + b.width() * 0.5);
            let tx = (tile_w as f32 * 0.5) - cx * scale;
            let ty = (tile_h as f32 - sz.height() * scale) * 0.5;
            let transform = tiny_skia::Transform::from_scale(scale, scale).post_translate(tx, ty);
            resvg::render(&tree, transform, &mut pixmap.as_mut());

            self.pieces.insert(
                key,
                PieceBitmap {
                    data: pixmap.take(),
                },
            );
        }

        self.tile_w = tile_w;
        self.tile_h = tile_h;
        Ok(())
    }

    /// Composites the current 8x8 board state into an exact-size RGBA image.
    pub fn compose_board_image(
        &self,
        app: &LocalGameApp,
        legal_targets: &HashSet<(u8, u8)>,
    ) -> DynamicImage {
        let tw = self.tile_w;
        let th = self.tile_h;
        let mut img = RgbaImage::new(tw * 8, th * 8);
        let squares = app.board.board_squares_top_to_bottom();
        let king_in_check = app.board.is_check();
        let turn = app.board.turn();
        let min_dim = tw.min(th) as f32;
        let rim = (min_dim * 0.05).round().max(1.0) as u32;

        for display_row in 0..8usize {
            let rank = if app.flipped {
                display_row as u8
            } else {
                (7 - display_row) as u8
            };

            for display_col in 0..8usize {
                let file = if app.flipped {
                    (7 - display_col) as u8
                } else {
                    display_col as u8
                };
                let coord = (file, rank);
                let sq_idx = (7 - rank as usize) * 8 + file as usize;
                let piece = squares[sq_idx];
                let is_light = (file + rank) % 2 == 1;

                let mut bg = if is_light { COL_LIGHT } else { COL_DARK };
                if let Some((from, to)) = app.last_move
                    && (coord == from || coord == to)
                {
                    bg = if is_light {
                        COL_LAST_LIGHT
                    } else {
                        COL_LAST_DARK
                    };
                }
                if app.hovered == Some(coord) {
                    bg = if is_light {
                        COL_HOVER_LIGHT
                    } else {
                        COL_HOVER_DARK
                    };
                }
                if app.selected == Some(coord) {
                    bg = COL_SELECTED;
                }

                let ox = display_col as u32 * tw;
                let oy = display_row as u32 * th;
                fill_tile(&mut img, ox, oy, tw, th, bg);

                if king_in_check && piece == Some((turn, PieceKind::King)) {
                    draw_inset_ring(&mut img, ox, oy, tw, th, COL_WHITE, 0, rim);
                    draw_inset_ring(&mut img, ox, oy, tw, th, COL_LILAC, rim, rim);
                }

                if (app.dragging_from.is_some() || app.cursor_active) && app.cursor == coord {
                    draw_inset_ring(&mut img, ox, oy, tw, th, COL_CURSOR, 0, rim);
                }

                if legal_targets.contains(&coord) {
                    if piece.is_some() {
                        draw_capture_ring(&mut img, ox, oy, tw, th, COL_LILAC);
                    } else {
                        draw_legal_dot(&mut img, ox, oy, tw, th);
                    }
                }

                if let Some(key) = piece
                    && let Some(bmp) = self.pieces.get(&key)
                {
                    blend_premultiplied(&mut img, ox, oy, tw, th, bmp);
                }
            }
        }

        DynamicImage::ImageRgba8(img)
    }
}

fn fill_tile(img: &mut RgbaImage, ox: u32, oy: u32, tw: u32, th: u32, rgb: [u8; 3]) {
    let px = Rgba([rgb[0], rgb[1], rgb[2], 255]);
    for y in 0..th {
        for x in 0..tw {
            img.put_pixel(ox + x, oy + y, px);
        }
    }
}

fn draw_inset_ring(
    img: &mut RgbaImage,
    ox: u32,
    oy: u32,
    tw: u32,
    th: u32,
    rgb: [u8; 3],
    inset: u32,
    thickness: u32,
) {
    let px = Rgba([rgb[0], rgb[1], rgb[2], 255]);
    let end_x = tw.saturating_sub(inset);
    let end_y = th.saturating_sub(inset);
    for y in inset..end_y {
        for x in inset..end_x {
            if x < inset + thickness
                || x >= end_x.saturating_sub(thickness)
                || y < inset + thickness
                || y >= end_y.saturating_sub(thickness)
            {
                img.put_pixel(ox + x, oy + y, px);
            }
        }
    }
}

fn blend_pixel(img: &mut RgbaImage, x: u32, y: u32, rgb: [u8; 3], alpha: u32) {
    if alpha == 0 {
        return;
    }
    if alpha >= 255 {
        img.put_pixel(x, y, Rgba([rgb[0], rgb[1], rgb[2], 255]));
        return;
    }
    let dst = img.get_pixel(x, y).0;
    let inv = 255 - alpha;
    let r = ((rgb[0] as u32 * alpha + dst[0] as u32 * inv + 127) / 255) as u8;
    let g = ((rgb[1] as u32 * alpha + dst[1] as u32 * inv + 127) / 255) as u8;
    let b = ((rgb[2] as u32 * alpha + dst[2] as u32 * inv + 127) / 255) as u8;
    img.put_pixel(x, y, Rgba([r, g, b, 255]));
}

fn draw_legal_dot(img: &mut RgbaImage, ox: u32, oy: u32, tw: u32, th: u32) {
    let cx = (tw as f32 - 1.0) * 0.5;
    let cy = (th as f32 - 1.0) * 0.5;
    let r = tw.min(th) as f32 * 0.13;
    for y in 0..th {
        for x in 0..tw {
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            let dist = (dx * dx + dy * dy).sqrt();
            let cov = (r + 0.5 - dist).clamp(0.0, 1.0);
            if cov > 0.0 {
                blend_pixel(img, ox + x, oy + y, COL_DOT, (cov * 96.0).round() as u32);
            }
        }
    }
}

fn draw_capture_ring(img: &mut RgbaImage, ox: u32, oy: u32, tw: u32, th: u32, rgb: [u8; 3]) {
    let cx = (tw as f32 - 1.0) * 0.5;
    let cy = (th as f32 - 1.0) * 0.5;
    let min_dim = tw.min(th) as f32;
    let r_outer = min_dim * 0.44;
    let r_inner = min_dim * 0.35;
    for y in 0..th {
        for x in 0..tw {
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            let dist = (dx * dx + dy * dy).sqrt();
            let cov =
                (r_outer + 0.5 - dist).clamp(0.0, 1.0) * (dist - (r_inner - 0.5)).clamp(0.0, 1.0);
            if cov > 0.0 {
                blend_pixel(img, ox + x, oy + y, rgb, (cov * 235.0).round() as u32);
            }
        }
    }
}

fn blend_premultiplied(
    img: &mut RgbaImage,
    ox: u32,
    oy: u32,
    tw: u32,
    th: u32,
    bmp: &PieceBitmap,
) {
    for y in 0..th {
        for x in 0..tw {
            let idx = ((y * tw + x) * 4) as usize;
            let a = bmp.data[idx + 3] as u32;
            if a == 0 {
                continue;
            }
            if a == 255 {
                img.put_pixel(
                    ox + x,
                    oy + y,
                    Rgba([bmp.data[idx], bmp.data[idx + 1], bmp.data[idx + 2], 255]),
                );
                continue;
            }
            let inv = 255 - a;
            let dst = img.get_pixel(ox + x, oy + y).0;
            let r = (bmp.data[idx] as u32 + (dst[0] as u32 * inv + 127) / 255).min(255) as u8;
            let g = (bmp.data[idx + 1] as u32 + (dst[1] as u32 * inv + 127) / 255).min(255) as u8;
            let b = (bmp.data[idx + 2] as u32 + (dst[2] as u32 * inv + 127) / 255).min(255) as u8;
            img.put_pixel(ox + x, oy + y, Rgba([r, g, b, 255]));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_all_12_spatial_svgs_and_composites_board() {
        let mut atlas = SvgPieceAtlas::empty();
        atlas
            .ensure_tile_size(80, 80)
            .expect("should parse all 12 SVGs");
        let app = LocalGameApp::new();
        let img = atlas.compose_board_image(&app, &HashSet::new());
        assert_eq!(img.width(), 640);
        assert_eq!(img.height(), 640);
    }
}
