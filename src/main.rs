use std::path::PathBuf;

use clap::Parser;
use image::{ImageFormat, ImageReader};
use miette::miette;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

#[derive(Parser, Debug, Default)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path of the file to convert
    path: String,

    /// Format to convert to
    #[arg(short, long)]
    target_format: Option<String>,

    /// Log level for logging to the console
    #[arg(short, long)]
    log_level: Option<String>,
}

fn main() -> miette::Result<()> {
    let args = Args::parse();
    run(args)?;
    Ok(())
}

fn run(args: Args) -> miette::Result<()> {
    let log_level = if let Some(log_level) = args.log_level {
        Some(string_to_log_level(&log_level)?)
    } else {
        None
    };
    if let Some(log_level) = log_level {
        let subscriber = FmtSubscriber::builder().with_max_level(log_level).finish();
        tracing::subscriber::set_global_default(subscriber)
            .map_err(|_| miette!("Failed setting the tracing subscriber"))?;
    }

    let path = PathBuf::from(args.path);
    if !path.exists() {
        return Err(miette!("Failed to find the file: {}", path.display()));
    }
    tracing::debug!("Path exists: {}", path.display());

    let reader =
        ImageReader::open(&path).map_err(|_| miette!("Failed to open file: {}", path.display()))?;
    tracing::trace!("Opened file: {}", path.display());

    let format = reader.format().expect("format must be known");
    tracing::debug!("Format of the input file: {:?}", format);

    let img = reader
        .decode()
        .map_err(|_| miette!("Failed to decode file: {}", path.display()))?;
    tracing::trace!("Decoded file: {}", path.display());

    let target_format = if let Some(target_format) = &args.target_format {
        let target_format = target_format.as_str().to_lowercase().trim().to_string();
        let target_format = string_to_format(&target_format)?;
        tracing::debug!("Target format: {:?}", target_format);
        Some(target_format)
    } else {
        tracing::trace!("No target format");
        None
    };

    if let Some(target_format) = target_format {
        let target_path = path.with_extension(format_to_string(target_format));
        tracing::debug!("Saving file: {}", target_path.display());

        img.save(&target_path).map_err(|_| {
            let fmt = format_to_string(target_format);
            miette!("Failed to save file with format: {fmt}")
        })?;
        tracing::trace!("Saved file: {}", target_path.display());
    }

    Ok(())
}

fn string_to_log_level(level: &str) -> miette::Result<Level> {
    Ok(match level {
        "trace" => Level::TRACE,
        "debug" => Level::DEBUG,
        "info" => Level::INFO,
        "warn" => Level::WARN,
        "error" => Level::ERROR,
        _ => return Err(miette!("Failed to determine log level")),
    })
}

fn string_to_format(format: &str) -> miette::Result<ImageFormat> {
    Ok(match format {
        "png" => ImageFormat::Png,
        "jpg" | "jpeg" => ImageFormat::Jpeg,
        "gif" => ImageFormat::Gif,
        "webp" => ImageFormat::WebP,
        "pnm" => ImageFormat::Pnm,
        "tiff" => ImageFormat::Tiff,
        "tga" => ImageFormat::Tga,
        "dds" => ImageFormat::Dds,
        "bmp" => ImageFormat::Bmp,
        "ico" => ImageFormat::Ico,
        "hdr" => ImageFormat::Hdr,
        "openexr" => ImageFormat::OpenExr,
        "farbfeld" => ImageFormat::Farbfeld,
        "avif" => ImageFormat::Avif,
        "qoi" => ImageFormat::Qoi,
        "pcx" => ImageFormat::Pcx,
        _ => return Err(miette!("Unknown format: {format}")),
    })
}

fn format_to_string(format: ImageFormat) -> String {
    match format {
        ImageFormat::Png => "png",
        ImageFormat::Jpeg => "jpeg",
        ImageFormat::Gif => "gif",
        ImageFormat::WebP => "webp",
        ImageFormat::Pnm => "pnm",
        ImageFormat::Tiff => "tiff",
        ImageFormat::Tga => "tga",
        ImageFormat::Dds => "dds",
        ImageFormat::Bmp => "bmp",
        ImageFormat::Ico => "ico",
        ImageFormat::Hdr => "hdr",
        ImageFormat::OpenExr => "openexr",
        ImageFormat::Farbfeld => "farbfeld",
        ImageFormat::Avif => "avif",
        ImageFormat::Qoi => "qoi",
        ImageFormat::Pcx => "pcx",
        _ => todo!(),
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use image::RgbImage;

    use super::*;

    #[test]
    fn jpg_to_png() {
        let dir = tempdir::TempDir::new("").unwrap();

        const SIZE: u32 = 32;

        let img = RgbImage::new(SIZE, SIZE);
        let input_path = dir.path().join("my_image.jpg");
        dbg!(&input_path);
        img.save_with_format(&input_path, ImageFormat::Jpeg)
            .unwrap();

        let args = Args {
            path: input_path.display().to_string(),
            target_format: Some("png".to_owned()),
            log_level: None,
        };

        run(args).unwrap();

        let output_path = input_path.with_extension("png");
        let reader = ImageReader::open(&output_path).unwrap();
        assert_eq!(reader.format(), Some(ImageFormat::Png));
        assert_eq!(reader.into_dimensions().unwrap(), (SIZE, SIZE));

        let reader = ImageReader::open(&output_path).unwrap();
        let result = reader.decode();
        assert!(result.is_ok());
    }
}
