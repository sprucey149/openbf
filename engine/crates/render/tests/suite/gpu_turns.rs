//! Renderers made on parallel threads, as the test harness makes them. Without
//! their turn at the GPU (`render::device::GpuTurn`) this crashes inside the
//! Vulkan loader, taking the whole test binary with it, on most runs.

#[test]
#[ignore = "requires a GPU adapter (run with --ignored on the NVIDIA box)"]
fn renderers_on_parallel_threads_do_not_crash_the_loader() {
    std::thread::scope(|s| {
        for _ in 0..8 {
            s.spawn(|| {
                for _ in 0..4 {
                    let r = render::Renderer::new_headless(8, 8).expect("headless renderer");
                    drop(r);
                }
            });
        }
    });
}
