use cgmath::Vector2;
use luminance::{context::GraphicsContext, tess::Mode};
use luminance_front::{
  tess::{Tess, TessBuilder, TessError},
  Backend,
};
use splines::{Interpolation, Key, Spline};

use crate::vertex::{LineVertex, PointVertex, VColor, VPos, VRadius};

const DELTA_T: f32 = 0.01;
const POINT_SELECTION_DIST: f32 = DELTA_T * 8.;

/// Position on screen.
pub type ScreenPos = Vector2<f32>;

/// Editor object.
pub struct Editor {
  // The actual spline the user is editing.
  spline: Spline<f32, ScreenPos>,
  // Currently selected content.
  selection: Option<Selection>,
  // List of display points.
  points: Tess<PointVertex, ()>,
  // List of lines.
  lines: Tess<LineVertex, u32>,
  // Hint to know whether tessellations should be rebuilt.
  rebuild_tess: bool,
}

impl Editor {
  /// Create a default editor.
  pub fn new<C>(ctx: &mut C) -> Self
  where
    C: GraphicsContext<Backend = Backend>,
  {
    let spline = Spline::from_vec(Vec::new());
    let selection = None;
    let points = TessBuilder::new(ctx).set_vertex_nb(1).build().unwrap();
    let lines = TessBuilder::new(ctx).set_vertex_nb(1).build().unwrap();
    let rebuild_tess = false;

    Editor {
      spline,
      selection,
      points,
      lines,
      rebuild_tess,
    }
  }

  /// Rebuild tessellation based on control points for lines.
  fn build_lines<C>(&mut self, ctx: &mut C) -> Result<(), EditorError>
  where
    C: GraphicsContext<Backend = Backend>,
  {
    let mut vertices = Vec::new(); // vertices making the lines
    let mut indices = Vec::new(); // indexed lines
    let mut index = 0u32;
    const PRIM_RESTART_INDEX: u32 = u32::max_value();
    let keys = self.spline.keys();

    if !keys.is_empty() {
      let up_t = keys.last().unwrap().t;
      let mut t = keys[0].t;

      while t < up_t {
        let (mut p, key, _) = self.spline.clamped_sample_with_key(t).unwrap();

        if let Interpolation::Bezier(_) = key.interpolation {
        } else {
          // this is needed to “see” the actual line being held
          p.x = t;
        }

        vertices.push(LineVertex::new(
          VPos::new(p.into()),
          VColor::new([0.5, 0.5, 1.]),
        ));
        indices.push(index);

        index += 1;
        t += DELTA_T;
      }

      // add the last key
      if let Some(key) = self.spline.keys().last() {
        vertices.push(LineVertex::new(
          VPos::new(key.value.into()),
          VColor::new([0.5, 0.5, 1.]),
        ));
        index += 1;
      }

      // iterate over all Bézier keys to generate their handles
      for key in keys {
        match key.interpolation {
          Interpolation::Bezier(u) => {
            let v = 2. * key.value - u;

            vertices.push(LineVertex::new(
              VPos::new(u.into()),
              VColor::new([0.4, 0.4, 0.4]),
            ));
            vertices.push(LineVertex::new(
              VPos::new(v.into()),
              VColor::new([0.4, 0.4, 0.4]),
            ));

            indices.push(PRIM_RESTART_INDEX);
            indices.push(index);
            indices.push(index + 1);

            index += 2;
          }

          Interpolation::StrokeBezier(input, output) => {
            vertices.push(LineVertex::new(
              VPos::new(input.into()),
              VColor::new([0.4, 0.4, 0.4]),
            ));
            vertices.push(LineVertex::new(
              VPos::new(key.value.into()),
              VColor::new([0.4, 0.4, 0.4]),
            ));
            vertices.push(LineVertex::new(
              VPos::new(output.into()),
              VColor::new([0.4, 0.4, 0.4]),
            ));

            indices.push(PRIM_RESTART_INDEX);
            indices.push(index);
            indices.push(index + 1);
            indices.push(index + 2);

            index += 3;
          }

          _ => (),
        }
      }
    }

    self.lines = TessBuilder::new(ctx)
      .set_mode(Mode::LineStrip)
      .set_vertices(vertices)
      .set_indices(indices)
      .set_primitive_restart_index(u32::max_value())
      .build()
      .map_err(EditorError::TessError)?;

    Ok(())
  }

