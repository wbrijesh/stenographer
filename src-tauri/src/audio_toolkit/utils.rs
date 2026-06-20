/// Returns the appropriate CPAL host. macOS-only build: always the default host.
pub fn get_cpal_host() -> cpal::Host {
    cpal::default_host()
}
