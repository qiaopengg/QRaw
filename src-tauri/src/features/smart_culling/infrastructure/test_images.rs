use image::{DynamicImage, ImageFormat};
use std::io::Cursor;

pub(super) fn jpeg_with_orientation(image: &DynamicImage, orientation: u8) -> Vec<u8> {
    let mut encoded = Cursor::new(Vec::new());
    image.write_to(&mut encoded, ImageFormat::Jpeg).unwrap();
    let jpeg = encoded.into_inner();
    // Minimal TIFF IFD with one SHORT Orientation field.
    let mut app1 = b"Exif\0\0II\x2a\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0".to_vec();
    app1.extend([orientation, 0, 0, 0, 0, 0, 0, 0]);
    let mut tagged = jpeg[..2].to_vec();
    tagged.extend([0xff, 0xe1]);
    tagged.extend(((app1.len() + 2) as u16).to_be_bytes());
    tagged.extend(app1);
    tagged.extend_from_slice(&jpeg[2..]);
    tagged
}
