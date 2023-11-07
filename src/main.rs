use std::{env, thread, io};
use std::io::Write;
use std::path::Path;
use std::time::Instant;
use colored::Colorize;

use crossterm::{QueueableCommand, cursor};
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
        println!("Time spent: {}", format!("{:.2?}", self.time.elapsed()).bold().truecolor(255, 127, 0));
    }
}

struct Printer {
    prev_str: String,
    offset: u16,
}

impl Printer {
    fn new() -> Printer {
        return Printer {prev_str: String::new(), offset: crossterm::cursor::position().unwrap().1}
    }

    fn reserve_line(&mut self, offset: u16) {
        self.offset = self.offset + offset;
    }

    fn start_print(&mut self, str: String) {
        let mut stdout = io::stdout();
        self.prev_str = str;
        stdout.queue(cursor::MoveTo(0, self.offset)).unwrap();
        stdout.write_all(self.prev_str.as_bytes()).unwrap();
    }

    fn finish_print(&self, res: bool) {
        let mut stdout = io::stdout();
        stdout.queue(cursor::MoveTo(0, self.offset)).unwrap(); 
        let padding = 96 - self.prev_str.len();
        if res {
            stdout.write_all(format!("{} {: >padding$}\n", self.prev_str, "done".bold().green()).as_bytes()).unwrap();
        } else {
            stdout.write_all(format!("{} {: >padding$}\n", self.prev_str, "fail".bold().red()).as_bytes()).unwrap();
        }
        stdout.flush().unwrap();
    }

    fn fail_print(&mut self, str: String) {
        let mut stdout = io::stdout();
        stdout.write_all(format!("{}\n", str.red().bold()).as_bytes()).unwrap();
        stdout.flush().unwrap();
    }

    fn warn_print(&mut self, str: String) {
        let mut stdout = io::stdout();
        stdout.write_all(format!("{}\n", str.truecolor(128, 128, 0)).as_bytes()).unwrap();
        stdout.flush().unwrap();
    }

}

