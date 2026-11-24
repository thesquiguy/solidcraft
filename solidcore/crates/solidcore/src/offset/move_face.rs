//! Not upstream (tangency-labs/solidcraft#15550): Move Face on planes. A plane face is moved
//! as a rigid body, turned about an axis and then translated, onto the plane that motion
//! carries its own plane to; the caller's local operation
//! ([`crate::topology::brep::local_operation::local_operation_simple`]) then extends or
//! trims the faces around it to meet the new plane, as a draft's does.
//!
//! A rigid motion carries a plane onto a plane exactly, so the moved surface is the face's
//! own plane transformed ([`Surface::transform`]) with no definition to re-certify: a plane is
//! its own definition. Only plane faces are moved here. A cone, cylinder or other curved face
//! is refused ([`MoveFaceRefusal::NotPlane`]), as the roadmap's first stage of direct
//! modelling (DM-1.1 of tangency-labs/solidcraft#16449) takes planes only.
//!
//! The moved plane is the one the motion carries the face's plane to, but its frame is the
//! face's own, tilted only as far as the normal turns and slid along the new normal: the part of
//! the motion that turns the plane within itself, or slides it along itself, changes no point of
//! the plane, so it is left out. The local operation is given the same plane either way, but a
//! frame spun far about the normal (0.7 radians, with a translation along it) made it return a
//! valid solid of the wrong volume; the face's own frame gives the closed form.

use crate::error::{offset::OffsetFailure, Failure};
use crate::geom::axis2_placement::Axis2Placement;
use crate::geom::certified::unit;
use crate::kernel::Kernel;
use crate::math::{Point3, Vec3};
use crate::surface::surface::{Surface, SurfaceKind};
use crate::topology::{Brep, FaceId};
use crate::{Result, SolidError};

const OP: &str = "offset::move_face";

/// A rigid motion of a face: an optional turn by `angle` radians (right-hand rule) about the
/// axis through `center` along `axis`, then a translation by `translation`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RigidMove {
    translation: Vec3,
    turn: Option<Turn>,
}

/// A turn about an axis: the axis's point, its unit direction and the angle in radians.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Turn {
    center: Point3,
    axis: Vec3,
    angle: f64,
}

/// Why a face cannot be moved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MoveFaceRefusal {
    /// The face's surface is not a plane.
    NotPlane,
    /// The turn takes the face's normal to a right angle or past it from where it was
    /// ([`TURN_LIMIT`]): the face would face across or into the material it bounded.
    TurnedPastSquare,
}

/// The least cosine of the angle between a plane face's normal before and after a move: a
/// turn to within about 1e-6 radians of square, or past it, is refused
/// ([`MoveFaceRefusal::TurnedPastSquare`]).
pub const TURN_LIMIT: f64 = 1e-6;

impl RigidMove {
    /// A translation by `translation` alone.
    ///
    /// # Errors
    ///
    /// [`SolidError::InvalidInput`] for a translation not finite.
    pub fn translation(translation: Vec3) -> Result<Self> {
        if !finite(translation) {
            return Err(SolidError::invalid_input(OP, "a translation not finite"));
        }
        Ok(Self {
            translation,
            turn: None,
        })
    }

    /// A turn by `angle` radians (right-hand rule about `axis`) about the axis through
    /// `center` along `axis`, then a translation by `translation`; the axis is made unit.
    ///
    /// # Errors
    ///
    /// [`SolidError::InvalidInput`] for an axis not finite or not longer than 1e-12, or a
    /// center, angle or translation not finite.
    pub fn turn_then_translate(
        center: Point3,
        axis: Vec3,
        angle: f64,
        translation: Vec3,
    ) -> Result<Self> {
        let Some(axis) = unit(axis) else {
            return Err(SolidError::invalid_input(
                OP,
                "an axis not finite or not longer than 1e-12",
            ));
        };
        if !finite(center) || !angle.is_finite() || !finite(translation) {
            return Err(SolidError::invalid_input(
                OP,
                "a center, angle or translation not finite",
            ));
        }
        Ok(Self {
            translation,
            turn: Some(Turn {
                center,
                axis,
                angle,
            }),
        })
    }

    /// The translation, applied after the turn.
    #[must_use]
    pub const fn translation_part(&self) -> Vec3 {
        self.translation
    }

    /// The turn's axis point, unit axis and angle in radians; `None` for a translation alone.
    #[must_use]
    pub fn turn_part(&self) -> Option<(Point3, Vec3, f64)> {
        self.turn.map(|t| (t.center, t.axis, t.angle))
    }

