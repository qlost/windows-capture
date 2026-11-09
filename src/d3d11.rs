use windows::{
    Graphics::DirectX::Direct3D11::IDirect3DDevice,
    Win32::{
        Foundation::HMODULE,
        Graphics::{
            Direct3D::{
                D3D_DRIVER_TYPE_HARDWARE, D3D_FEATURE_LEVEL, D3D_FEATURE_LEVEL_9_1,
                D3D_FEATURE_LEVEL_9_2, D3D_FEATURE_LEVEL_9_3, D3D_FEATURE_LEVEL_10_0,
                D3D_FEATURE_LEVEL_10_1, D3D_FEATURE_LEVEL_11_0, D3D_FEATURE_LEVEL_11_1,
            },
            Direct3D11::{
                D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION, D3D11CreateDevice,
                ID3D11Device, ID3D11DeviceContext,
            },
            Dxgi::IDXGIDevice,
        },
        System::LibraryLoader::{GetProcAddress, LoadLibraryW},
    },
    core::Interface,
};

#[derive(thiserror::Error, Eq, PartialEq, Clone, Debug)]
pub enum Error {
    #[error("Failed to create DirectX device with the recommended feature levels")]
    FeatureLevelNotSatisfied,
    #[error("Windows API Error: {0}")]
    WindowsError(#[from] windows::core::Error),
}

/// Used To Send DirectX Device Across Threads
pub struct SendDirectX<T>(pub T);

impl<T> SendDirectX<T> {
    /// Create A New `SendDirectX` Instance
    ///
    /// # Arguments
    ///
    /// * `device` - The DirectX Device
    ///
    /// # Returns
    ///
    /// Returns A New `SendDirectX` Instance
    #[must_use]
    #[inline]
    pub const fn new(device: T) -> Self {
        Self(device)
    }
}

#[allow(clippy::non_send_fields_in_send_ty)]
unsafe impl<T> Send for SendDirectX<T> {}

/// Create `ID3D11Device` and `ID3D11DeviceContext`
#[inline]
pub fn create_d3d_device() -> Result<(ID3D11Device, ID3D11DeviceContext), Error> {
    // Array of Direct3D feature levels.
    // The feature levels are listed in descending order of capability.
    // The highest feature level supported by the system is at index 0.
    // The lowest feature level supported by the system is at the last index.
    let feature_flags = [
        D3D_FEATURE_LEVEL_11_1,
        D3D_FEATURE_LEVEL_11_0,
        D3D_FEATURE_LEVEL_10_1,
        D3D_FEATURE_LEVEL_10_0,
        D3D_FEATURE_LEVEL_9_3,
        D3D_FEATURE_LEVEL_9_2,
        D3D_FEATURE_LEVEL_9_1,
    ];

    let mut d3d_device = None;
    let mut feature_level = D3D_FEATURE_LEVEL::default();
    let mut d3d_device_context = None;
    unsafe {
        D3D11CreateDevice(
            None,
            D3D_DRIVER_TYPE_HARDWARE,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            Some(&feature_flags),
            D3D11_SDK_VERSION,
            Some(&mut d3d_device),
            Some(&mut feature_level),
            Some(&mut d3d_device_context),
        )?;
    };

    if feature_level.0 < D3D_FEATURE_LEVEL_11_0.0 {
        return Err(Error::FeatureLevelNotSatisfied);
    }

    Ok((d3d_device.unwrap(), d3d_device_context.unwrap()))
}

/// Create `IDirect3DDevice` From `ID3D11Device`
#[inline]
pub fn create_direct3d_device(d3d_device: &ID3D11Device) -> Result<IDirect3DDevice, Error> {
    let function = (*CREATE_DIRECT3D11_DEVICE.get_or_init(load_create_direct3d11_device))
        .ok_or_else(|| {
            // ERROR_PROC_NOT_FOUND
            windows::core::Error::from(windows::core::HRESULT::from_win32(127))
        })?;
    let dxgi_device: IDXGIDevice = d3d_device.cast()?;

    let mut inspectable: *mut core::ffi::c_void = core::ptr::null_mut();
    unsafe {
        function(dxgi_device.as_raw(), &mut inspectable).ok()?;
    }
    let inspectable: windows::core::IInspectable =
        unsafe { windows::core::Type::from_abi(inspectable) }?;
    let device: IDirect3DDevice = inspectable.cast()?;

    Ok(device)
}

type CreateDirect3D11DeviceFromDXGIDeviceFn = unsafe extern "system" fn(
    *mut core::ffi::c_void,
    *mut *mut core::ffi::c_void,
) -> windows::core::HRESULT;

/// `CreateDirect3D11DeviceFromDXGIDevice` is only available on Windows 8+.
/// Load it dynamically from `d3d11.dll` to avoid a static import, so the
/// binary still loads on older systems (Windows 7). Screen capture itself is
/// Windows 10-only, so on Windows 7 this returns an error.
static CREATE_DIRECT3D11_DEVICE: std::sync::OnceLock<Option<CreateDirect3D11DeviceFromDXGIDeviceFn>> =
    std::sync::OnceLock::new();

fn load_create_direct3d11_device() -> Option<CreateDirect3D11DeviceFromDXGIDeviceFn> {
    let d3d11 = unsafe { LoadLibraryW(windows::core::w!("d3d11.dll")) }.ok()?;
    let proc = unsafe {
        GetProcAddress(d3d11, windows::core::s!("CreateDirect3D11DeviceFromDXGIDevice"))
    }?;
    Some(unsafe { std::mem::transmute(proc) })
}
