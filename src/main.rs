use std::{
    io,
    path::{Path, PathBuf},
};

use clap::{Parser, Subcommand};
use ignore::Walk;
use image::{ImageFormat, ImageReader};
use miette::miette;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

#[derive(Parser, Debug, Default)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path of the file to convert
    path: String,

    /// Log level for logging to the console
    #[arg(short, long)]
    log_level: Option<String>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Convert {
        /// Format to convert to
        #[arg(short, long)]
        target_format: String,
    },
    Is {
        /// Format to check for
        #[arg(short, long)]
        format: String,
    },
    Info,
}

struct Context<'a> {
    stdout: &'a mut dyn io::Write,
}

fn main() -> miette::Result<()> {
    let args = Args::parse();
    let context = Context {
        stdout: &mut io::stdout(),
    };
    run(context, args)?;
    Ok(())
}

fn run(mut context: Context, args: Args) -> miette::Result<()> {
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

    match args.command {
        Some(Commands::Convert { target_format }) => convert(&path, target_format)?,
        Some(Commands::Is { format }) => {
            if is(&path, &format)? {
                return Ok(());
            } else {
                return Err(miette!("Format mismatch"));
            }
        }
        Some(Commands::Info) | None => info(&mut context, &path)?,
    }

    Ok(())
}

fn info(context: &mut Context, path: &Path) -> miette::Result<()> {
    match to_path_type(path) {
        Some(PathType::File) => {
            let reader = ImageReader::open(&path)
                .map_err(|_| miette!("Failed to open file: {}", path.display()))?;
            tracing::trace!("Opened file: {}", path.display());

            let format = reader
                .format()
                .map(format_to_string)
                .unwrap_or("unknown".to_owned());
            writeln!(context.stdout, "{} {}", path.display(), format)
                .map_err(|_| miette!("Failed to write to stdout"))?;
        }
        Some(PathType::Directory) => todo!(),
        None => return Err(miette!("Failed to access path: {}", path.display())),
    }
    Ok(())
}

enum PathType {
    File,
    Directory,
}

fn to_path_type(path: &Path) -> Option<PathType> {
    if path.is_file() {
        Some(PathType::File)
    } else if path.is_dir() {
        Some(PathType::Directory)
    } else {
        tracing::warn!(
            "Probably no permissions to access the path: {}",
            path.display()
        );
        None
    }
}

fn is(path: &Path, format: &str) -> miette::Result<bool> {
    let format = dirty_string_to_format(format)?;
    match to_path_type(path) {
        Some(PathType::File) => is_image_with_type(path, format),
        Some(PathType::Directory) => todo!(),
        None => todo!(),
    }
}

fn convert(path: &Path, target_format: String) -> miette::Result<()> {
    let target_format = dirty_string_to_format(&target_format)?;
    tracing::debug!("Target format: {:?}", target_format);

    if path.is_file() {
        convert_file(&path, target_format).map_err(|_| miette!("Failed to convert the file"))?;
    } else if path.is_dir() {
        convert_directory(&path, target_format)
            .map_err(|_| miette!("Failed to convert files in directory"))?;
    } else {
        tracing::warn!(
            "Probably no permissions to access the path: {}",
            path.display()
        );
        return Err(miette!(
            "Failed to access the given path: {}",
            path.display()
        ));
    }

    Ok(())
}

fn convert_file(path: &Path, target_format: ImageFormat) -> miette::Result<()> {
    let reader =
        ImageReader::open(&path).map_err(|_| miette!("Failed to open file: {}", path.display()))?;
    tracing::trace!("Opened file: {}", path.display());

    let format = reader.format().expect("format must be known");
    tracing::debug!("Format of the input file: {:?}", format);

    let img = reader
        .decode()
        .map_err(|_| miette!("Failed to decode file: {}", path.display()))?;
    tracing::trace!("Decoded file: {}", path.display());

    let target_path = path.with_extension(format_to_string(target_format));
    tracing::debug!("Saving file: {}", target_path.display());

    img.save(&target_path).map_err(|_| {
        let fmt = format_to_string(target_format);
        miette!("Failed to save file with format: {fmt}")
    })?;
    tracing::trace!("Saved file: {}", target_path.display());

    Ok(())
}

fn convert_directory(path: &Path, target_format: ImageFormat) -> miette::Result<()> {
    for result in Walk::new(path) {
        if let Ok(entry) = result {
            if is_image_file(entry.path()).unwrap_or(false) {
                convert_file(entry.path(), target_format)?;
            }
        }
    }

    Ok(())
}

fn is_image_file(path: &Path) -> miette::Result<bool> {
    let reader =
        ImageReader::open(&path).map_err(|_| miette!("Failed to open file: {}", path.display()))?;
    Ok(reader.format().is_some())
}

