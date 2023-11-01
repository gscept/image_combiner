use std::{env, thread, io};
use std::io::Write;
use std::path::Path;
use std::time::Instant;

use image::{RgbaImage, DynamicImage, Rgba};

#[derive(Clone, Copy, Debug)]
enum ChannelFormat {
    Uint8,
    Uint16,
    Float32
}
struct Timer {
    time: Instant
}

impl Timer {

    fn new() -> Timer {
        return Timer { time: Instant::now() }
    }

    /// Start the timer
    fn start(&mut self) {
        self.time = Instant::now();
    }

    /// Write current elapsed time and reset timer
    fn elapsed(&mut self) {
        self.stop();
        self.start();
    }

    /// Stop the timer and print output
    fn stop(&self) {
        println!("Time spent: {:.2?}", self.time.elapsed());
    }

}
fn help() {
    println!("Usage: \n
    -0 <path> Path of source image 0
    -1 <path> Path of source image 1
    -2 <path> Path of source image 2
    -3 <path> Path of source image 3
    -s <mask> The swizzle mask, default is bbbw
    -o <path> Output path

    The swizzle mask (-s) maps the value in the mask to the channel in the output image corresponding to its index.
    Allowed values are:
        [0..3] - Reads the image at index
        b, w, g - Fills with either 0 (b), 255 (w) or 128 (g)

    By default, the swizzle mask is bbbw which means the output image will have [0, 0, 0, 255] in every channel. 
    Example 1: 
        Mask 0123 would map [s0, s1, s2, s3] to output [r, g, b, a] by extracting the first channel in each source image.
    Example 2: 
        Mask 01bw would map [s0, s1, 0, 255] to output [r, g, b, a].

    In the above examples, the s prefix corresponds to a source image
");
}

/// Open source image for reading
fn open_source_img(path: &std::path::Path) -> Option<DynamicImage> {
    print!("Reading {}... ", path.to_str().unwrap());
    let image_read = image::io::Reader::open(path);
    if let Ok(image) = image_read {
        if let Ok(image_raw) = image.decode() {
            println!("Done");
            return Some(image_raw);
        }
    }

    println!("Failed");
    return None;
}

fn main() {
    let args : Vec<String> = env::args().collect();
    let mut paths : Vec<Option<&Path>> = vec![None, None, None, None];
    let mut swizzle_mask: &str = "";
    let mut output_path = None;
    if (1..args.len()).len() % 2 != 0 {
        help();
        return;
    }
    for arg_i in (1..args.len()).step_by(2) {
        match args[arg_i].as_str() {
            "-0" => paths[0] = Some(Path::new(args[arg_i + 1].as_str())),
            "-1" => paths[1] = Some(Path::new(args[arg_i + 1].as_str())),
            "-2" => paths[2] = Some(Path::new(args[arg_i + 1].as_str())),
            "-3" => paths[3] = Some(Path::new(args[arg_i + 1].as_str())),
            "-s" => swizzle_mask = args[arg_i + 1].as_str(),
            "-o" => output_path = Some(Path::new(args[arg_i + 1].as_str())),
            _ => {
                help();
                return;
            },
        }
    }

    if swizzle_mask.len() < 2 {
        println!("Swizzle mask is less than 2, nothing to do here...");
        return;
    }

    // Get output image
    if let Some(path) = output_path {
        if let Some(parent) = path.parent() {

            let mut timer = Timer::new();

            // Create entire file path to file
            if let Err(e) = std::fs::create_dir_all(parent) {
                println!("Invalid path {}", parent.to_str().unwrap());
                return;
            }

            // Open files
            let mut files: Vec<Option<DynamicImage>> = vec![None, None, None, None];

            for path_idx in 0..paths.len() {
                if let Some(path) = paths[path_idx] {
                    files[path_idx] = open_source_img(path);

                    if let None = files[path_idx] {
                        println!("Invalid path {} for input -{}", path.to_str().unwrap(), path_idx);
                        help();
                        return;
                    }
                }
            }

            // Break down swizzle mask into components
            let mut fill = Rgba([0, 0, 0, 255]);
            let chars = swizzle_mask.as_bytes();
            let swizzles: Vec<Option<u32>> = swizzle_mask.chars().map(|f| f.to_digit(10)).collect();
            let mut swizzled_images = Vec::<&[u8]>::new();
            let mut byte_strides = Vec::<u8>::new();
            let mut red_channel_strides = Vec::<u8>::new();
            let mut formats = Vec::<ChannelFormat>::new();
            for channel in 0..swizzles.len() {
                if let Some(swizzle) = swizzles[channel] {
                    if let Some(file) = &files[swizzle as usize] {
                        swizzled_images.push(file.as_bytes());
                        byte_strides.push(file.color().bytes_per_pixel());
                        red_channel_strides.push(file.color().channel_count());
                        let format = match file.color() {
                            image::ColorType::L8 => ChannelFormat::Uint8,
                            image::ColorType::La8 => ChannelFormat::Uint8,
                            image::ColorType::Rgb8 => ChannelFormat::Uint8,
                            image::ColorType::Rgba8 => ChannelFormat::Uint8,
    
                            image::ColorType::L16 => ChannelFormat::Uint16,
                            image::ColorType::La16 => ChannelFormat::Uint16,
                            image::ColorType::Rgb16 => ChannelFormat::Uint16,
                            image::ColorType::Rgba16 => ChannelFormat::Uint16,
    
                            image::ColorType::Rgb32F => ChannelFormat::Float32,
                            image::ColorType::Rgba32F => ChannelFormat::Float32,
                            _ => ChannelFormat::Uint8
                        };
                        formats.push(format);
                    } else {
                        println!("Swizzle mask needs source {}, but none provided", swizzle_mask.as_bytes()[channel]);
                        help();
                        return;
                    }
                } else {
                    // If swizzle isn't a number, check if it uses any fill value
                    match chars[channel] as char {
                        'b' => fill.0[channel] = 0,
                        'w' => fill.0[channel] = 255,
                        'g' => fill.0[channel] = 128,
                        _ => {
                            println!("Invalid swizzle character '{}'", chars[channel] as char);
                            help();
                            return;
                        }
                    }
                }
               
            }

            // Assert all images have the same size
            let mut width = 0xFFFFFFFF;
            let mut height = 0xFFFFFFFF;
            let mut image_size_mismatch = false;

            // Get dimensions of images
            for img_opt in &files {
                if let Some(img) = img_opt {
                    if width == 0xFFFFFFFF || height == 0xFFFFFFFF {
                        width = img.width();
                        height = img.height();
                    } else {
                        if width != img.width() || height != img.height() {
                            image_size_mismatch = true;
                            break;
                        }
                    }                    
                }
            }

            // If any size mismatches, throw error
            if image_size_mismatch {
                println!("All input images must share the same size:");
                for img_idx in 0..files.len() {
                    if let Some(img) = &files[img_idx] {
                        println!("{} (argument -{}) - width: {}, height: {}", paths[img_idx].unwrap().to_str().unwrap(), img_idx, img.width(), img.height());
                    }
                }
                help();
                return;
            }

            let thread_job_size = width as usize * 32;
            let num_cpus = num_cpus::get(); // Assume hyperthreading

            // Create image
            print!("Combining image (using {} threads)... ", num_cpus);
            io::stdout().flush().unwrap();
            let mut rgba: RgbaImage = RgbaImage::from_pixel(width, height, fill);

            for img_idx in 0..swizzled_images.len() {
                let read_stride = byte_strides[img_idx] as usize;
                let red_channel_stride = red_channel_strides[img_idx] as usize;
                let format = formats[img_idx];
                let mut source_data = swizzled_images[img_idx].chunks(thread_job_size * read_stride);
                let mut dest_data = rgba.chunks_mut(thread_job_size * 4);

                for _ in (0..source_data.len()).step_by(num_cpus) {
                    thread::scope(|s: &thread::Scope<'_, '_>| {
                        for _ in 0..num_cpus {
                            if let Some(source_chunk) = source_data.next() {
                                let dest_chunk = dest_data.next().unwrap();
                                s.spawn(|| {
                                    for i in 0..thread_job_size {
                                        let value : u8;
                                        unsafe {
                                            value = match format {
                                                ChannelFormat::Uint8 => source_chunk[i * red_channel_stride],
                                                ChannelFormat::Uint16 => {
                                                    std::mem::transmute::<&[u8], &[u16]>(source_chunk)[i * red_channel_stride] as u8
                                                },
                                                ChannelFormat::Float32 => {
                                                    std::mem::transmute::<&[u8], &[f32]>(source_chunk)[i * red_channel_stride] as u8
                                                }
                                            }
                                        }
                                        dest_chunk[i * 4 + img_idx] = value;
                                    }
                                });
                            }
                        }
                    });
                }
            }
            println!("Done");

            // Finally save file
            print!("Writing out {}... ", path.to_str().unwrap());
            io::stdout().flush().unwrap();
            rgba.save_with_format(path, image::ImageFormat::Png).unwrap();
            println!("Done");

            timer.elapsed();
        }        
    }
    
}