fn help() {
    println!("Usage: \n
    -0 <path> Path of source image 0
    -1 <path> Path of source image 1
    -2 <path> Path of source image 2
    -3 <path> Path of source image 3
    -s <mask> The swizzle mask, default is bbbw
    -m <mask> The select mask, default is rrrr
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

    In the above examples, the s prefix corresponds to a source image.

    The select mask (-m) selects which channel from the source image to select. By default it's [r, r, r, r]
");
}

/// Open images for reading
fn open_source_images(paths: &Vec<Option<&std::path::Path>>) -> [Option<DynamicImage>; 4] {

    // Setup array of image results
    let mut rets = [None, None, None, None];
    let mut rets_iter = rets.iter_mut();
    thread::scope(|s| {

        // Each file will have it's own line in the output
        let mut cursor_offset: u16 = 0;
        for path_idx in 0..paths.len() {
            if let Some(path) = paths[path_idx] {

                // Create a new printer for the thread
                let mut local_printer = Printer::new();

                // Assign the printer a line
                local_printer.reserve_line(cursor_offset);
                cursor_offset += 1;
                local_printer.start_print(format!("Reading {}", path.to_str().unwrap().bold()));
                let ret = rets_iter.next().unwrap();
                s.spawn(move || {
                    let path_str = path.to_str().unwrap();
                    let image_reader = image::io::Reader::open(path_str);
                    if let Ok(image) = image_reader {
                        if let Ok(image_raw) = image.decode() {

                            // Success, terminate thread
                            local_printer.finish_print(true);
                            ret.replace(image_raw);
                            return;
                        }
                    }
        
                    local_printer.finish_print(false);
                });
            }
        }        
    });

    return rets;
}

fn main() {
    let args : Vec<String> = env::args().collect();
    let mut printer = Printer::new();
    let mut paths : Vec<Option<&Path>> = vec![None, None, None, None];
    let mut swizzle_mask: &str = "bbbw";
    let mut select_mask: &str = "rrrr";
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
            "-m" => select_mask = args[arg_i + 1].as_str(),
            "-s" => swizzle_mask = args[arg_i + 1].as_str(),
            "-o" => output_path = Some(Path::new(args[arg_i + 1].as_str())),
            _ => {
                help();
                return;
            },
        }
    }

    if swizzle_mask.len() < 2 {
        printer.fail_print("Swizzle mask is less than 2, nothing to do here...".to_string());
        return;
    }

    // Get output image
    if let Some(path) = output_path {
        if let Some(parent) = path.parent() {

            let mut timer = Timer::new();

            // Create entire file path to file
            if let Err(_) = std::fs::create_dir_all(parent) {
                printer.fail_print(format!("Invalid path {}", parent.to_str().unwrap()));
                return;
            }

            // Open files
            let mut channel_selects: Vec<usize> = vec![0, 0, 0, 0];
            let select_mask_bytes = select_mask.as_bytes();

            let images = open_source_images(&paths);
            for path_idx in 0..paths.len() {
                if let Some(path) = paths[path_idx] {

                    match select_mask_bytes[path_idx] as char {
                        'r' => channel_selects[path_idx] = 0,
                        'g' => channel_selects[path_idx] = 1,
                        'b' => channel_selects[path_idx] = 2,
                        'a' => channel_selects[path_idx] = 3,
                        other => {
                            printer.fail_print(format!("Invalid select mask {} for input {}", other as char, path_idx));
                            help();
                            return;
                        }
                    }
                }
            }

            // Break down swizzle mask into components
            let mut fill = Rgba([0, 0, 0, 255]);
            let swizzles: Vec<Option<u32>> = swizzle_mask.chars().map(|f| f.to_digit(10)).collect();
            let mut swizzled_images = Vec::<&[u8]>::new();
            let mut byte_strides = Vec::<u8>::new();
            let mut red_channel_strides = Vec::<u8>::new();
            let mut formats = Vec::<ChannelFormat>::new();
            for channel in 0..swizzles.len() {
                if let Some(swizzle) = swizzles[channel] {
                    if swizzle > 3 {
                        printer.fail_print(format!("Swizzle mask contains source image out of bounds {}", swizzle));
                        help();
                        return;
                    }
                    if let Some(file) = &images[swizzle as usize] {
                        swizzled_images.push(file.as_bytes());
                        byte_strides.push(file.color().bytes_per_pixel());
                        let channel_count = file.color().channel_count();
                        red_channel_strides.push(channel_count);
                        if channel_count <= channel_selects[swizzle as usize] as u8 {
                            printer.warn_print(format!("[WARNING] Input {} has {} channel(s) but select mask is '{}', clamping channel to {}", swizzle, channel_count, select_mask_bytes[swizzle as usize] as char, channel_count));
                            channel_selects[swizzle as usize] = (channel_count - 1) as usize;
                        }
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
                        printer.fail_print(format!("Swizzle mask needs input source '{}', but none provided", channel - 1));
                        help();
                        return;
                    }
                } else {
                    // If swizzle isn't a number, check if it uses any fill value
                    let swizzle_mask_bytes = swizzle_mask.as_bytes();
                    match swizzle_mask_bytes[channel] as char {
                        'b' => fill.0[channel] = 0,
                        'w' => fill.0[channel] = 255,
                        'g' => fill.0[channel] = 128,
                        _ => {
                            printer.fail_print(format!("Invalid swizzle character '{}'", swizzle_mask_bytes[channel] as char));
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
            for img_opt in &images {
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
                printer.fail_print("All input images must share the same size:".to_string());
                for img_idx in 0..images.len() {
                    if let Some(img) = &images[img_idx] {
                        printer.fail_print(format!("{} (Input {}) - width: {}, height: {}", paths[img_idx].unwrap().to_str().unwrap(), img_idx, img.width(), img.height()));
                    }
                }
                help();
                return;
            }

            let thread_job_size = width as usize * 32;
            let num_cpus = num_cpus::get(); // Assume hyperthreading

            // Create image
            printer.start_print(format!("Combining image {}", format!("{}x{}", width, height).bold()));
            let mut rgba: RgbaImage = RgbaImage::from_pixel(width, height, fill);

            for img_idx in 0..swizzled_images.len() {
                let read_stride = byte_strides[img_idx] as usize;
                let red_channel_stride = red_channel_strides[img_idx] as usize;
                let channel_select_offset = channel_selects[img_idx] as usize;
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
                                                ChannelFormat::Uint8 => source_chunk[i * red_channel_stride + channel_select_offset],
                                                ChannelFormat::Uint16 => {
                                                    std::mem::transmute::<&[u8], &[u16]>(source_chunk)[i * red_channel_stride + channel_select_offset] as u8
                                                },
                                                ChannelFormat::Float32 => {
                                                    std::mem::transmute::<&[u8], &[f32]>(source_chunk)[i * red_channel_stride + channel_select_offset] as u8
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
            printer.finish_print(true);

            // Finally save file
            printer.start_print(format!("Writing out {}", path.to_str().unwrap().bold()));
            io::stdout().flush().unwrap();
            if let Ok(_) = rgba.save_with_format(path, image::ImageFormat::Png) {
                printer.finish_print(true);
            } else {
                printer.finish_print(false);
            }

            timer.elapsed();
        }        
    }
    
}
