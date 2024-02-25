// Imports
use super::{
    Arrow, CubicBezier, Ellipse, Line, Polygon, Polyline, QuadraticBezier, Rectangle, Shapeable,
};
use crate::transform::Transformable;
use p2d::bounding_volume::Aabb;
use serde::{Deserialize, Serialize};

/// Shape, storing shape variants.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename = "shape")]
pub enum Shape {
    /// A line shape.
    #[serde(rename = "line")]
    Line(Line),
    /// An arrow shape.
    #[serde(rename = "arrow")]
    Arrow(Arrow),
    /// A rectangle shape.
    #[serde(rename = "rect")]
    Rectangle(Rectangle),
    /// An ellipse shape.
    #[serde(rename = "ellipse")]
    Ellipse(Ellipse),
    /// A quadratic bezier curve shape.
    #[serde(rename = "quadbez")]
    QuadraticBezier(QuadraticBezier),
    /// A cubic bezier curve shape.
    #[serde(rename = "cubbez")]
    CubicBezier(CubicBezier),
    /// A polyline shape.
    #[serde(rename = "polyline")]
    Polyline(Polyline),
    /// A polygon shape.
    #[serde(rename = "polygon")]
    Polygon(Polygon),
}

impl Default for Shape {
    fn default() -> Self {
        Self::Line(Line::default())
    }
}

impl Transformable for Shape {
    fn translate(&mut self, offset: na::Vector2<f64>) {
        match self {
            Self::Arrow(arrow) => {
                arrow.translate(offset);
            }
            Self::Line(line) => {
                line.translate(offset);
            }
            Self::Rectangle(rectangle) => {
                rectangle.translate(offset);
            }
            Self::Ellipse(ellipse) => {
                ellipse.translate(offset);
            }
            Self::QuadraticBezier(quadbez) => {
                quadbez.translate(offset);
            }
            Self::CubicBezier(cubbez) => {
                cubbez.translate(offset);
            }
            Self::Polyline(polyline) => {
                polyline.translate(offset);
            }
            Self::Polygon(polygon) => {
                polygon.translate(offset);
            }
        }
    }

    fn rotate(&mut self, angle: f64, center: na::Point2<f64>) {
        match self {
            Self::Line(line) => {
                line.rotate(angle, center);
            }
            Self::Arrow(arrow) => {
                arrow.rotate(angle, center);
            }
            Self::Rectangle(rectangle) => {
                rectangle.rotate(angle, center);
            }
            Self::Ellipse(ellipse) => {
                ellipse.rotate(angle, center);
            }
            Self::QuadraticBezier(quadbez) => {
                quadbez.rotate(angle, center);
            }
            Self::CubicBezier(cubbez) => {
                cubbez.rotate(angle, center);
            }
            Self::Polyline(polyline) => {
                polyline.rotate(angle, center);
            }
            Self::Polygon(polygon) => {
                polygon.rotate(angle, center);
            }
        }
    }

    fn scale(&mut self, scale_stroke: na::Vector2<f64>, scale_resize: na::Vector2<f64>) {
        match self {
            Self::Line(line) => {
                line.scale(scale_stroke, scale_resize);
            }
            Self::Arrow(arrow) => {
                arrow.scale(scale_stroke, scale_resize);
            }
            Self::Rectangle(rectangle) => {
                rectangle.scale(scale_stroke, scale_resize);
            }
            Self::Ellipse(ellipse) => {
                ellipse.scale(scale_stroke, scale_resize);
            }
            Self::QuadraticBezier(quadbez) => {
                quadbez.scale(scale_stroke, scale_resize);
            }
            Self::CubicBezier(cubbez) => {
                cubbez.scale(scale_stroke, scale_resize);
            }
            Self::Polyline(polyline) => {
                polyline.scale(scale_stroke, scale_resize);
            }
            Self::Polygon(polygon) => {
                polygon.scale(scale_stroke, scale_resize);
            }
        }
    }
}

impl Shapeable for Shape {
    fn bounds(&self) -> Aabb {
        match self {
            Self::Arrow(arrow) => arrow.bounds(),
            Self::Line(line) => line.bounds(),
            Self::Rectangle(rectangle) => rectangle.bounds(),
            Self::Ellipse(ellipse) => ellipse.bounds(),
            Self::QuadraticBezier(quadbez) => quadbez.bounds(),
            Self::CubicBezier(cubbez) => cubbez.bounds(),
            Self::Polyline(polyline) => polyline.bounds(),
            Self::Polygon(polygon) => polygon.bounds(),
        }
    }

    fn hitboxes(&self) -> Vec<Aabb> {
        match self {
            Self::Arrow(arrow) => arrow.hitboxes(),
            Self::Line(line) => line.hitboxes(),
            Self::Rectangle(rectangle) => rectangle.hitboxes(),
            Self::Ellipse(ellipse) => ellipse.hitboxes(),
            Self::QuadraticBezier(quadbez) => quadbez.hitboxes(),
            Self::CubicBezier(cubbez) => cubbez.hitboxes(),
            Self::Polyline(polyline) => polyline.hitboxes(),
            Self::Polygon(polygon) => polygon.hitboxes(),
        }
    }

    fn outline_path(&self) -> kurbo::BezPath {
        match self {
            Self::Arrow(arrow) => arrow.outline_path(),
            Self::Line(line) => line.outline_path(),
            Self::Rectangle(rectangle) => rectangle.outline_path(),
            Self::Ellipse(ellipse) => ellipse.outline_path(),
            Self::QuadraticBezier(quadbez) => quadbez.outline_path(),
            Self::CubicBezier(cubbez) => cubbez.outline_path(),
            Self::Polyline(polyline) => polyline.outline_path(),
            Self::Polygon(polygon) => polygon.outline_path(),
        }
    }
}
