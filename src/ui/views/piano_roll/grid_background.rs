//! Teclado, rejilla y regla del piano roll.
use crate::ui::{
    DesignSystem,
    piano_roll::geometry::{Grid, KEYBOARD_WIDTH},
};
use elevate::musical::{keyboard::is_black, pitch::note_name};
use gpui::{App, Bounds, Pixels, TextAlign, TextRun, Window, fill, point, px, rgb, rgba, size};

pub(super) fn paint_rows(
    grid: &Grid,
    ds: &DesignSystem,
    highlight: bool,
    window: &mut Window,
    cx: &mut App,
) {
    window.paint_quad(fill(grid.all, rgb(ds.panel_dark)));
    let keys_left = grid.all.left() + px(8.0);
    let keys_width = KEYBOARD_WIDTH - 14.0;
    rectangle(
        window,
        keys_left,
        grid.notes.top(),
        keys_width,
        f32::from(grid.notes.size.height),
        (ds.text << 8) | 0xff,
    );
    for pitch in 0..=127 {
        let y = grid.y(pitch);
        if y + px(grid.row_height) <= grid.notes.top() || y >= grid.notes.bottom() {
            continue;
        }
        let color = if is_black(pitch) {
            ds.panel_dark
        } else {
            ds.panel
        };
        rectangle(
            window,
            grid.notes.left(),
            y,
            f32::from(grid.notes.size.width),
            grid.row_height,
            (color << 8) | 0xff,
        );
        if highlight && matches!(pitch % 12, 0 | 2 | 3 | 5 | 7 | 8 | 10) {
            rectangle(
                window,
                grid.notes.left(),
                y,
                f32::from(grid.notes.size.width),
                grid.row_height,
                (ds.accent << 8) | 0x0b,
            );
        }
        rectangle(
            window,
            grid.notes.left(),
            y,
            f32::from(grid.notes.size.width),
            0.5,
            (ds.grid << 8) | 0x3c,
        );
        if is_black(pitch) {
            window.paint_quad(
                fill(
                    Bounds::new(
                        point(keys_left, y + px(0.6)),
                        size(px(keys_width * 0.57), px((grid.row_height - 1.2).max(1.0))),
                    ),
                    rgb(ds.panel_dark),
                )
                .corner_radii(px(1.0)),
            );
        } else {
            rectangle(
                window,
                keys_left,
                y,
                keys_width,
                0.6,
                (ds.muted << 8) | 0x90,
            );
            if grid.row_height >= 9.0 || pitch % 12 == 0 {
                label(
                    window,
                    cx,
                    &note_name(pitch),
                    point(
                        keys_left + px(keys_width - 22.0),
                        y + px((grid.row_height - 11.0).max(0.0) / 2.0),
                    ),
                    ds.panel_dark,
                    9.0,
                );
            }
        }
    }
    rectangle(
        window,
        grid.all.left(),
        grid.notes.bottom() + px(1.0),
        f32::from(grid.all.size.width),
        1.0,
        (ds.accent << 8) | 0xb0,
    );
}

pub(super) fn paint_time(
    grid: &Grid,
    ds: &DesignSystem,
    beats: f64,
    window: &mut Window,
    cx: &mut App,
) {
    let subdivisions = if grid.pixels_per_beat >= 16.0 { 4 } else { 1 };
    let first = (grid.first_beat * subdivisions as f64).floor() as usize;
    let visible_beats = f64::from(f32::from(grid.notes.size.width) / grid.pixels_per_beat);
    let last = ((grid.first_beat + visible_beats).min(beats) * subdivisions as f64).ceil() as usize;
    for step in first..=last.min(first.saturating_add(4096)) {
        let beat = step as f64 / subdivisions as f64;
        let x = grid.x(beat);
        let is_bar = step % (4 * subdivisions) == 0;
        let is_beat = step % subdivisions == 0;
        let color = if is_bar {
            (ds.accent << 8) | 0xa0
        } else if is_beat {
            (ds.grid << 8) | 0xba
        } else {
            (ds.border << 8) | 0x6a
        };
        let top = if is_bar {
            grid.all.top() + px(4.0)
        } else {
            grid.notes.top()
        };
        rectangle(
            window,
            x,
            top,
            if is_bar { 1.0 } else { 0.6 },
            f32::from(grid.all.bottom() - top) - 10.0,
            color,
        );
        if is_bar && beat < beats {
            label(
                window,
                cx,
                &(beat as usize / 4 + 1).to_string(),
                point(x + px(8.0), grid.all.top() + px(3.0)),
                ds.text,
                11.0,
            );
        }
        if is_beat && beat < beats && grid.pixels_per_beat > 12.0 {
            label(
                window,
                cx,
                &(beat as usize % 4 + 1).to_string(),
                point(
                    x + px(grid.pixels_per_beat * 0.42),
                    grid.all.top() + px(23.0),
                ),
                ds.muted,
                9.0,
            );
        }
    }
    if grid.velocity.size.height > px(0.0) {
        for fraction in [0.25, 0.5, 0.75] {
            rectangle(
                window,
                grid.notes.left(),
                grid.velocity.bottom() - grid.velocity.size.height * fraction,
                f32::from(grid.notes.size.width),
                0.5,
                (ds.grid << 8) | 0x50,
            );
        }
    }
}

pub(super) fn rectangle(
    window: &mut Window,
    x: Pixels,
    y: Pixels,
    width: f32,
    height: f32,
    color: u32,
) {
    window.paint_quad(fill(
        Bounds::new(point(x, y), size(px(width.max(0.0)), px(height.max(0.0)))),
        rgba(color),
    ));
}

pub(super) fn label(
    window: &mut Window,
    cx: &mut App,
    text: &str,
    origin: gpui::Point<Pixels>,
    color: u32,
    font_size: f32,
) {
    let run = TextRun {
        len: text.len(),
        font: window.text_style().font(),
        color: rgb(color).into(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let line =
        window
            .text_system()
            .shape_line(text.to_string().into(), px(font_size), &[run], None);
    if let Err(error) = line.paint(
        origin,
        px(font_size + 2.0),
        TextAlign::Left,
        None,
        window,
        cx,
    ) {
        eprintln!("No se pudo dibujar una etiqueta del piano roll: {error}");
    }
}
