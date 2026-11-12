//! Texture images in the document (Basic Materials part 2): stored once by
//! their content, undone with the step that added them, purged when no
//! material uses them.

use prism_doc::{Command, Document, Image, Material, Texture};

fn png(rgba: [u8; 4]) -> Vec<u8> {
    let img = image::RgbaImage::from_pixel(4, 4, image::Rgba(rgba));
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png)
        .expect("png");
    out.into_inner()
}

fn textured(name: &str, image: &Image) -> Material {
    let mut m = Material::colour(name, [200, 200, 200]);
    m.texture = Some(Texture::of(image));
    m
}

#[test]
fn an_image_is_stored_once_and_purged_when_unused() {
    let mut d = Document::default();
    let brick = Image::read(png([180, 60, 40, 255])).expect("a png");
    // Two materials with the same image: it is kept once.
    for name in ["Brick", "Brick again"] {
        d.run(Command::Batch {
            commands: vec![
                Command::AddImage {
                    image: brick.clone(),
                },
                Command::AddMaterial {
                    material: textured(name, &brick),
                },
            ],
        });
    }
    assert_eq!(d.assembly().images().len(), 1);
    assert_eq!(d.undo_name(), Some("New Material"), "named by the material");
    // Undo takes the step's image away with it (here, still used once).
    assert!(d.undo());
    assert_eq!(d.assembly().images().len(), 1);
    assert!(d.undo());
    assert!(d.assembly().images().is_empty(), "undone with its step");
    assert!(d.redo() && d.redo());
    // A second image nothing uses goes with Purge, and so does the first
    // once its materials are unused and purged.
    d.run(Command::AddImage {
        image: Image::read(png([0, 0, 0, 255])).unwrap(),
    });
    assert_eq!(d.assembly().unused_images().len(), 1);
    d.run(Command::PurgeMaterials);
    assert!(d.assembly().images().is_empty());
    assert!(d.assembly().materials().0.is_empty());
}

#[test]
fn a_texture_naming_a_missing_image_is_dropped() {
    let brick = Image::read(png([180, 60, 40, 255])).unwrap();
    let asm = prism_doc::Assembly::default()
        .with_materials(
            [(prism_topo::MaterialId(0), textured("Brick", &brick))].into(),
            1,
            Default::default(),
        )
        .with_images(Default::default());
    let m = asm.material(prism_topo::MaterialId(0)).expect("kept");
    assert_eq!(m.texture, None, "the colour stays, the texture goes");
}
