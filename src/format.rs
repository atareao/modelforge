//! Utilidades de formato para tamaños y progreso de descarga.

/// Formatea un tamaño en bytes con unidad binaria (p. ej. `1.5 GiB`).
#[must_use]
pub fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];

    if bytes < 1024 {
        return format!("{bytes} B");
    }

    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    format!("{value:.1} {}", UNITS[unit])
}

/// Calcula el porcentaje completado (`0..=100`), redondeado.
#[must_use]
pub fn percent(done: u64, total: u64) -> u8 {
    if total == 0 {
        return 0;
    }
    let ratio = done.min(total) as f64 / total as f64;
    (ratio * 100.0).round() as u8
}

/// Línea legible de progreso: `"3.0 MiB / 6.0 MiB (50%)"`.
#[must_use]
pub fn progress_line(done: u64, total: u64) -> String {
    format!(
        "{} / {} ({}%)",
        format_bytes(done),
        format_bytes(total),
        percent(done, total)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_bytes_with_binary_units() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1024), "1.0 KiB");
        assert_eq!(format_bytes(1536), "1.5 KiB");
        assert_eq!(format_bytes(12 * 1024 * 1024 * 1024), "12.0 GiB");
    }

    #[test]
    fn percent_handles_bounds_and_zero_total() {
        assert_eq!(percent(0, 100), 0);
        assert_eq!(percent(50, 100), 50);
        assert_eq!(percent(1, 3), 33);
        assert_eq!(percent(200, 100), 100);
        assert_eq!(percent(5, 0), 0);
    }

    #[test]
    fn progress_line_combines_size_and_percent() {
        assert_eq!(
            progress_line(3 * 1024 * 1024, 6 * 1024 * 1024),
            "3.0 MiB / 6.0 MiB (50%)"
        );
    }
}
