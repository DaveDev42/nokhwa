use nokhwa::{Camera, CameraFormat, KnownCameraControl};

fn main() {
    let mut camera = Camera::new(0, None).unwrap();
    let known = camera.camera_controls_by_id().unwrap();
    let mut control = *known.get(&KnownCameraControl::Gamma).unwrap();
    control.set_value(101).unwrap();
    camera.set_camera_control(control).unwrap();
}
