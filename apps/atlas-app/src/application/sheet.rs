use super::Application;
use crate::scene::sheet::{SheetPoint, points};
use atlas_boundary::sheet::SheetTrace;
use atlas_geo::registration::PlanarPoint;
use egui::{Align2, Color32, FontId, Pos2, Rect, Sense, Shape, Stroke, Vec2};

pub(super) struct SheetView {
    pub visible: bool,
    zoom: f32,
    pan: Vec2,
}

impl Default for SheetView {
    fn default() -> Self {
        Self {
            visible: false,
            zoom: 1.0,
            pan: Vec2::ZERO,
        }
    }
}

impl SheetView {
    pub(super) fn open() -> Self {
        Self {
            visible: true,
            ..Default::default()
        }
    }
    fn gestures(&mut self, ui: &egui::Ui, response: &egui::Response) {
        if response.dragged() {
            self.pan += ui.input(|i| i.pointer.delta());
        }
        if response.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            let old = self.zoom;
            self.zoom = (old * (scroll / 300.0).exp()).clamp(0.5, 8.0);
            if let Some(pointer) = response.hover_pos() {
                let anchor = pointer - response.rect.center();
                self.pan = anchor - (anchor - self.pan) * (self.zoom / old);
            }
        }
    }
}

impl Application {
    pub(super) fn sheet_panel(&mut self, ui: &mut egui::Ui) -> bool {
        let drawing = self
            .selected
            .and_then(|id| self.atlas.drawings(id).first().copied().cloned());
        let Some(drawing) = drawing else {
            return false;
        };
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.sheet.visible, false, "Geographic map");
            ui.selectable_value(&mut self.sheet.visible, true, "Source drawing");
            if self.sheet.visible && ui.button("Fit drawing").clicked() {
                self.sheet.zoom = 1.0;
                self.sheet.pan = Vec2::ZERO;
            }
        });
        if !self.sheet.visible {
            return false;
        }
        if let Some(source) = self
            .atlas
            .sources
            .sources
            .iter()
            .find(|s| s.id == drawing.source_id)
        {
            ui.strong(&source.title);
        }
        let page = drawing
            .units
            .first()
            .and_then(|u| self.atlas.data.records.iter().find(|r| r.id == u.record_id))
            .and_then(|r| r.page_or_sheet.as_deref())
            .unwrap_or("—");
        ui.label(format!(
            "Printed page {page} · Scan page {} · Sheet coordinates · Drag to pan · Scroll to zoom",
            drawing.scan_page
        ));
        let points = points(&self.atlas, &drawing);
        self.draw_sheet(ui, &drawing, &points);
        true
    }

    fn draw_sheet(&mut self, ui: &mut egui::Ui, drawing: &SheetTrace, points: &[SheetPoint]) {
        let (rect, response) = ui.allocate_exact_size(
            ui.available_size().max(Vec2::splat(1.0)),
            Sense::click_and_drag(),
        );
        self.sheet.gestures(ui, &response);
        let bounds = bounds(drawing);
        let fit = ((rect.width() - 120.0) / bounds.width())
            .min((rect.height() - 60.0) / bounds.height())
            .max(0.01);
        let transform = |p: PlanarPoint| {
            rect.center()
                + self.sheet.pan
                + (egui::pos2(p.x as f32, p.y as f32) - bounds.center()) * fit * self.sheet.zoom
        };
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, Color32::from_rgb(248, 246, 234));
        boundaries(&painter, drawing, transform);
        let mut labels = Vec::new();
        for point in points {
            let pos = transform(point.point);
            let selected = self.selected.is_some_and(|id| point.entities.contains(&id));
            let color = if selected {
                Color32::from_rgb(203, 91, 40)
            } else {
                Color32::from_rgb(27, 89, 101)
            };
            painter.circle_filled(pos, if point.administrative { 5.0 } else { 3.5 }, color);
            label(&painter, pos, point, color, &mut labels);
        }
        let hits = response
            .hover_pos()
            .map(|p| crate::scene::sheet::pick(points, |v| transform(v).distance(p)))
            .unwrap_or_default();
        if response.clicked()
            && let Some(id) = hits.first().copied()
        {
            self.hits = hits;
            self.select(id, false);
        } else if !hits.is_empty() {
            response.on_hover_text(
                hits.iter()
                    .map(|id| self.atlas.label(*id))
                    .collect::<Vec<_>>()
                    .join(" / "),
            );
        }
        painter.text(
            rect.left_bottom() + egui::vec2(12.0, -12.0),
            Align2::LEFT_BOTTOM,
            "Commune boundary · Settlement and administrative seats",
            FontId::proportional(12.0),
            Color32::DARK_GRAY,
        );
    }
}

fn bounds(drawing: &SheetTrace) -> Rect {
    drawing
        .vertices
        .iter()
        .chain(drawing.places.iter().map(|p| &p.point))
        .fold(Rect::NOTHING, |rect, p| {
            rect.union(Rect::from_min_max(
                egui::pos2(p.x as f32, p.y as f32),
                egui::pos2(p.x as f32, p.y as f32),
            ))
        })
}

fn label(
    painter: &egui::Painter,
    pos: Pos2,
    point: &SheetPoint,
    color: Color32,
    occupied: &mut Vec<Rect>,
) {
    let text = point.names.first().map_or("", String::as_str);
    let galley = painter.layout_no_wrap(text.into(), FontId::proportional(13.0), color);
    let size = galley.size();
    let candidates = [7.0, 18.0, 29.0, 40.0].into_iter().flat_map(|gap| {
        [
            pos + egui::vec2(gap, -size.y / 2.0),
            pos - egui::vec2(size.x + gap, size.y / 2.0),
            pos + egui::vec2(gap, -size.y - gap),
            pos + egui::vec2(gap, gap),
            pos - egui::vec2(size.x + gap, size.y + gap),
            pos + egui::vec2(-size.x - gap, gap),
            pos - egui::vec2(size.x / 2.0, size.y + gap),
            pos + egui::vec2(-size.x / 2.0, gap),
        ]
    });
    if let Some(rect) = candidates
        .map(|p| Rect::from_min_size(p, size))
        .min_by_key(|r| occupied.iter().filter(|o| o.intersects(*r)).count())
    {
        occupied.push(rect.expand(2.0));
        let nearest = rect.clamp(pos);
        if nearest.distance(pos) > 12.0 {
            painter.line_segment([pos, nearest], Stroke::new(0.5, color.gamma_multiply(0.5)));
        }
        painter.galley(rect.min, galley, color);
    }
}

fn boundaries(
    painter: &egui::Painter,
    drawing: &SheetTrace,
    transform: impl Fn(PlanarPoint) -> Pos2,
) {
    for unit in &drawing.units {
        for polygon in &unit.polygons {
            for ring in std::iter::once(&polygon.exterior).chain(&polygon.interiors) {
                let vertices = ring
                    .iter()
                    .filter_map(|i| drawing.vertices.get(*i))
                    .map(|p| transform(*p))
                    .collect();
                painter.add(Shape::line(
                    vertices,
                    Stroke::new(2.5, Color32::from_rgb(44, 94, 87)),
                ));
            }
        }
    }
}