fn is_image_with_type(path: &Path, format: ImageFormat) -> miette::Result<bool> {
    let reader =
        ImageReader::open(&path).map_err(|_| miette!("Failed to open file: {}", path.display()))?;
    Ok(reader.format().map_or(false, |f| f == format))
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

fn dirty_string_to_format(format: &str) -> miette::Result<ImageFormat> {
    let format = format.to_lowercase().trim().to_string();
    string_to_format(&format)
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
    use core::str;
    use std::fs;

    use image::RgbImage;
    use tempdir::TempDir;

    use super::*;

    struct Tester {
        temp_dir: TempDir,
    }

    impl Tester {
        fn new() -> Self {
            Self {
                temp_dir: TempDir::new("").unwrap(),
            }
        }

        fn path_buf(&self) -> PathBuf {
            self.temp_dir.path().to_path_buf()
        }

        fn save_empty_image(
            &self,
            path: impl AsRef<Path>,
            size: u32,
            format: ImageFormat,
        ) -> PathBuf {
            let img = RgbImage::new(size, size);
            let path = self.temp_dir.path().join(path);
            if let Some(parent_path) = path.parent() {
                fs::create_dir_all(parent_path).unwrap();
            }
            dbg!(&path);
            img.save_with_format(&path, format).unwrap();
            path
        }
    }

    #[test]
    fn convert_jpg_to_png() {
        const SIZE: u32 = 32;
        let tester = Tester::new();
        let input_path = tester.save_empty_image("my_image.jpg", SIZE, ImageFormat::Jpeg);

        let args = Args {
            path: input_path.display().to_string(),
            command: Some(Commands::Convert {
                target_format: "png".to_owned(),
            }),
            ..Default::default()
        };

        let context = Context {
            stdout: &mut io::stdout(),
        };

        run(context, args).unwrap();

        let output_path = input_path.with_extension("png");
        let reader = ImageReader::open(&output_path).unwrap();
        assert_eq!(reader.format(), Some(ImageFormat::Png));
        assert_eq!(reader.into_dimensions().unwrap(), (SIZE, SIZE));

        let reader = ImageReader::open(&output_path).unwrap();
        let result = reader.decode();
        assert!(result.is_ok());
    }

    #[test]
    fn convert_folder_jpg_to_png() {
        const SIZE: u32 = 32;
        let tester = Tester::new();
        let folder_path = PathBuf::from("folder");
        let input_path0 =
            tester.save_empty_image(folder_path.join("my_image0.jpg"), SIZE, ImageFormat::Jpeg);
        let input_path1 =
            tester.save_empty_image(folder_path.join("my_image1.jpg"), SIZE, ImageFormat::Jpeg);

        let args = Args {
            path: tester.path_buf().join(folder_path).display().to_string(),
            command: Some(Commands::Convert {
                target_format: "png".to_owned(),
            }),
            ..Default::default()
        };

        let context = Context {
            stdout: &mut io::stdout(),
        };

        run(context, args).unwrap();

        fn assert_file(path: &Path) {
            let output_path = path.with_extension("png");

            let reader = ImageReader::open(&output_path).unwrap();
            assert_eq!(reader.format(), Some(ImageFormat::Png));
            assert_eq!(reader.into_dimensions().unwrap(), (SIZE, SIZE));

            let reader = ImageReader::open(&output_path).unwrap();
            let result = reader.decode();
            assert!(result.is_ok());
        }

        assert_file(&input_path0);
        assert_file(&input_path1);
    }

    #[test]
    fn is_not_png() {
        const SIZE: u32 = 32;
        let tester = Tester::new();
        let input_path = tester.save_empty_image("my_image.jpg", SIZE, ImageFormat::Jpeg);

        let args = Args {
            path: input_path.display().to_string(),
            command: Some(Commands::Is {
                format: "png".to_owned(),
            }),
            ..Default::default()
        };

        let context = Context {
            stdout: &mut io::stdout(),
        };

        assert!(run(context, args).is_err());
    }

    #[test]
    fn is_png() {
        const SIZE: u32 = 32;
        let tester = Tester::new();
        let input_path = tester.save_empty_image("my_image.png", SIZE, ImageFormat::Png);

        let args = Args {
            path: input_path.display().to_string(),
            command: Some(Commands::Is {
                format: "png".to_owned(),
            }),
            ..Default::default()
        };

        let context = Context {
            stdout: &mut io::stdout(),
        };

        assert!(run(context, args).is_ok());
    }

    #[test]
    fn info_png() {
        const SIZE: u32 = 32;
        let tester = Tester::new();
        let input_path = tester.save_empty_image("my_image.png", SIZE, ImageFormat::Png);

        let args = Args {
            path: input_path.display().to_string(),
            command: Some(Commands::Info),
            ..Default::default()
        };

        let mut stdout = Vec::new();
        let context = Context {
            stdout: &mut stdout,
        };

        run(context, args).unwrap();

        let expected = format!("{} png\n", input_path.display());
        let found = str::from_utf8(&stdout).unwrap();
        assert_eq!(found, expected);
    }
}
