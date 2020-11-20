use std::path::Path;

fn main() {
    let file = if std::env::args().count() == 2 {
        std::env::args().nth(1).unwrap()
    } else {
        panic!("Please enter an image filename")
    };

    let im = image::open(&Path::new(&file)).unwrap();
    let output = clahe_rs::clahe(im.to_luma()).unwrap();

    output
        .save_with_format(&Path::new("output.png"), image::ImageFormat::Png)
        .unwrap();
}
