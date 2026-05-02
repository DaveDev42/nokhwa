# nokhwa-core
This crate contains core type definitions for `nokhwa`. It is split out
so other crates may depend on the type surface without pulling in any
platform-specific capture backend.

Inside there are standard definitions (`Resolution`, `CameraInfo`, `CameraIndex`, `CameraFormat`, etc.), and 
there are decoders for NV12, YUY2/YUYV, MJPEG, GRAY, and RGB24, with a flexible trait based system for you to add your
own decoders. 