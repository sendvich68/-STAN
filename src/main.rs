use macroquad::prelude::*;
use macroquad::file::load_file;
use serde_json::Value;

type Ring = Vec<Vec2>;
type Polygon = Vec<Ring>;

#[derive(Clone)]
struct Region {
    name: String,
    polygons: Vec<Polygon>,
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
            Level::None => Color::from_rgba(238, 238, 238, 255),
            Level::Yellow => Color::from_rgba(255, 220, 30, 255),
            Level::Orange => Color::from_rgba(255, 145, 20, 255),
            Level::Red => Color::from_rgba(235, 55, 55, 255),
            Level::DarkRed => Color::from_rgba(125, 15, 30, 255),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Level::None => "Нет тревоги",
            Level::Yellow => "Жёлтый",
            Level::Orange => "Оранжевый",
            Level::Red => "Красный",
            Level::DarkRed => "Тёмно-красный",
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

fn parse_ring(value: &Value) -> Ring {
    value.as_array()
        .map(|points| {
            points.iter()
                .filter_map(|p| {
                    let a = p.as_array()?;
                    Some(vec2(
                        a.get(0)?.as_f64()? as f32,
                        a.get(1)?.as_f64()? as f32,
                    ))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn parse_polygon(value: &Value) -> Polygon {
    value.as_array()
        .map(|rings| rings.iter().map(parse_ring).collect())
        .unwrap_or_default()
}

fn load_regions(data: &[u8]) -> Vec<Region> {
    let json: Value = serde_json::from_slice(data)
        .expect("Ошибка чтения GeoJSON");

    let features = json["features"]
        .as_array()
        .expect("В GeoJSON отсутствует features");

    let mut regions = Vec::new();

    for feature in features {
        let geometry = &feature["geometry"];
        let properties = &feature["properties"];

        let name = properties["shapeName"]
            .as_str()
            .or_else(|| properties["name"].as_str())
            .unwrap_or("Область")
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

        if !polygons.is_empty() {
            regions.push(Region { name, polygons });
        }
    }

    regions
}

fn get_bounds(regions: &[Region]) -> (f32, f32, f32, f32) {
    let mut min_x = f32::MAX;
    let mut max_x = f32::MIN;
    let mut min_y = f32::MAX;
    let mut max_y = f32::MIN;

    for region in regions {
        for polygon in &region.polygons {
            for ring in polygon {
                for p in ring {
                    min_x = min_x.min(p.x);
                    max_x = max_x.max(p.x);
                    min_y = min_y.min(p.y);
                    max_y = max_y.max(p.y);
                }
            }
        }
    }

    (min_x, min_y, max_x, max_y)
}

fn project(
    p: Vec2,
    bounds: (f32, f32, f32, f32),
    rect: Rect,
) -> Vec2 {
    let (min_x, min_y, max_x, max_y) = bounds;

    vec2(
        rect.x + (p.x - min_x) / (max_x - min_x) * rect.w,
        rect.y + (max_y - p.y) / (max_y - min_y) * rect.h,
    )
}

fn inverse_project(
    p: Vec2,
    bounds: (f32, f32, f32, f32),
    rect: Rect,
) -> Vec2 {
    let (min_x, min_y, max_x, max_y) = bounds;

    vec2(
        min_x + (p.x - rect.x) / rect.w * (max_x - min_x),
        max_y - (p.y - rect.y) / rect.h * (max_y - min_y),
    )
}

fn point_in_ring(p: Vec2, ring: &[Vec2]) -> bool {
    let mut inside = false;

    if ring.len() < 3 {
        return false;
    }

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

fn region_contains(region: &Region, p: Vec2) -> bool {
    for polygon in &region.polygons {
        if polygon.is_empty() {
            continue;
        }

        if point_in_ring(p, &polygon[0])
            && !polygon.iter().skip(1).any(|hole| point_in_ring(p, hole))
        {
            return true;
        }
    }

    false
}

fn draw_region(
    region: &Region,
    color: Color,
    bounds: (f32, f32, f32, f32),
    rect: Rect,
) {
    for polygon in &region.polygons {
        if polygon.is_empty() {
            continue;
        }

        let mut flat: Vec<f64> = Vec::new();
        let mut holes: Vec<usize> = Vec::new();

        for (ring_index, ring) in polygon.iter().enumerate() {
            if ring_index > 0 {
                holes.push(flat.len() / 2);
            }

            for p in ring {
                flat.push(p.x as f64);
                flat.push(p.y as f64);
            }
        }

        if let Ok(indices) = earcutr::earcut(&flat, &holes, 2) {
            for triangle in indices.chunks_exact(3) {
                let point = |index: usize| {
                    project(
                        vec2(
                            flat[index * 2] as f32,
                            flat[index * 2 + 1] as f32,
                        ),
                        bounds,
                        rect,
                    )
                };

                draw_triangle(
                    point(triangle[0]),
                    point(triangle[1]),
                    point(triangle[2]),
                    color,
                );
            }
        }

        // Обводка границ
        for ring in polygon {
            for pair in ring.windows(2) {
                let a = project(pair[0], bounds, rect);
                let b = project(pair[1], bounds, rect);

                draw_line(a.x, a.y, b.x, b.y, 1.5, DARKGRAY);
            }
        }
    }
}

fn draw_button(
    rect: Rect,
    label: &str,
    color: Color,
    font: Option<&Font>,
) -> bool {
    draw_rectangle(rect.x, rect.y, rect.w, rect.h, color);

    draw_rectangle_lines(
        rect.x,
        rect.y,
        rect.w,
        rect.h,
        2.0,
        BLACK,
    );

    draw_text_ex(
        label,
        rect.x + 5.0,
        rect.y + rect.h / 2.0 + 7.0,
        TextParams {
            font,
            font_size: 17,
            color: BLACK,
            ..Default::default()
        },
    );

    is_mouse_button_pressed(MouseButton::Left)
        && rect.contains(vec2(mouse_position().0, mouse_position().1))
}

#[macroquad::main(window_conf)]
async fn main() {
    let data = load_file("assets/regions.geojson")
        .await
        .expect("Не найден assets/regions.geojson");

    let font = load_ttf_font("assets/DejaVuSans.ttf")
        .await
        .ok();

    let regions = load_regions(&data);

    assert!(!regions.is_empty(), "Карта не содержит областей");

    let bounds = get_bounds(&regions);

    let mut levels = vec![Level::None; regions.len()];
    let mut selected: usize = 0;

    loop {
        clear_background(Color::from_rgba(242, 244, 248, 255));

        let sw = screen_width();
        let sh = screen_height();

        draw_text_ex(
            "Тривога БПЛА STAN",
            16.0,
            36.0,
            TextParams {
                font: font.as_ref(),
                font_size: 28,
                color: BLACK,
                ..Default::default()
            },
        );

        let max_w = sw - 24.0;
        let max_h = sh * 0.63;

        let geo_w = bounds.2 - bounds.0;
        let geo_h = bounds.3 - bounds.1;

        let scale = (max_w / geo_w).min(max_h / geo_h);

        let map_w = geo_w * scale;
        let map_h = geo_h * scale;

        let map_rect = Rect::new(
            (sw - map_w) / 2.0,
            55.0 + (max_h - map_h) / 2.0,
            map_w,
            map_h,
        );

        // Рисуем все регионы
        for (i, region) in regions.iter().enumerate() {
            draw_region(
                region,
                levels[i].color(),
                bounds,
                map_rect,
            );
        }

        // Выбор региона касанием
        let mouse = vec2(mouse_position().0, mouse_position().1);

        if is_mouse_button_pressed(MouseButton::Left)
            && map_rect.contains(mouse)
        {
            let geo_point = inverse_project(mouse, bounds, map_rect);

            for (i, region) in regions.iter().enumerate() {
                if region_contains(region, geo_point) {
                    selected = i;
                    break;
                }
            }
        }

        let panel_y = sh - 160.0;

        draw_text_ex(
            &format!(
                "Регион: {}",
                regions[selected].name
            ),
            16.0,
            panel_y,
            TextParams {
                font: font.as_ref(),
                font_size: 19,
                color: BLACK,
                ..Default::default()
            },
        );

        draw_text_ex(
            &format!("Уровень: {}", levels[selected].name()),
            16.0,
            panel_y + 27.0,
            TextParams {
                font: font.as_ref(),
                font_size: 17,
                color: BLACK,
                ..Default::default()
            },
        );

        let buttons = [
            ("Сброс", Level::None),
            ("Жёлтый", Level::Yellow),
            ("Оранжевый", Level::Orange),
            ("Красный", Level::Red),
            ("Тёмный", Level::DarkRed),
        ];

        let gap = 5.0;
        let bw = (sw - 24.0 - gap * 4.0) / 5.0;

        for (i, (label, level)) in buttons.iter().enumerate() {
            let rect = Rect::new(
                12.0 + i as f32 * (bw + gap),
                panel_y + 45.0,
                bw,
                53.0,
            );

            if draw_button(
                rect,
                label,
                level.color(),
                font.as_ref(),
            ) {
                levels[selected] = *level;
            }
        }

        draw_text_ex(
            "Выбери область на карте и установи её цвет.",
            12.0,
            sh - 12.0,
            TextParams {
                font: font.as_ref(),
                font_size: 13,
                color: DARKGRAY,
                ..Default::default()
            },
        );

        next_frame().await;
    }
}
