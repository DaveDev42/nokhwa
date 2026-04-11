use nokhwa::utils::{CameraIndex, ControlValueSetter, KnownCameraControl};
use nokhwa::Camera;

fn main() {
    let mut camera = Camera::new_with_highest_resolution(CameraIndex::Index(0)).unwrap();
    camera.open_stream().unwrap();
    let controls = camera.camera_controls_by_id().unwrap();
    if let Some(control) = controls.get(&KnownCameraControl::Gamma) {
        println!("Current gamma: {control:?}");
        camera
            .set_camera_control(
                KnownCameraControl::Gamma,
                ControlValueSetter::Integer(101),
            )
            .unwrap();
    }
}
