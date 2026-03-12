// src/config.rs
//
// Opaque config builder wrapping timsrust's SpectrumReaderConfig.
// Used by tims_open_with_config() to customize reader construction.

#[cfg(feature = "with_timsrust")]
use timsrust::readers::SpectrumReaderConfig;

/// FFI-facing config wrapper. When with_timsrust is enabled, wraps the
/// real SpectrumReaderConfig. Otherwise a dummy struct for API compat.
pub struct TimsFfiConfig {
    #[cfg(feature = "with_timsrust")]
    pub(crate) inner: SpectrumReaderConfig,

    #[cfg(not(feature = "with_timsrust"))]
    _dummy: (),
}

impl TimsFfiConfig {
    pub fn new() -> Self {
        TimsFfiConfig {
            #[cfg(feature = "with_timsrust")]
            inner: SpectrumReaderConfig::default(),

            #[cfg(not(feature = "with_timsrust"))]
            _dummy: (),
        }
    }

    #[cfg(feature = "with_timsrust")]
    pub fn set_smoothing_window(&mut self, window: u32) {
        self.inner.spectrum_processing_params.smoothing_window = window;
    }

    #[cfg(not(feature = "with_timsrust"))]
    pub fn set_smoothing_window(&mut self, _window: u32) {}

    #[cfg(feature = "with_timsrust")]
    pub fn set_centroiding_window(&mut self, window: u32) {
        self.inner.spectrum_processing_params.centroiding_window = window;
    }

    #[cfg(not(feature = "with_timsrust"))]
    pub fn set_centroiding_window(&mut self, _window: u32) {}

    #[cfg(feature = "with_timsrust")]
    pub fn set_calibration_tolerance(&mut self, tolerance: f64) {
        self.inner.spectrum_processing_params.calibration_tolerance = tolerance;
    }

    #[cfg(not(feature = "with_timsrust"))]
    pub fn set_calibration_tolerance(&mut self, _tolerance: f64) {}

    #[cfg(feature = "with_timsrust")]
    pub fn set_calibrate(&mut self, enabled: bool) {
        self.inner.spectrum_processing_params.calibrate = enabled;
    }

    #[cfg(not(feature = "with_timsrust"))]
    pub fn set_calibrate(&mut self, _enabled: bool) {}
}
