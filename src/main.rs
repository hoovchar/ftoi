use std::{
    env::args,
    error::Error,
    fs::File,
    io::{BufReader, Read, Write},
};

use image::{io::Reader, GenericImageView, Rgba, RgbaImage};
use lzma::LzmaWriter;

fn main() -> Result<(), Box<dyn Error>> {
    let argv = args().collect::<Vec<String>>();

    let real_args: Vec<String> = argv
        .iter()
        .filter(|i| !i.starts_with('-'))
        .map(|i| i.to_string())
        .collect();

    let sub_args: String = argv
        .iter()
        .filter(|i| i.starts_with('-'))
        .map(|i| i[1..].to_string())
        .collect::<Vec<String>>()
        .join("");

    let do_compression = sub_args.contains('c');
    let mode = match sub_args.contains('d') {
        true => Mode::Decode,
        false => Mode::Encode,
    };
    let file_path = match real_args.get(1) {
        None => {
            eprintln!("No file specified!");
            send_help();
            panic!()
        }
        Some(path) => path.to_string(),
    };
    let output_path = match real_args.get(2) {
        None => "/dev/stdout".to_string(),
        Some(path) => path.to_string(),
    };

    let mut file = File::open(&file_path)?;

    match mode {
        Mode::Encode => {
            let file_size = file.metadata()?.len();
            eprintln!("Real file size: {}", file_size);
            let mut file_data: Vec<u8> = Vec::new();

            if do_compression {
                let mut compressor = LzmaWriter::new_compressor(&mut file_data, 7)?;
                let mut buf = [0u8; 1024];
                loop {
                    match file.read(&mut buf) {
                        Err(e) => panic!("Error: {}", e),
                        Ok(size) => {
                            if size == 0 {
                                break;
                            }
                            compressor.write_all(&buf[..size])?;
                        }
                    }
                }
                compressor.finish()?;
                eprintln!("Compressed file size: {}", file_data.len());
            } else {
                file.read_to_end(&mut file_data)?;
            }

            let image_xy_size = ((file_data.len() + 8) as f64 / 4.0).sqrt().ceil() as u32;
            let mut image = RgbaImage::new(image_xy_size, image_xy_size);
            let mut file_data_reader = BufReader::new(file_data.as_slice());
            let mut buf = [0u8; 4];
            let file_size_be_bytes = file_data.len().to_be_bytes();
            let mut file_size_be_bytes_reader = BufReader::new(file_size_be_bytes.as_slice());
            file_size_be_bytes_reader.read_exact(&mut buf)?;
            image.put_pixel(0, 0, Rgba(buf));
            file_size_be_bytes_reader.read_exact(&mut buf)?;
            image.put_pixel(0, 1, Rgba(buf));

            for x in 0..image_xy_size {
                for y in 0..image_xy_size {
                    if (y == 0 || y == 1) && x == 0 {
                        continue;
                    }
                    file_data_reader.read(&mut buf)?;
                    image.put_pixel(x, y, Rgba(buf))
                }
            }

            image.save(output_path)?;
        }
        Mode::Decode => {
            let image = Reader::open(file_path)?.decode()?;
            let mut image_data: Vec<u8> = Vec::new();

            let file_size = {
                let mut r = [0u8; 8];
                let p1 = image.get_pixel(0, 0).0;
                let p2 = image.get_pixel(0, 1).0;
                r[..4usize].copy_from_slice(&p1);
                r[4..].copy_from_slice(&p2);

                u64::from_be_bytes(r)
            };
            println!("{}", file_size);
            let max_y = image.width();
            let max_x = image.height();

            for x in 0..max_x {
                for y in 0..max_y {
                    if (y == 0 || y == 1) && x == 0 {
                        continue;
                    }
                    image_data.append(&mut image.get_pixel(x, y).0.to_vec())
                }
            }
            let mut output_file = File::create(output_path)?;
            let mut take = image_data.take(file_size);
            let mut buf = [0u8; 1024];

            if do_compression {
                let mut decompressor = LzmaWriter::new_decompressor(output_file)?;
                loop {
                    match take.read(&mut buf) {
                        Err(e) => panic!("Error: {}", e),
                        Ok(size) => {
                            if size == 0 {
                                break;
                            }
                            decompressor.write_all(&buf[..size])?;
                        }
                    }
                }
                decompressor.finish()?;
            } else {
                loop {
                    match take.read(&mut buf) {
                        Err(e) => panic!("Error: {}", e),
                        Ok(size) => {
                            if size == 0 {
                                break;
                            }
                            output_file.write_all(&buf[..size])?;
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

fn send_help() {
    println!(
        "\nftoi [args] input_file output_file\n\
    \nargs:\n\
    \t-e -- Encode a file into an image\n\
    \t-d -- Decode an image into a file\n\
    \t-c -- Encode/Decode with compression applied.\n\
    "
    )
}

enum Mode {
    Decode,
    Encode,
}
