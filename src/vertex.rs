use luminance_derive::{Vertex, Semantics};
#[derive(Copy, Clone, Debug, Semantics)]
pub enum Semantics {
  #[sem(name = "position", repr = "[f32; 2]", wrapper = "VPos")]
  Position,
  #[sem(name = "radius", repr = "f32", wrapper = "VRadius")]
  Radius,
  #[sem(name = "color", repr = "[f32; 3]", wrapper = "VColor")]
  Color,
}

#[derive(Clone, Debug, Vertex)]
#[vertex(sem = "Semantics")]
#[repr(C)]
pub struct LineVertex(VPos);

#[derive(Clone, Debug, Vertex)]
#[vertex(sem = "Semantics")]
#[repr(C)]
pub struct PointVertex(pub VPos, pub VColor, pub VRadius);