use crate::ecs::{FrameClock, InputSnapshot, RenderFrame, DEFAULT_INPUT_DT_SECONDS};

pub trait InputPort {
    fn snapshot(&mut self, frame_clock: &mut FrameClock) -> InputSnapshot;
}

pub trait OutputPort {
    fn project(&mut self, frame: RenderFrame);
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct RenderFrameBuffer {
    last: Option<RenderFrame>,
}

impl RenderFrameBuffer {
    pub fn last(&self) -> Option<&RenderFrame> {
        self.last.as_ref()
    }
}

impl OutputPort for RenderFrameBuffer {
    fn project(&mut self, frame: RenderFrame) {
        self.last = Some(frame);
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct NoopInputPort;

impl InputPort for NoopInputPort {
    fn snapshot(&mut self, frame_clock: &mut FrameClock) -> InputSnapshot {
        InputSnapshot {
            frame: frame_clock.next_frame(),
            dt_seconds: DEFAULT_INPUT_DT_SECONDS,
            inputs: Vec::new(),
        }
    }
}
