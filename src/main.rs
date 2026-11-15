use macroquad::prelude::*;

const REGIONS: [&str; 25] = [
    "Вінницька", "Волинська", "Дніпропетровська", "Донецька", "Житомирська",
    "Закарпатська", "Запорізька", "Івано-Франківська", "Київська", "Кіровоградська",
    "Луганська", "Львівська", "Миколаївська", "Одеська", "Полтавська",
    "Рівненська", "Сумська", "Тернопільська", "Харківська", "Херсонська",
    "Хмельницька", "Черкаська", "Чернівецька", "Чернігівська", "м. Київ",
];

#[derive(Clone, Copy, PartialEq)]
enum Level { None, Yellow, Orange, Red, DarkRed }

impl Level {
    fn color(self) -> Color {
        match self {
            Level::None => Color::from_rgba(225, 225, 225, 255),
            Level::Yellow => Color::from_rgba(255, 220, 45, 255),
            Level::Orange => Color::from_rgba(255, 145, 35, 255),
            Level::Red => Color::from_rgba(235, 65, 65, 255),
            Level::DarkRed => Color::from_rgba(135, 20, 35, 255),
        }
    }
    fn label(self) -> &'static str {
        match self {
            Level::None => "Немає",
            Level::Yellow => "Жовтий",
            Level::Orange => "Помаранчевий",
            Level::Red => "Червоний",
            Level::DarkRed => "Темно-червоний",
        }
    }
}

fn window_conf() -> Conf {
    Conf {
        window_title: "Тривога БПЛА STAN".to_owned(),
        window_width: 900,
        window_height: 1500,
        high_dpi: true,
        fullscreen: false,
        ..Default::default()
    }
}

fn draw_button(rect: Rect, text: &str, fill: Color, selected: bool) -> bool {
    draw_rectangle(rect.x, rect.y, rect.w, rect.h, fill);
    draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, if selected { 4.0 } else { 1.0 },
        if selected { WHITE } else { DARKGRAY });
    let fs = 20.0_f32.min(rect.h * 0.42);
    let dims = measure_text(text, None, fs as u16, 1.0);
    draw_text(text, rect.x + (rect.w - dims.width) / 2.0,
        rect.y + (rect.h + fs * 0.35) / 2.0, fs, BLACK);
    is_mouse_button_pressed(MouseButton::Left)
        && rect.contains(vec2(mouse_position().0, mouse_position().1))
}

