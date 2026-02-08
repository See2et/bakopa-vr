use std::collections::HashMap;

use godot::classes::Node3D;
use godot::prelude::*;
use tracing::debug;

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
pub struct RenderStateProjector {
    remote_projection_cache: HashMap<String, Transform3D>,
}

#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
pub enum ProjectionError {
    #[error("target node is invalid")]
    InvalidTargetNode,
}

impl RenderStateProjector {
    fn refresh_remote_projection_cache(&mut self, frame: &RenderFrame) {
        self.remote_projection_cache = frame
            .remote_poses()
            .iter()
            .map(|remote| {
                (
                    remote.participant_id.clone(),
                    pose_to_transform3d(&remote.pose),
                )
            })
            .collect();
    }

    pub fn project(
        &mut self,
        frame: &RenderFrame,
        target: &mut OnEditor<Gd<Node3D>>,
    ) -> Result<(), ProjectionError> {
        self.refresh_remote_projection_cache(frame);
        if target.is_invalid() {
            debug!(
                target: "godot_adapter",
                frame_id = ?frame.frame,
                target_type = "Node3D",
                target_invalid = true,
                "projection skipped because target node is invalid"
            );
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

    pub(crate) fn remote_cache_after_project(frame: &RenderFrame) -> Vec<(String, Transform3D)> {
        let mut projector = RenderStateProjector::default();
        let mut target = OnEditor::<Gd<Node3D>>::default();
        let _ = projector.project(frame, &mut target);
        projector
            .remote_projection_cache
            .iter()
            .map(|(participant_id, transform)| (participant_id.clone(), *transform))
            .collect()
    }
}
