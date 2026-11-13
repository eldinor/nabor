use nabor_core::*;
use std::fs;

#[test]
fn transparent_thumbnails_rebuild_legacy_cache_and_keep_alpha_and_original_bytes() {
    let t = tempfile::tempdir().unwrap();
    let (c, _) = Collection::open(&t.path().join("collection"), true).unwrap();
    let bytes = encode_clipboard(
        &[
            255, 0, 0, 0, 0, 255, 0, 128, 0, 0, 255, 255, 255, 255, 255, 255,
        ],
        2,
        2,
    )
    .unwrap();
    let i = c
        .import_image(None, "transparent.png", &bytes, None)
        .unwrap();
    let a = &i.assets[0];
    let legacy = c
        .root
        .join(".nabor/thumbnails")
        .join(format!("{}.jpg", a.id));
    fs::write(&legacy, b"old cached preview").unwrap();
    let output = thumbnail(&c.root, &i, a).unwrap().unwrap();
    assert_eq!(output.extension().unwrap(), "png");
    let image = image::open(&output).unwrap().to_rgba8();
    assert_eq!(image.get_pixel(0, 0).0[3], 0);
    assert_eq!(image.get_pixel(1, 0).0[3], 128);
    assert_eq!(image.get_pixel(0, 1).0, [0, 0, 255, 255]);
    assert!(!legacy.exists());
    assert_eq!(fs::read(c.resolve(&i, a).unwrap()).unwrap(), bytes);
    assert_eq!(cached_thumbnail(&c.root, &a.id).unwrap(), output);
    fs::remove_file(c.resolve(&i, a).unwrap()).unwrap();
    assert_eq!(thumbnail(&c.root, &i, a).unwrap().unwrap(), output);
}

#[test]
fn opaque_images_stay_jpeg_and_replacement_removes_obsolete_alpha_cache() {
    let t = tempfile::tempdir().unwrap();
    let (c, _) = Collection::open(&t.path().join("collection"), true).unwrap();
    let i = c
        .import_image(
            None,
            "source.png",
            &encode_clipboard(&[4, 20, 60, 0], 1, 1).unwrap(),
            None,
        )
        .unwrap();
    let a = &i.assets[0];
    let png = thumbnail(&c.root, &i, a).unwrap().unwrap();
    fs::write(
        c.resolve(&i, a).unwrap(),
        encode_clipboard(&[4, 20, 60, 255, 4, 20, 60, 255], 2, 1).unwrap(),
    )
    .unwrap();
    let jpeg = thumbnail(&c.root, &i, a).unwrap().unwrap();
    assert_eq!(jpeg.extension().unwrap(), "jpg");
    assert!(!png.exists());
    assert_eq!(cached_thumbnail(&c.root, &a.id).unwrap(), jpeg);
}