  /// Rebuild tessellation based on control points for points.
  fn build_points<C>(&mut self, ctx: &mut C) -> Result<(), EditorError>
  where
    C: GraphicsContext<Backend = Backend>,
  {
    let mut vertices = Vec::new();
    let keys = self.spline.keys();

    let mut specials = Vec::new();

    if !keys.is_empty() {
      for (i, cp) in keys.iter().enumerate() {
        let mut vertex = PointVertex::new(
          VPos::new(cp.value.into()),
          VColor::new([0.5, 0.5, 1.]),
          VRadius::new(0.025 / 2.),
        );

        if let Some(Selection::Key(i_sel)) = self.selection {
          if i_sel == i {
            vertex.1 = VColor::new([1., 0.5, 0.5]);
            vertex.2 = VRadius::new(0.025 / 2.);
          }
        }

        vertices.push(vertex);

        match cp.interpolation {
          Interpolation::Bezier(mut u) => {
            for _ in 0..2 {
              let mut vertex = PointVertex::new(
                VPos::new(u.into()),
                VColor::new([0.5, 1., 0.5]),
                VRadius::new(0.015 / 2.),
              );

              if let Some(Selection::Handle(i_sel, _)) = self.selection {
                if i_sel == i {
                  vertex.1 = VColor::new([1., 0.5, 0.5]);
                  vertex.2 = VRadius::new(0.015 / 2.);
                }
              }

              specials.push(vertex);

              u = 2. * cp.value - u;
            }
          }

          Interpolation::StrokeBezier(input, output) => {
            // input
            let mut vertex = PointVertex::new(
              VPos::new(input.into()),
              VColor::new([0.5, 1., 0.5]),
              VRadius::new(0.015 / 2.),
            );

            if let Some(Selection::Handle(i_sel, HandleSelection::Own)) = self.selection {
              if i_sel == i {
                vertex.1 = VColor::new([1., 0.5, 0.5]);
                vertex.2 = VRadius::new(0.015 / 2.);
              }
            }

            specials.push(vertex);

            // input
            let mut vertex = PointVertex::new(
              VPos::new(output.into()),
              VColor::new([0.5, 1., 0.5]),
              VRadius::new(0.015 / 2.),
            );

            if let Some(Selection::Handle(i_sel, HandleSelection::Mirror)) = self.selection {
              if i_sel == i {
                vertex.1 = VColor::new([1., 0.5, 0.5]);
                vertex.2 = VRadius::new(0.015 / 2.);
              }
            }

            specials.push(vertex);
          }

          _ => (),
        }
      }
    }

    vertices.extend(specials);

    self.points = TessBuilder::new(ctx)
      .set_mode(Mode::Point)
      .set_vertices(vertices)
      .build()
      .map_err(EditorError::TessError)?;

    Ok(())
  }

  /// Rebuild tessellation if needed.
  pub fn rebuild_tess_if_needed<C>(&mut self, surface: &mut C) -> Result<(), EditorError>
  where
    C: GraphicsContext<Backend = Backend>,
  {
    if self.rebuild_tess {
      self.rebuild_tess = false;
      self.build_points(surface)?;
      self.build_lines(surface)?;
    }

    Ok(())
  }

  /// Move a point.
  pub fn move_key(&mut self, index: usize, p: ScreenPos) -> Result<(), EditorError> {
    let mut key = self
      .spline
      .remove(index)
      .ok_or_else(|| EditorError::UnknownKey(index))?;

    // move along interpolation handle if any
    if let Interpolation::Bezier(ref mut h) = key.interpolation {
      *h += p - key.value;
    }

    self.spline.add(Key::new(p[0], p, key.interpolation));
    self.rebuild_tess = true;

    Ok(())
  }

