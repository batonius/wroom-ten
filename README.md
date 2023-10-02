## wroom ten

Reimplementation of [the ray-ten's ray tracing approach](https://github.com/batonius/ray-ten) on GPU using [wgpu](https://github.com/gfx-rs/wgpu).
Uses compute shaders for the bouncing and fragment shaders for ray tracing.
GTX 4080 struggles at 45fps with 1000 balls at 2K resolution.