use std::{env, thread, io};
use std::io::Write;
use std::path::Path;

use image::{RgbaImage, DynamicImage, Rgba};

fn help() {
    println!("Usage: \n
    -0 <path> Path to AO texture
    -1 <path> Path to Metalness texture
    -2 <path> Path to Roughness texture
    -3 <path> Path to Emissive texture
    -s <mask> The swizzle mask, default is 0123
    -o <path> Output
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

            // Create entire file path to file
            if let Err(e) = std::fs::create_dir_all(parent) {
                println!("Invalid path {}", parent.to_str().unwrap());
                return;
            }

            use std::time::Instant;
            let mut now = Instant::now();

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
            let swizzles: Vec<usize> = swizzle_mask.chars().map(|f| f as usize - '0' as usize).collect();
            let mut swizzled_images = Vec::<RgbaImage>::new();
            for channel in 0..swizzles.len() {
                if let Some(file) = &files[swizzles[channel]] {
                    swizzled_images.push(file.to_rgba8());
                } else {
                    println!("Swizzle mask needs source {}, but none provided", swizzle_mask.as_bytes()[channel]);
                    help();
                    return;
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

            const THREAD_JOB_SIZE: usize = 2048;
            let num_cpus = num_cpus::get();

            // Create image
            print!("Combining image (using {} threads)... ", num_cpus);
            io::stdout().flush().unwrap();
            let mut rgba: RgbaImage = RgbaImage::from_pixel(width, height, Rgba([0, 0, 0, 255]));

            for img_idx in 0..swizzled_images.len() {
                let mut source_data = swizzled_images[img_idx].chunks(THREAD_JOB_SIZE * 4);
                let mut dest_data = rgba.chunks_mut(THREAD_JOB_SIZE * 4);

                for _ in (0..source_data.len()).step_by(num_cpus) {
                    thread::scope(|s: &thread::Scope<'_, '_>| {
                        for _ in 0..num_cpus {
                            if let Some(source_chunk) = source_data.next() {
                                let dest_chunk = dest_data.next().unwrap();
                                s.spawn(|| {
                                    for i in 0..THREAD_JOB_SIZE {
                                        let value = source_chunk[i * 4];
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

            let elapsed = now.elapsed();
            println!("Time spent: {:.2?}", elapsed);
        }        
    }
    
}