  /// Move a handle of a point.
  pub fn move_handle(
    &mut self,
    index: usize,
    p: ScreenPos,
    handle_selection: HandleSelection,
  ) -> Result<(), EditorError> {
    let key = self
      .spline
      .get_mut(index)
      .ok_or_else(|| EditorError::UnknownKey(index))?;

    match *key.interpolation {
      Interpolation::Bezier(ref mut handle) => {
        match handle_selection {
          HandleSelection::Own => {
            *handle = p;
          }

          HandleSelection::Mirror => {
            // recompute the position by symetrically rotate it
            *handle = 2. * *key.value - p;
          }
        }

        self.rebuild_tess = true;
        Ok(())
      }

      Interpolation::StrokeBezier(ref mut input, ref mut output) => {
        match handle_selection {
          HandleSelection::Own => {
            *input = p;
          }

          HandleSelection::Mirror => {
            *output = p;
          }
        }

        self.rebuild_tess = true;
        Ok(())
      }

      _ => Err(EditorError::WrongInterpolationAssumed(index)),
    }
  }

  /// Add a new point.
  pub fn add_point(&mut self, p: ScreenPos, interpolation: Interpolation<f32, ScreenPos>) {
    self.selection = None;
    self.spline.add(Key::new(p[0], p, interpolation));
    self.rebuild_tess = true;
  }

  /// Remove a point.
  pub fn remove_point(&mut self, index: usize) -> Result<Key<f32, ScreenPos>, EditorError> {
    let r = self
      .spline
      .remove(index)
      .ok_or_else(|| EditorError::UnknownKey(index))?;
    self.rebuild_tess = true;
    self.selection = None;

    Ok(r)
  }

  /// Check if there’s a selection.
  pub fn is_selecting(&self) -> bool {
    self.selection.is_some()
  }

  /// Get the currently selected point, if any.
  pub fn selected_point(&self) -> Option<usize> {
    self.selection.and_then(|s| {
      if let Selection::Key(i) = s {
        Some(i)
      } else {
        None
      }
    })
  }

  /// Currently selected content.
  pub fn selection(&self) -> &Option<Selection> {
    &self.selection
  }

  /// Try to select some content at the given position. The selected content is returned if any.
  pub fn select(&mut self, cursor_pos: ScreenPos) -> Option<Selection> {
    let [x, y]: [f32; 2] = cursor_pos.into();
    let mut found = None;

    // we want to select a point; check if any is nearby
    for (i, p) in self.spline.keys().iter().enumerate() {
      let [px, py]: [f32; 2] = p.value.into();
      let dist = ((x - px).powf(2.) + (y - py).powf(2.)).sqrt();

      if dist <= POINT_SELECTION_DIST {
        // try to select a key first
        match found {
          Some((_, prev_dist)) if dist < prev_dist => {
            found = Some((Selection::Key(i), dist));
          }

          None => {
            found = Some((Selection::Key(i), dist));
          }

          _ => (),
        }
      } else {
        match p.interpolation {
          Interpolation::Bezier(handle) => {
            // try to select a handle
            let [px, py]: [f32; 2] = handle.into();
            let dist = ((x - px).powf(2.) + (y - py).powf(2.)).sqrt();

            if dist <= POINT_SELECTION_DIST {
              match found {
                Some((_, prev_dist)) if dist < prev_dist => {
                  found = Some((Selection::Handle(i, HandleSelection::Own), dist));
                }

                None => {
                  found = Some((Selection::Handle(i, HandleSelection::Own), dist));
                }

                _ => (),
              }
            }

            let [px, py]: [f32; 2] = (2. * p.value - handle).into();
            let dist = ((x - px).powf(2.) + (y - py).powf(2.)).sqrt();

            if dist <= POINT_SELECTION_DIST {
              match found {
                Some((_, prev_dist)) if dist < prev_dist => {
                  found = Some((Selection::Handle(i, HandleSelection::Mirror), dist));
                }

                None => {
                  found = Some((Selection::Handle(i, HandleSelection::Mirror), dist));
                }

                _ => (),
              }
            }
          }

          Interpolation::StrokeBezier(input, output) => {
            // try to select the input handle
            let [px, py]: [f32; 2] = input.into();
            let dist = ((x - px).powf(2.) + (y - py).powf(2.)).sqrt();

            if dist <= POINT_SELECTION_DIST {
              match found {
                Some((_, prev_dist)) if dist < prev_dist => {
                  found = Some((Selection::Handle(i, HandleSelection::Own), dist));
                }

                None => {
                  found = Some((Selection::Handle(i, HandleSelection::Own), dist));
                }

                _ => (),
              }
            }

            let [px, py]: [f32; 2] = output.into();
            let dist = ((x - px).powf(2.) + (y - py).powf(2.)).sqrt();

            if dist <= POINT_SELECTION_DIST {
              match found {
                Some((_, prev_dist)) if dist < prev_dist => {
                  found = Some((Selection::Handle(i, HandleSelection::Mirror), dist));
                }

                None => {
                  found = Some((Selection::Handle(i, HandleSelection::Mirror), dist));
                }

                _ => (),
              }
            }
          }

          _ => (),
        }
      }
    }

    self.selection = found.map(|(selection, _)| {
      match selection {
        Selection::Key(i) => {
          println!("selecting point {}", i);
        }

        Selection::Handle(i, _) => {
          println!("selecting handle {}", i);
        }
      }

      selection
    });

    self.rebuild_tess = true;

    self.selection
  }

