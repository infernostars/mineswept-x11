pub(crate) fn rgba_to_bgra(rgba: &[u8]) -> Vec<u8> {
    // Ensure the input length is a multiple of 4
    assert!(rgba.len() % 4 == 0, "Input length must be a multiple of 4");

    // Create a vector to hold the converted BGRA data
    let mut bgra = Vec::with_capacity(rgba.len());

    // Iterate over the input data in chunks of 4 (representing one pixel)
    for pixel in rgba.chunks(4) {
        // Extract RGBA components
        let r = pixel[0];
        let g = pixel[1];
        let b = pixel[2];
        let a = pixel[3];

        // Push BGRA components to the output vector
        bgra.push(b); // Blue
        bgra.push(g); // Green
        bgra.push(r); // Red
        bgra.push(0); // Alpha
    }

    bgra
}