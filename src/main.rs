use macroquad::prelude::*;
use macroquad::file::{load_file, save_file};
use serde_json::Value;

#[derive(Clone)]
struct Region {
    name: String,
    polygons: Vec<Vec<Vec<Vec2>>>,
}

#[derive(Clone, Copy, PartialEq)]
enum Level {
    None,
    Yellow,
    Orange,
    Red,
    DarkRed,
}

impl Level {
    fn color(self) -> Color {
        match self {
            Self::None => Color::from_rgba(235, 235, 225, 255),
            Self::Yellow => Color::from_rgba(255, 220, 30, 255),
            Self::Orange => Color::from_rgba(255, 145, 20, 255),
            Self::Red => Color::from_rgba(235, 55, 55, 255),
            Self::DarkRed => Color::from_rgba(125, 15, 30, 255),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::None => "Нет",
            Self::Yellow => "Жёлтый",
            Self::Orange => "Оранжевый",
            Self::Red => "Красный",
            Self::DarkRed => "Тёмно-красный",
        }
    }

    fn from_number(n: u8) -> Self {
        match n {
            1 => Self::Yellow,
            2 => Self::Orange,
            3 => Self::Red,
            4 => Self::DarkRed,
            _ => Self::None,
        }
    }

    fn number(self) -> u8 {
        match self {
            Self::None => 0,
            Self::Yellow => 1,
            Self::Orange => 2,
            Self::Red => 3,
            Self::DarkRed => 4,
        }
    }
}

fn window_conf() -> Conf {
    Conf {
        window_title: "Тривога БПЛА STAN".to_string(),
        window_width: 900,
        window_height: 1400,
        high_dpi: true,
        ..Default::default()
    }
}