#[macroquad::main(window_conf)]
async fn main() {
    let map = load_texture("assets/map.jpg").await.ok();
    if let Some(t) = &map { t.set_filter(FilterMode::Linear); }

    // Optional user-provided decorative marker image:
    // put the file at assets/photos/беспилотник.png.
    let drone = load_texture("assets/photos/беспилотник.png").await.ok();

    let mut levels = vec![Level::None; REGIONS.len()];
    let mut selected_region: usize = 0;
    let mut current_level = Level::Yellow;
    let mut marker = vec2(0.5, 0.5); // normalized map position; visual/demo marker only
    let mut dragging = false;
    let mut map_rect = Rect::new(0.0, 0.0, 1.0, 1.0);
    let mut scroll = 0.0;

    loop {
        clear_background(Color::from_rgba(242, 244, 247, 255));
        let sw = screen_width();
        let sh = screen_height();

        draw_text("Тривога БПЛА STAN", 18.0, 38.0, 30.0, BLACK);
        draw_text("Навчальна дошка цивільних сповіщень", 18.0, 64.0, 17.0, DARKGRAY);

        // Map fitted into the upper portion of the screen.
        let top = 82.0;
        let bottom = sh * 0.57;
        let available_w = sw - 24.0;
        let available_h = bottom - top - 8.0;
        let (mw, mh) = if let Some(t) = &map {
            let scale = (available_w / t.width()).min(available_h / t.height());
            (t.width() * scale, t.height() * scale)
        } else { (available_w, available_h) };
        map_rect = Rect::new((sw - mw) / 2.0, top, mw, mh);
        draw_rectangle(map_rect.x, map_rect.y, map_rect.w, map_rect.h, WHITE);
        if let Some(t) = &map {
            draw_texture_ex(t, map_rect.x, map_rect.y, WHITE,
                DrawTextureParams { dest_size: Some(vec2(map_rect.w, map_rect.h)), ..Default::default() });
        } else {
            draw_text("Не знайдено assets/map.jpg", 28.0, top + 45.0, 22.0, RED);
        }

        // A manually positioned decorative marker, not live aircraft tracking.
        let mx = map_rect.x + marker.x * map_rect.w;
        let my = map_rect.y + marker.y * map_rect.h;
        if let Some(t) = &drone {
            draw_texture_ex(t, mx - 22.0, my - 22.0, WHITE,
                DrawTextureParams { dest_size: Some(vec2(44.0, 44.0)), ..Default::default() });
        } else {
            draw_circle(mx, my, 13.0, Color::from_rgba(30, 90, 160, 230));
            draw_circle_lines(mx, my, 13.0, 2.0, WHITE);
            draw_text("БПЛА", mx + 15.0, my - 8.0, 16.0, BLACK);
        }

        let mouse = vec2(mouse_position().0, mouse_position().1);
        if is_mouse_button_pressed(MouseButton::Left) && map_rect.contains(mouse) {
            let dx = mouse.x - mx;
            let dy = mouse.y - my;
            if dx * dx + dy * dy < 42.0 * 42.0 { dragging = true; }
        }
        if is_mouse_button_down(MouseButton::Left) && dragging && map_rect.w > 0.0 && map_rect.h > 0.0 {
            marker.x = ((mouse.x - map_rect.x) / map_rect.w).clamp(0.0, 1.0);
            marker.y = ((mouse.y - map_rect.y) / map_rect.h).clamp(0.0, 1.0);
        }
        if is_mouse_button_released(MouseButton::Left) { dragging = false; }

        // Selected region + severity controls.
        let panel_y = bottom + 12.0;
        draw_text("Оберіть область:", 18.0, panel_y + 22.0, 22.0, BLACK);
        let cols = 3;
        let gap = 7.0;
        let cell_w = (sw - 36.0 - gap * 2.0) / cols as f32;
        let cell_h = 38.0;
        let list_top = panel_y + 34.0;
        let visible_rows = ((sh - list_top - 210.0) / (cell_h + gap)).floor().max(1.0) as usize;
        let max_scroll = REGIONS.len().div_ceil(cols).saturating_sub(visible_rows) as f32;
        scroll = scroll.clamp(0.0, max_scroll);
        for (i, name) in REGIONS.iter().enumerate() {
            let row = i / cols;
            let col = i % cols;
            let y = list_top + (row as f32 - scroll) * (cell_h + gap);
            if y < list_top - cell_h || y > sh - 220.0 { continue; }
            let rect = Rect::new(18.0 + col as f32 * (cell_w + gap), y, cell_w, cell_h);
            let selected = selected_region == i;
            if draw_button(rect, name, levels[i].color(), selected) { selected_region = i; }
        }

        let controls_y = sh - 164.0;
        draw_text(&format!("Область: {} — {}", REGIONS[selected_region], levels[selected_region].label()),
            18.0, controls_y - 12.0, 19.0, BLACK);
        let labels = [
            ("Скинути", Level::None), ("Жовтий", Level::Yellow),
            ("Помаранч.", Level::Orange), ("Червоний", Level::Red),
            ("Темно-черв.", Level::DarkRed),
        ];
        let bw = (sw - 36.0 - 4.0 * 6.0) / 5.0;
        for (i, (label, lvl)) in labels.iter().enumerate() {
            let rect = Rect::new(18.0 + i as f32 * (bw + 6.0), controls_y, bw, 48.0);
            if draw_button(rect, label, lvl.color(), current_level == *lvl) {
                current_level = *lvl;
                levels[selected_region] = *lvl;
            }
        }

        draw_text("Прокрутіть список областей свайпом/коліщатком.", 18.0, sh - 91.0, 15.0, DARKGRAY);
        draw_text("Маркер на карті — лише ручна позначка для макета; GPS та live-відстеження немає.",
            18.0, sh - 66.0, 13.0, DARKGRAY);

        // Simple wheel scrolling on desktop; touch drag scroll can be added later.
        let (_, wheel_y) = mouse_wheel();
        if wheel_y.abs() > 0.0 { scroll = (scroll - wheel_y.signum()).clamp(0.0, max_scroll); }

        next_frame().await;
    }
}
