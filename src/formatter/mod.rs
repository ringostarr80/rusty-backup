pub struct Formatter {}

impl Formatter {
    pub fn format_size(size: usize, precision: u8) -> String {
        let mut size_float = size as f64;
        let mut size_unit = "B";

        if size_float > 1024.0 {
            size_float /= 1024.0;
            size_unit = "KB";
        }
        if size_float > 1024.0 {
            size_float /= 1024.0;
            size_unit = "MB";
        }
        if size_float > 1024.0 {
            size_float /= 1024.0;
            size_unit = "GB";
        }
        if size_float > 1024.0 {
            size_float /= 1024.0;
            size_unit = "TB";
        }

        format!(
            "{number:.prec$} {unit}",
            number = size_float,
            prec = precision as usize,
            unit = size_unit
        )
    }
}