fn parse_ring(value: &Value) -> Vec<Vec2> {
    value.as_array()
        .map(|points| {
            points.iter()
                .filter_map(|p| {
                    let a = p.as_array()?;
                    Some(vec2(
                        a.first()?.as_f64()? as f32,
                        a.get(1)?.as_f64()? as f32,
                    ))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn parse_polygon(value: &Value) -> Vec<Vec<Vec2>> {
    value.as_array()
        .map(|rings| rings.iter().map(parse_ring).collect())
        .unwrap_or_default()
}

fn load_regions(data: &[u8]) -> Vec<Region> {
    let json: Value = serde_json::from_slice(data)
        .expect("Invalid GeoJSON");

    let features = json["features"]
        .as_array()
        .expect("GeoJSON has no features");

    let mut result = Vec::new();

    for feature in features {
        let geometry = &feature["geometry"];
        let properties = &feature["properties"];

        let name = properties["shapeName"]
            .as_str()
            .or_else(|| properties["name"].as_str())
            .or_else(|| properties["shapeISO"].as_str())
            .unwrap_or("Region")
            .to_string();

        let polygons = match geometry["type"].as_str() {
            Some("Polygon") => {
                vec![parse_polygon(&geometry["coordinates"])]
            }
            Some("MultiPolygon") => {
                geometry["coordinates"]
                    .as_array()
                    .map(|items| items.iter().map(parse_polygon).collect())
                    .unwrap_or_default()
            }
            _ => continue,
        };

        result.push(Region { name, polygons });
    }

    result
}

fn bounds(regions: &[Region]) -> (f32, f32, f32, f32) {
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;

    for region in regions {
        for polygon in &region.polygons {
            for ring in polygon {
                for p in ring {
                    min_x = min_x.min(p.x);
                    min_y = min_y.min(p.y);
                    max_x = max_x.max(p.x);
                    max_y = max_y.max(p.y);
                }
            }
        }
    }

    (min_x, min_y, max_x, max_y)
}

fn project(
    p: Vec2,
    b: (f32, f32, f32, f32),
    rect: Rect,
    zoom: f32,
    offset: Vec2,
) -> Vec2 {
    let (min_x, min_y, max_x, max_y) = b;

    vec2(
        rect.x + (p.x - min_x) / (max_x - min_x) * rect.w * zoom + offset.x,
        rect.y + (max_y - p.y) / (max_y - min_y) * rect.h * zoom + offset.y,
    )
}

fn inverse_project(
    p: Vec2,
    b: (f32, f32, f32, f32),
    rect: Rect,
    zoom: f32,
    offset: Vec2,
) -> Vec2 {
    let (min_x, min_y, max_x, max_y) = b;

    let x = (p.x - rect.x - offset.x) / (rect.w * zoom);
    let y = (p.y - rect.y - offset.y) / (rect.h * zoom);

    vec2(
        min_x + x * (max_x - min_x),
        max_y - y * (max_y - min_y),
    )
}

fn point_in_ring(p: Vec2, ring: &[Vec2]) -> bool {
    if ring.len() < 3 {
        return false;
    }

    let mut inside = false;
    let mut j = ring.len() - 1;

    for i in 0..ring.len() {
        let a = ring[i];
        let b = ring[j];

        if (a.y > p.y) != (b.y > p.y) {
            let x = (b.x - a.x) * (p.y - a.y)
                / (b.y - a.y) + a.x;

            if p.x < x {
                inside = !inside;
            }
        }

        j = i;
    }

    inside
}

fn contains(region: &Region, p: Vec2) -> bool {
    region.polygons.iter().any(|polygon| {
        if polygon.is_empty() {
            return false;
        }

        point_in_ring(p, &polygon[0])
            && !polygon.iter().skip(1)
                .any(|hole| point_in_ring(p, hole))
    })
}

fn draw_region(
    region: &Region,
    color: Color,
    b: (f32, f32, f32, f32),
    rect: Rect,
    zoom: f32,
    offset: Vec2,
) {
    for polygon in &region.polygons {
        if polygon.is_empty() {
            continue;
        }

        let mut vertices = Vec::new();
        let mut holes = Vec::new();

        for (i, ring) in polygon.iter().enumerate() {
            if i > 0 {
                holes.push(vertices.len() / 2);
            }

            for p in ring {
                vertices.push(p.x as f64);
                vertices.push(p.y as f64);
            }
        }

        if let Ok(indices) = earcutr::earcut(&vertices, &holes, 2) {
            for tri in indices.chunks_exact(3) {
                let get_point = |i: usize| {
                    project(
                        vec2(
                            vertices[i * 2] as f32,
                            vertices[i * 2 + 1] as f32,
                        ),
                        b,
                        rect,
                        zoom,
                        offset,
                    )
                };

                draw_triangle(
                    get_point(tri[0]),
                    get_point(tri[1]),
                    get_point(tri[2]),
                    color,
                );
            }
        }

        for ring in polygon {
            for pair in ring.windows(2) {
                let a = project(pair[0], b, rect, zoom, offset);
                let c = project(pair[1], b, rect, zoom, offset);

                draw_line(a.x, a.y, c.x, c.y, 1.2, DARKGRAY);
            }
        }
    }
}

fn load_saved_levels(count: usize) -> Vec<Level> {
    let mut result = vec![Level::None; count];

    if let Ok(data) = std::fs::read("levels.json") {
        if let Ok(json) = serde_json::from_slice::<Vec<u8>>(&data) {
            for (i, value) in json.iter().enumerate().take(count) {
                result[i] = Level::from_number(*value);
            }
        }
    }

    result
}

fn save_levels(levels: &[Level]) {
    let values: Vec<u8> = levels.iter().map(|l| l.number()).collect();

    if let Ok(data) = serde_json::to_vec(&values) {
        let _ = save_file("levels.json", &data);
    }
}

fn draw_button(rect: Rect, label: &str, color: Color) -> bool {
    draw_rectangle(rect.x, rect.y, rect.w, rect.h, color);

    draw_rectangle_lines(
        rect.x,
        rect.y,
        rect.w,
        rect.h,
        1.5,
        BLACK,
    );

    let font_size = 16;
    let dims = measure_text(label, None, font_size, 1.0);

    draw_text(
        label,
        rect.x + (rect.w - dims.width) / 2.0,
        rect.y + rect.h / 2.0 + dims.height / 3.0,
        font_size as f32,
        BLACK,
    );

    is_mouse_button_pressed(MouseButton::Left)
        && rect.contains(vec2(mouse_position().0, mouse_position().1))
}

#[macroquad::main(window_conf)]
async fn main() {
    let data = load_file("assets/regions.geojson")
        .await
        .expect("Missing assets/regions.geojson");

    let regions = load_regions(&data);

    assert!(!regions.is_empty(), "No regions found");

    let b = bounds(&regions);

    let mut levels = load_saved_levels(regions.len());
    let mut selected = 0usize;

    let mut zoom = 1.0f32;
    let mut offset = vec2(0.0, 0.0);

    let mut last_touch = vec2(0.0, 0.0);
    let mut dragging = false;

    let drone = load_texture("assets/photos/drone.png").await.ok();

    loop {
        clear_background(Color::from_rgba(242, 244, 248, 255));

        let sw = screen_width();
        let sh = screen_height();

        draw_text("Тривога БПЛА STAN", 15.0, 32.0, 27.0, BLACK);

        let panel_height = 175.0;
        let map_top = 45.0;
        let map_bottom = sh - panel_height;

        let max_w = sw - 20.0;
        let max_h = map_bottom - map_top - 10.0;

        let geo_w = b.2 - b.0;
        let geo_h = b.3 - b.1;

        let scale = (max_w / geo_w).min(max_h / geo_h);

        let rect = Rect::new(
            (sw - geo_w * scale) / 2.0,
            map_top + (max_h - geo_h * scale) / 2.0,
            geo_w * scale,
            geo_h * scale,
        );

        for (i, region) in regions.iter().enumerate() {
            draw_region(
                region,
                levels[i].color(),
                b,
                rect,
                zoom,
                offset,
            );
        }

        // Select region by tapping the map.
        let mouse = vec2(mouse_position().0, mouse_position().1);

        if is_mouse_button_pressed(MouseButton::Left)
            && mouse.y < map_bottom
        {
            let geo = inverse_project(mouse, b, rect, zoom, offset);

            for (i, region) in regions.iter().enumerate() {
                if contains(region, geo) {
                    selected = i;
                    break;
                }
            }

            last_touch = mouse;
            dragging = true;
        }

        if is_mouse_button_down(MouseButton::Left) && dragging {
            let delta = mouse - last_touch;
            offset += delta;
            last_touch = mouse;
        }

        if is_mouse_button_released(MouseButton::Left) {
            dragging = false;
        }

        // Mouse wheel zoom.
        let (_, wheel) = mouse_wheel();

        if wheel != 0.0 {
            zoom = (zoom + wheel * 0.08).clamp(0.7, 5.0);
        }

        // Region information.
        let panel_y = sh - panel_height + 12.0;

        draw_text(
            &format!("Область: {}", regions[selected].name),
            14.0,
            panel_y + 20.0,
            19.0,
            BLACK,
        );

        draw_text(
            &format!("Уровень: {}", levels[selected].label()),
            14.0,
            panel_y + 43.0,
            17.0,
            BLACK,
        );

        let buttons = [
            ("Сброс", Level::None),
            ("Жёлтый", Level::Yellow),
            ("Оранжевый", Level::Orange),
            ("Красный", Level::Red),
            ("Тёмный", Level::DarkRed),
        ];

        let gap = 4.0;
        let bw = (sw - 20.0 - gap * 4.0) / 5.0;

        for (i, (label, level)) in buttons.iter().enumerate() {
            let button_rect = Rect::new(
                10.0 + i as f32 * (bw + gap),
                panel_y + 55.0,
                bw,
                48.0,
            );

            if draw_button(button_rect, label, level.color()) {
                levels[selected] = *level;
                save_levels(&levels);
            }
        }

        if let Some(texture) = &drone {
            draw_texture_ex(
                texture,
                sw - 52.0,
                8.0,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(vec2(36.0, 36.0)),
                    ..Default::default()
                },
            );
        }

        draw_text(
            "Ручная гражданская доска предупреждений",
            12.0,
            sh - 10.0,
            13.0,
            DARKGRAY,
        );

        next_frame().await;
    }
            }
