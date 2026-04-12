use nokhwa::utils::{CameraIndex, ControlValueSetter, KnownCameraControl};
use nokhwa::error::NokhwaError;
use nokhwa::{CameraSession, OpenRequest, OpenedCamera};

fn main() -> Result<(), NokhwaError> {
    let opened = CameraSession::open(CameraIndex::Index(0), OpenRequest::any())?;
    let OpenedCamera::Stream(mut camera) = opened else {
        return Err(NokhwaError::general("expected stream-capable camera"));
    };
    camera.open()?;
    let controls = camera.controls()?;
    if let Some(control) = controls
        .iter()
        .find(|c| c.control() == KnownCameraControl::Gamma)
    {
        println!("Current gamma: {control:?}");
        camera.set_control(KnownCameraControl::Gamma, ControlValueSetter::Integer(101))?;
    }
    Ok(())
}
