use godot::classes::Node3D;
use godot::prelude::*;
use std::fmt;

use client_domain::ecs::{Pose, RenderFrame};

fn pose_to_transform3d(pose: &Pose) -> Transform3D {
    let origin = Vector3::new(pose.position.x, pose.position.y, pose.position.z);
    let quat = Quaternion::new(
        pose.orientation.x,
        pose.orientation.y,
        pose.orientation.z,
        pose.orientation.w,
    );
    let basis = Basis::from_quaternion(quat);
    Transform3D::new(basis, origin)
}

fn render_frame_transform(frame: &RenderFrame) -> Transform3D {
    pose_to_transform3d(frame.primary_pose())
}

trait TransformTarget {
    fn set_transform(&mut self, transform: Transform3D);
}

impl TransformTarget for Gd<Node3D> {
    fn set_transform(&mut self, transform: Transform3D) {
        Node3D::set_transform(self, transform);
    }
}

fn project_render_frame_to_target(frame: &RenderFrame, target: &mut impl TransformTarget) {
    target.set_transform(render_frame_transform(frame));
}

#[derive(Debug, Default)]
pub struct RenderStateProjector;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectionError {
    InvalidTargetNode,
}

impl fmt::Display for ProjectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTargetNode => write!(f, "target node is invalid"),
        }
    }
}

impl RenderStateProjector {
    pub fn project(
        &mut self,
        frame: &RenderFrame,
        target: &mut OnEditor<Gd<Node3D>>,
    ) -> Result<(), ProjectionError> {
        if target.is_invalid() {
            return Err(ProjectionError::InvalidTargetNode);
        }
        let node = &mut **target;
        project_render_frame_to_target(frame, node);
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod tests_support {
    use super::*;

    pub(crate) fn transform_from_pose(pose: &Pose) -> Transform3D {
        pose_to_transform3d(pose)
    }

    pub(crate) fn transform_from_frame(frame: &RenderFrame) -> Transform3D {
        render_frame_transform(frame)
    }
}
