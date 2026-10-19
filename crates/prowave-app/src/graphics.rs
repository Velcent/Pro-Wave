//! Which graphics API draws the window. By default eframe starts every API it knows (Vulkan,
//! DirectX 12 and OpenGL) and takes the fastest GPU, which costs about 100 MB of memory more
//! than needed for a 2D interface like Pro Wave's. Here:
//!
//! - macOS uses Metal, its only choice;
//! - Windows and Linux use OpenGL when this machine has a working OpenGL driver (checked before
//!   the window opens), and otherwise the system's own API (DirectX 12 / Vulkan), so a broken
//!   OpenGL driver still gets a window. Starting both would cost the memory saved;
//! - a real GPU is preferred over a software renderer.
//!
//! `WGPU_BACKEND` (`gl`, `dx12`, `vulkan`, `metal`) overrides the choice, for troubleshooting.
use eframe::egui_wgpu::{WgpuConfiguration, WgpuSetup, WgpuSetupCreateNew};
use eframe::wgpu;
use std::sync::Arc;

/// The system's own graphics API: the fallback when OpenGL does not work.
fn native() -> wgpu::Backends {
    if cfg!(target_os = "macos") {
        wgpu::Backends::METAL
    } else if cfg!(windows) {
        wgpu::Backends::DX12
    } else {
        wgpu::Backends::VULKAN
    }
}

/// The graphics API used on this machine: Metal on macOS; elsewhere OpenGL when `gl_works`,
/// else the system's own API, with software OpenGL kept as the last resort (a virtual machine
/// without a GPU), which [`pick`] only takes when nothing else can draw.
pub fn backends(gl_works: bool) -> wgpu::Backends {
    if cfg!(target_os = "macos") {
        native()
    } else if gl_works {
        wgpu::Backends::GL
    } else {
        native() | wgpu::Backends::GL
    }
}

fn instance_descriptor(backends: wgpu::Backends) -> wgpu::InstanceDescriptor {
    wgpu::InstanceDescriptor {
        backends,
        flags: wgpu::InstanceFlags::from_build_config().with_env(),
        backend_options: wgpu::BackendOptions::from_env_or_default(),
        memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
        display: None,
    }
}

/// Whether an OpenGL adapter that is a real GPU can be created on this machine.
fn opengl_works() -> bool {
    let instance = wgpu::Instance::new(instance_descriptor(wgpu::Backends::GL));
    let adapters = block_on(instance.enumerate_adapters(wgpu::Backends::GL));
    let found = adapters
        .iter()
        .any(|a| a.get_info().device_type != wgpu::DeviceType::Cpu);
    if !found {
        tracing::info!("no hardware OpenGL driver; using the system's own graphics API");
    }
    found
}

/// Runs a future that completes without waiting, as wgpu's adapter listing does on desktops.
fn block_on<F: std::future::Future>(future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
    loop {
        if let std::task::Poll::Ready(out) = future.as_mut().poll(&mut cx) {
            return out;
        }
        std::thread::yield_now();
    }
}

/// Sort key for an adapter: lower is better. A real GPU before a software renderer, then
/// OpenGL before the other API.
fn rank(backend: wgpu::Backend, device: wgpu::DeviceType) -> (bool, bool) {
    (
        device == wgpu::DeviceType::Cpu,
        backend != wgpu::Backend::Gl,
    )
}

fn pick(
    adapters: &[wgpu::Adapter],
    surface: Option<&wgpu::Surface<'_>>,
) -> Result<wgpu::Adapter, String> {
    adapters
        .iter()
        .filter(|a| surface.is_none_or(|s| a.is_surface_supported(s)))
        .min_by_key(|a| {
            let info = a.get_info();
            rank(info.backend, info.device_type)
        })
        .cloned()
        .ok_or_else(|| "no graphics adapter can draw to this window".to_string())
}

/// eframe's wgpu settings for Pro Wave (see the module docs).
pub fn config() -> WgpuConfiguration {
    let mut setup = WgpuSetupCreateNew::without_display_handle();
    if wgpu::Backends::from_env().is_none() {
        let gl_works = !cfg!(target_os = "macos") && opengl_works();
        setup.instance_descriptor.backends = backends(gl_works);
        setup.native_adapter_selector = Some(Arc::new(pick));
    }
    WgpuConfiguration {
        wgpu_setup: WgpuSetup::CreateNew(setup),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wgpu::{Backend, DeviceType};

    #[test]
    fn opengl_on_a_real_gpu_is_preferred() {
        let mut options = [
            (Backend::Dx12, DeviceType::DiscreteGpu),
            (Backend::Gl, DeviceType::Cpu),
            (Backend::Gl, DeviceType::IntegratedGpu),
            (Backend::Vulkan, DeviceType::Cpu),
        ];
        options.sort_by_key(|(b, d)| rank(*b, *d));
        assert_eq!(options[0], (Backend::Gl, DeviceType::IntegratedGpu));
        assert_eq!(
            options[1],
            (Backend::Dx12, DeviceType::DiscreteGpu),
            "then the GPU"
        );
        assert_eq!(options[2].1, DeviceType::Cpu, "software rendering last");
    }

    #[test]
    fn each_system_starts_one_api_and_falls_back_without_opengl() {
        if cfg!(target_os = "macos") {
            assert_eq!(backends(true), wgpu::Backends::METAL);
            assert_eq!(backends(false), wgpu::Backends::METAL);
        } else {
            assert_eq!(backends(true), wgpu::Backends::GL);
            assert_eq!(backends(false), native() | wgpu::Backends::GL);
            assert!(!native().contains(wgpu::Backends::GL));
        }
    }

    #[test]
    fn a_ready_future_is_returned_at_once() {
        assert_eq!(block_on(async { 7 }), 7);
    }
}
