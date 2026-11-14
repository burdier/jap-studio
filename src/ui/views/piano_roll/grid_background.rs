//! Teclado, rejilla y regla del piano roll.
use crate::ui::{
    DesignSystem,
    components::bar_grid,
    piano_roll::geometry::{Grid, KEYBOARD_WIDTH},
    time_grid::musical,
};
use elevate::musical::{keyboard::is_black, pitch::note_name, scale::PitchScale};
use gpui::{App, Bounds, Pixels, TextAlign, TextRun, Window, fill, point, px, rgb, rgba, size};

pub(super) fn paint_rows(
    grid: &Grid,
    ds: &DesignSystem,
    highlight: bool,
    show_scale_keyboard: bool,
    scale: PitchScale,
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
        if highlight && scale.contains(pitch) {
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
        if show_scale_keyboard && scale.contains(pitch) {
            rectangle(
                window,
                keys_left,
                y,
                keys_width * if is_black(pitch) { 0.57 } else { 1.0 },
                grid.row_height,
                (ds.accent << 8) | 0x38,
            );
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
    meter: elevate::musical::time::Meter,
    resolution: elevate::musical::grid::NoteGrid,
    window: &mut Window,
    cx: &mut App,
) {
    let bar_length = meter.beats_per_bar();
    let width = f64::from(f32::from(grid.notes.size.width)).min(grid.axis().x_at(beats).max(0.0));
    let time_bounds = Bounds::new(
        point(grid.notes.left(), grid.all.top()),
        size(px(width as f32), grid.all.size.height - px(10.0)),
    );
    if bar_length * grid.axis().pixels_per_beat >= 8.0 {
        bar_grid::paint_bands(time_bounds, grid.axis(), bar_length, ds, window);
    }
    let marks = musical::marks(
        grid.axis(),
        meter,
        width,
        f64::from(ds.font_size_xs),
        Some(resolution),
    );
    let ruler = Bounds::new(
        time_bounds.origin,
        size(time_bounds.size.width, grid.notes.top() - grid.all.top()),
    );
    for (origin, line) in bar_grid::ruler_labels(ruler, grid.axis(), &marks, ds, window) {
        let _ = line.paint(origin, ruler.size.height, TextAlign::Left, None, window, cx);
    }
    for mark in marks {
        let x = grid.notes.left() + px(grid.axis().x_at(mark.beat).round() as f32);
        let (thickness, tick_height, color) = bar_grid::mark_style(mark.kind, ds);
        rectangle(
            window,
            x,
            grid.notes.top(),
            thickness,
            f32::from(time_bounds.bottom() - grid.notes.top()),
            color,
        );
        rectangle(
            window,
            x,
            grid.notes.top() - px(tick_height),
            thickness,
            tick_height,
            color,
        );
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