    /// The motion as a placement ([`Axis2Placement`]): the identity turned about the axis,
    /// then moved by the translation, each post-concatenated.
    #[must_use]
    pub fn placement(&self) -> Axis2Placement {
        let mut p = Axis2Placement::IDENTITY;
        if let Some(t) = self.turn {
            p.rotate_about_axis_at_point(t.angle, t.center, t.axis);
        }
        p.translate(self.translation);
        p
    }
}

/// Whether every component of `v` is finite.
fn finite(v: Vec3) -> bool {
    v.x.is_finite() && v.y.is_finite() && v.z.is_finite()
}

/// The surface plane face `f` of `b` takes when moved by `motion`: its own plane carried by
/// the motion, with no definition (a rigid image of a plane is a plane, exactly). The face's
/// outward sense rides on its face use and is unchanged by moving the surface.
///
/// # Errors
///
/// `SM_ERR_INVALID_INPUT` for a face without a surface; those of the surface's kind, copy and
/// transform. A face that cannot be moved is an `Ok(Err(refusal))`.
pub fn moved_surface(
    k: &Kernel,
    b: &Brep,
    f: FaceId,
    motion: &RigidMove,
) -> Result<std::result::Result<Surface, MoveFaceRefusal>> {
    let Some(surf) = b.face(f)?.surface.as_ref() else {
        return Err(
            Failure::new(OP, OffsetFailure::FaceHasNoSurface { face: f })
                .with_status(crate::error::SM_ERR_INVALID_INPUT)
                .into_error(),
        );
    };
    let SurfaceKind::Plane(plane) = surf.kind_with_nurb(k)? else {
        return Ok(Err(MoveFaceRefusal::NotPlane));
    };
    let (origin, normal) = (plane.position().origin(), plane.position().z_axis());
    let carried = motion.placement();
    let (to, turned) = (
        carried.transform_point(origin),
        carried.transform_vector(normal),
    );
    let cos = normal.dot(turned);
    if cos.is_nan() || cos <= TURN_LIMIT {
        return Ok(Err(MoveFaceRefusal::TurnedPastSquare));
    }
    // The face's own frame, tilted about `normal × turned` through its origin by the angle
    // between the normals, then slid along the new normal onto the carried origin's plane.
    let mut frame = Axis2Placement::IDENTITY;
    let tilt = normal.cross(turned);
    let sin = tilt.length();
    if sin > 0.0 {
        frame.rotate_about_axis_at_point(sin.atan2(cos), origin, tilt);
    }
    let slide = (to - origin).dot(turned);
    frame.translate(turned * slide);
    let mut moved = surf.copy(k)?;
    moved.clear_definition();
    moved.transform(k, &frame, None)?;
    Ok(Ok(moved))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hostile_motions_are_refused() {
        let nan = Vec3::new(f64::NAN, 0.0, 0.0);
        let inf = Vec3::new(0.0, f64::INFINITY, 0.0);
        assert!(RigidMove::translation(nan).is_err());
        assert!(RigidMove::translation(inf).is_err());
        assert!(RigidMove::turn_then_translate(Vec3::ZERO, Vec3::ZERO, 0.1, Vec3::ZERO).is_err());
        assert!(RigidMove::turn_then_translate(Vec3::ZERO, nan, 0.1, Vec3::ZERO).is_err());
        assert!(RigidMove::turn_then_translate(nan, Vec3::Z, 0.1, Vec3::ZERO).is_err());
        assert!(RigidMove::turn_then_translate(Vec3::ZERO, Vec3::Z, f64::NAN, Vec3::ZERO).is_err());
        assert!(RigidMove::turn_then_translate(Vec3::ZERO, Vec3::Z, 0.1, inf).is_err());
    }

    /// A translation alone is the identity moved: the placement's axes stay the world's.
    #[test]
    fn a_translation_moves_the_origin_only() {
        let m = RigidMove::translation(Vec3::new(1.0, 2.0, 3.0)).unwrap();
        let p = m.placement();
        assert_eq!(p.origin(), Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(p.x_axis(), Vec3::X);
        assert_eq!(p.y_axis(), Vec3::Y);
        assert_eq!(m.turn_part(), None);
    }

    /// A half turn about the z axis through (1, 0, 0) carries the origin to (2, 0, 0), and the
    /// translation then adds to it; the axis given long is made unit.
    #[test]
    fn a_turn_then_a_translation() {
        let m = RigidMove::turn_then_translate(
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 4.0),
            std::f64::consts::PI,
            Vec3::new(0.0, 0.0, 5.0),
        )
        .unwrap();
        let o = m.placement().origin();
        assert!((o.x - 2.0).abs() < 1e-12, "{o:?}");
        assert!(o.y.abs() < 1e-12, "{o:?}");
        assert!((o.z - 5.0).abs() < 1e-12, "{o:?}");
        assert_eq!(m.turn_part().map(|t| t.1), Some(Vec3::Z));
    }
}