  /// Deselect, if anything was selected.
  pub fn deselect(&mut self) {
    self.selection = None;
    self.rebuild_tess = true;
  }

  /// Toggle the interpolation of a key to something else.
  pub fn toggle_interpolation(&mut self, index: usize) -> Result<(), EditorError> {
    let key = self
      .spline
      .get_mut(index)
      .ok_or_else(|| EditorError::UnknownKey(index))?;
    let prev = *key.interpolation;
    *key.interpolation = Self::cycle_interpolation(*key.value, prev);

    self.rebuild_tess = true;

    println!(
      "toggling interpolation for key {}; {:?} -> {:?}",
      index, prev, key.interpolation
    );
    Ok(())
  }

  /// Switch interpolation back and forth between modes.
  fn cycle_interpolation(
    p: ScreenPos,
    i: Interpolation<f32, ScreenPos>,
  ) -> Interpolation<f32, ScreenPos> {
    match i {
      Interpolation::Step(_) => Interpolation::Linear,
      Interpolation::Linear => Interpolation::Cosine,
      Interpolation::Cosine => Interpolation::Bezier(p + ScreenPos::new(0.1, 0.1)),
      Interpolation::Bezier(_) => {
        Interpolation::StrokeBezier(p - ScreenPos::new(0.1, 0.1), p + ScreenPos::new(0.1, 0.1))
      }
      Interpolation::StrokeBezier(..) => Interpolation::Step(0.5),
      _ => i,
    }
  }

  /// Get the underlying point tessellation.
  pub fn points(&self) -> &Tess<PointVertex, ()> {
    &self.points
  }

  /// Get the underlying line tessellation.
  pub fn lines(&self) -> &Tess<LineVertex, u32> {
    &self.lines
  }
}

/// Possible errors that might occur while using the editor.
#[derive(Debug)]
pub enum EditorError {
  /// Unknown key index (i.e. likely out of bounds).
  UnknownKey(usize),
  /// Error while rebuilding tessellation.
  TessError(TessError),
  /// Wrong interpolation assumed (typical for Bézier).
  WrongInterpolationAssumed(usize),
}

/// A selection. It can either be a control point (Key) or a handle for a Bézier curve. In case
/// of a handle, we either select the “real” handle or its mirrored sibling.
#[derive(Clone, Copy, Debug)]
pub enum Selection {
  /// A selected control point.
  Key(usize),
  /// A selected handle.
  Handle(usize, HandleSelection),
}

/// Part of handle being selected.
#[derive(Clone, Copy, Debug)]
pub enum HandleSelection {
  /// The actual handle of the control point.
  Own,
  /// Mirror handle of the control point.
  Mirror,
}
